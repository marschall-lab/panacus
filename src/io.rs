/* standard use */
use std::collections::HashMap;
use std::io::{BufRead, BufReader, BufWriter, Read, Write};
use std::io::{Error, ErrorKind};
use std::path::Path;
use std::str::{self, FromStr};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

/* external use */
use flate2::read::MultiGzDecoder;
use itertools::Itertools;
use quick_csv::Csv;
use rayon::prelude::*;
use strum_macros::{EnumString, EnumVariantNames};

/* internal use */
use crate::abacus::*;
use crate::graph::*;
use crate::hist::*;
use crate::html::*;
use crate::util::*;

#[derive(Debug, Clone, Copy, PartialEq, EnumString, EnumVariantNames)]
#[strum(serialize_all = "lowercase")]
pub enum OutputFormat {
    Table,
    Html,
}

pub fn bufreader_from_compressed_gfa(gfa_file: &str) -> BufReader<Box<dyn Read>> {
    log::info!("loading graph from {}", &gfa_file);
    let f = std::fs::File::open(gfa_file).expect("Error opening file");
    let reader: Box<dyn Read> = if gfa_file.ends_with(".gz") {
        log::info!("assuming that {} is gzip compressed..", &gfa_file);
        Box::new(MultiGzDecoder::new(f))
    } else {
        Box::new(f)
    };
    BufReader::new(reader)
}

pub fn parse_bed_to_path_segments<R: Read>(
    data: &mut BufReader<R>,
    use_block_info: bool,
) -> Vec<PathSegment> {
    // based on https://en.wikipedia.org/wiki/BED_(file_format)
    let mut segments = Vec::new();

    for (i, line) in data.lines().enumerate() {
        let line = match line {
            Ok(l) => l,
            Err(e) => {
                panic!("error reading line {}: {}", i + 1, e);
            }
        };

        let fields = {
            let mut fields: Vec<&str> = line.split('\t').collect();
            if fields.is_empty() {
                fields = vec![&line];
            }
            fields
        };
        let path_name = fields[0];

        if path_name.starts_with("browser ")
            || path_name.starts_with("track ")
            || path_name.starts_with("#")
        {
            continue;
        }

        if fields.len() == 1 {
            segments.push(PathSegment::from_str(path_name));
        } else if fields.len() >= 3 {
            let start = usize::from_str(fields[1]).expect(&format!(
                "error line {}: `{}` is not an usize",
                i + 1,
                fields[1]
            ));
            let end = usize::from_str(fields[2]).expect(&format!(
                "error line {}: `{}` is not an usize",
                i + 1,
                fields[2]
            ));

            if use_block_info && fields.len() == 12 {
                let block_count = fields[9].parse::<usize>().unwrap_or(0);
                let block_sizes: Vec<usize> = fields[10]
                    .split(',')
                    .filter_map(|s| usize::from_str(s.trim()).ok())
                    .collect();
                let block_starts: Vec<usize> = fields[11]
                    .split(',')
                    .filter_map(|s| usize::from_str(s.trim()).ok())
                    .collect();

                if block_count == block_sizes.len() && block_count == block_starts.len() {
                    for (size, start_offset) in block_sizes.iter().zip(block_starts.iter()) {
                        let block_start = start + start_offset;
                        let block_end = block_start + size;
                        segments.push(PathSegment::from_str_start_end(
                            path_name,
                            block_start,
                            block_end,
                        ));
                    }
                } else {
                    panic!(
                        "error in block sizes/starts in line {}: counts do not match",
                        i + 1
                    );
                }
            } else {
                segments.push(PathSegment::from_str_start_end(path_name, start, end));
            }
        } else {
            panic!(
                "error in line {}: row must have either 1, 3, or 12 columns, but has 2",
                i + 1
            );
        }
    }

    segments
}

pub fn parse_groups<R: Read>(data: &mut BufReader<R>) -> Result<Vec<(PathSegment, String)>, Error> {
    let mut res: Vec<(PathSegment, String)> = Vec::new();

    let mut i = 1;
    let mut buf = vec![];
    while data.read_until(b'\n', &mut buf).unwrap_or(0) > 0 {
        //Remove new line at the end
        if let Some(&last_byte) = buf.last() {
            if last_byte == b'\n' || last_byte == b'\r' {
                buf.pop();
            }
        }
        let line = String::from_utf8(buf.clone())
            .expect(&format!("error in line {}: some character is not UTF-8", i));
        let columns: Vec<&str> = line.split('\t').collect();

        if columns.len() != 2 {
            let msg = format!("error in line {}: table must have exactly two columns", i);
            log::error!("{}", &msg);
            return Err(Error::new(ErrorKind::InvalidData, msg));
        }

        let path_seg = PathSegment::from_str(columns[0]);
        res.push((path_seg, columns[1].to_string()));

        i += 1;
        buf.clear();
    }

    Ok(res)
}

pub fn parse_tsv<R: Read>(
    data: &mut BufReader<R>,
) -> Result<(Vec<Vec<u8>>, Vec<Vec<Vec<u8>>>), Error> {
    let mut comments = Vec::new();
    let mut table = Vec::new();
    let reader = Csv::from_reader(data)
        .delimiter(b'\t')
        .flexible(true)
        .has_header(false);

    let mut is_header = true;
    for (i, row) in reader.enumerate() {
        let row: Vec<Vec<u8>> = row
            .map_err(|_| {
                let msg = format!("unable to parse row {}", i);
                log::error!("{}", &msg);
                Error::new(ErrorKind::Other, msg)
            })?
            .bytes_columns()
            .map(|x| x.to_vec())
            .collect();
        if is_header && (b'#' == row[0][0]) {
            let mut c = row[0].to_vec();
            for e in &row[1..] {
                c.push(b'\t');
                c.extend(e);
            }
            comments.push(c);
        } else {
            is_header = false;
            table.push(row);
        }
    }
    Ok((comments, table))
}

fn transpose_table(table: &Vec<Vec<Vec<u8>>>) -> Vec<Vec<&[u8]>> {
    let n = table.first().unwrap_or(&Vec::new()).len();

    let mut res = vec![vec![&table[0][0][..]; table.len()]; n];

    for j in 0..n {
        for i in 0..table.len() {
            res[j][i] = &table[i][j][..];
        }
    }

    res
}

fn parse_column(col: &Vec<&[u8]>, offset: usize) -> Result<Vec<usize>, Error> {
    let skip_lines = 2;
    let mut res = vec![0; col.len() - skip_lines];

    for (i, e) in col[skip_lines..].iter().enumerate() {
        if let Ok(val) = usize::from_str(str::from_utf8(e).unwrap()) {
            res[i] = val;
        } else {
            let msg = format!(
                "error in line {}: value must be integer, but is '{}'",
                i + 3 + offset,
                &str::from_utf8(e).unwrap()
            );
            log::error!("{}", &msg);
            Err(Error::new(ErrorKind::InvalidData, msg))?
        }
    }

    Ok(res)
}

pub fn parse_hists<R: Read>(
    data: &mut BufReader<R>,
) -> Result<(Vec<(CountType, Vec<usize>)>, Vec<Vec<u8>>), Error> {
    log::info!("loading coverage histogram from");
    let (comments, raw_table) = parse_tsv(data)?;
    let raw_table = transpose_table(&raw_table);
    if raw_table.len() < 4 && b"panacus" != raw_table[0][0] {
        let msg = format!(
            "error in line {}: table appears not to be generated by panacus",
            comments.len()
        );
        log::error!("{}", &msg);
        return Err(Error::new(ErrorKind::InvalidData, msg));
    }

    let mut res = Vec::new();

    let index = parse_column(&raw_table[0], comments.len())?;
    let mx = index.iter().max().unwrap();
    for col in &raw_table[1..] {
        if b"hist" == &col[0] {
            let count = CountType::from_str(str::from_utf8(col[1]).unwrap()).map_err(|_| {
                let msg = format!(
                    "error in line {}: expected count type declaration, but got '{}'",
                    2 + comments.len(),
                    &str::from_utf8(col[1]).unwrap()
                );
                log::error!("{}", &msg);
                Error::new(ErrorKind::InvalidData, msg)
            })?;
            let mut cov = vec![0; mx + 1];
            for (i, c) in index.iter().zip(parse_column(col, comments.len())?) {
                cov[*i] = c;
            }

            res.push((count, cov));
        }
    }

    if res.is_empty() {
        let msg = "table does not contain hist columns";
        log::error!("{}", msg);
        Err(Error::new(ErrorKind::InvalidData, msg))
    } else {
        Ok((res, comments))
    }
}

#[allow(dead_code)]
pub fn parse_threshold_file<R: Read>(data: &mut BufReader<R>) -> Result<Vec<Threshold>, Error> {
    let mut res = Vec::new();

    let reader = Csv::from_reader(data)
        .delimiter(b'\t')
        .flexible(true)
        .has_header(false);
    for (i, row) in reader.enumerate() {
        let row = row.unwrap();
        let mut row_it = row.bytes_columns();
        if let Some(col) = row_it.next() {
            let threshold_str = str::from_utf8(col).unwrap();
            if let Ok(t) = usize::from_str(threshold_str) {
                res.push(Threshold::Absolute(t));
            } else if let Ok(t) = f64::from_str(threshold_str) {
                res.push(Threshold::Relative(t));
            } else {
                return Err(Error::new(
                    ErrorKind::InvalidData,
                    &format!(
                        "threshold \"{}\" (line {}) is neither an integer nor a float",
                        &threshold_str,
                        i + 1
                    )[..],
                ));
            }
        }
    }

    Ok(res)
}

pub fn parse_walk_identifier(data: &[u8]) -> (PathSegment, &[u8]) {
    let mut six_col: Vec<&str> = Vec::with_capacity(6);

    let mut it = data.iter();
    let mut i = 0;
    for _ in 0..6 {
        let j = it.position(|x| x == &b'\t').unwrap();
        six_col.push(str::from_utf8(&data[i..i + j]).unwrap());
        i += j + 1;
    }

    let seq_start = match six_col[4] {
        "*" => None,
        a => Some(usize::from_str(a).unwrap()),
    };

    let seq_end = match six_col[5] {
        "*" => None,
        a => Some(usize::from_str(a).unwrap()),
    };

    let path_seg = PathSegment::new(
        six_col[1].to_string(),
        six_col[2].to_string(),
        six_col[3].to_string(),
        seq_start,
        seq_end,
    );

    (path_seg, &data[i..])
}

pub fn parse_path_identifier(data: &[u8]) -> (PathSegment, &[u8]) {
    let mut iter = data.iter();

    let start = iter.position(|&x| x == b'\t').unwrap() + 1;
    let offset = iter.position(|&x| x == b'\t').unwrap();
    let path_name = str::from_utf8(&data[start..start + offset]).unwrap();
    (
        PathSegment::from_str(path_name),
        &data[start + offset + 1..],
    )
}

fn parse_walk_seq_to_item_vec(
    data: &[u8],
    graph_aux: &GraphAuxilliary,
) -> Vec<(ItemId, Orientation)> {
    // later codes assumes that data is non-empty...
    if data.is_empty() {
        return Vec::new();
    }

    // whatever the orientation of the first node is, will be used to split the sequence first;
    // this ensures that the first split results in an empty sequence at the beginning
    let s1 = Orientation::from_lg(data[0]);
    let s2 = s1.flip();

    let mut it = data.iter();
    let end = it
        .position(|x| x == &b'\t' || x == &b'\n' || x == &b'\r')
        .unwrap_or_else(|| it.len());

    log::debug!("parsing walk sequences of size {}..", end);

    // ignore first > | < so that no empty is created for 1st node
    let sids: Vec<(ItemId, Orientation)> = data[..end]
        .par_split(|x| &s1 == x)
        .map(|x| {
            if x.is_empty() {
                // not nice... but Rust expects struct `std::iter::Once<(ItemIdSize, util::Orientation)>`
                //
                // this case shouldn't occur too often, so should be fine in terms for runtime
                vec![]
            } else {
                let i = x.iter().position(|z| &s2 == z).unwrap_or(x.len());
                let sid = (
                    *graph_aux.node2id.get(&x[..i]).unwrap_or_else(|| {
                        panic!(
                            "walk contains unknown node {{{}}}'",
                            str::from_utf8(&x[..i]).unwrap()
                        )
                    }),
                    s1,
                );
                if i < x.len() {
                    // not nice... but Rust expects struct `std::iter::Once<(ItemIdSize, util::Orientation)>`
                    //
                    // this case can happen more frequently... hopefully it doesn't blow up the
                    // runtime
                    [sid]
                        .into_par_iter()
                        .chain(
                            x[i + 1..]
                                .par_split(|y| &s2 == y)
                                .map(|y| {
                                    if y.is_empty() {
                                        vec![]
                                    } else {
                                        vec![(
                                            *graph_aux.node2id.get(y).unwrap_or_else(|| {
                                                panic!(
                                                    "walk contains unknown node {{{}}}",
                                                    str::from_utf8(y).unwrap()
                                                )
                                            }),
                                            s2,
                                        )]
                                    }
                                })
                                .flatten(),
                        )
                        .collect()
                } else {
                    vec![sid]
                }
            }
        })
        .flatten()
        .collect();
    log::debug!("..done");
    sids
}

fn parse_walk_seq_update_tables(
    data: &[u8],
    graph_aux: &GraphAuxilliary,
    item_table: &mut ItemTable,
    exclude_table: Option<&mut ActiveTable>,
    num_path: usize,
) -> (u32, u32) {
    // later codes assumes that data is non-empty...
    if data.is_empty() {
        return (0, 0);
    }

    let items_ptr = Wrap(&mut item_table.items);
    let id_prefsum_ptr = Wrap(&mut item_table.id_prefsum);

    let mutex_vec: Vec<_> = item_table
        .items
        .iter()
        .map(|x| Arc::new(Mutex::new(x)))
        .collect();

    let mut it = data.iter();
    let end = it
        .position(|x| x == &b'\t' || x == &b'\n' || x == &b'\r')
        .unwrap();

    log::debug!("parsing walk sequences of size {}..", end);

    let bp_len = Arc::new(AtomicU32::new(0));
    // ignore first > | < so that no empty is created for 1st node
    data[1..end]
        .par_split(|&x| x == b'>' || x == b'<')
        .for_each(|node| {
            let sid = *graph_aux
                .node2id
                .get(node)
                .unwrap_or_else(|| panic!("unknown node {}", str::from_utf8(node).unwrap()));
            let idx = (sid.0 as usize) % SIZE_T;
            if let Ok(_) = mutex_vec[idx].lock() {
                unsafe {
                    (*items_ptr.0)[idx].push(sid.0);
                    (*id_prefsum_ptr.0)[idx][num_path + 1] += 1;
                }
            }
            bp_len.fetch_add(graph_aux.node_len(&sid), Ordering::SeqCst);
        });
    let bp_len = bp_len.load(Ordering::SeqCst);

    // compute prefix sum
    let mut num_nodes_path = 0;
    for i in 0..SIZE_T {
        num_nodes_path += item_table.id_prefsum[i][num_path + 1];
        item_table.id_prefsum[i][num_path + 1] += item_table.id_prefsum[i][num_path];
    }

    // is exclude table is given, we assume that all nodes of the path are excluded
    if let Some(ex) = exclude_table {
        log::error!("flagging nodes of path as excluded");
        for i in 0..SIZE_T {
            for j in (item_table.id_prefsum[i][num_path] as usize)
                ..(item_table.id_prefsum[i][num_path + 1] as usize)
            {
                ex.items[item_table.items[i][j] as usize] |= true;
            }
        }
    }

    log::debug!("..done");
    (num_nodes_path as u32, bp_len)
}

fn parse_path_seq_to_item_vec(
    data: &[u8],
    graph_aux: &GraphAuxilliary,
) -> Vec<(ItemId, Orientation)> {
    let mut it = data.iter();
    let end = it
        .position(|x| x == &b'\t' || x == &b'\n' || x == &b'\r')
        .unwrap();

    log::debug!("parsing path sequences of size {}..", end);

    let sids: Vec<(ItemId, Orientation)> = data[..end]
        .par_split(|&x| x == b',')
        .map(|node| {
            // Parallel
            let sid = *graph_aux
                .node2id
                .get(&node[..node.len() - 1])
                .unwrap_or_else(|| {
                    panic!(
                        "unknown node {}",
                        str::from_utf8(&node[..node.len() - 1]).unwrap()
                    )
                });
            (sid, Orientation::from_pm(node[node.len() - 1]))
        })
        .collect();

    log::debug!("..done");

    sids
}

fn parse_path_seq_update_tables(
    data: &[u8],
    graph_aux: &GraphAuxilliary,
    item_table: &mut ItemTable,
    exclude_table: Option<&mut ActiveTable>,
    num_path: usize,
) -> (u32, u32) {
    let mut it = data.iter();
    let end = it
        .position(|x| x == &b'\t' || x == &b'\n' || x == &b'\r')
        .unwrap();

    log::debug!("parsing path sequences of size {} bytes..", end);

    let items_ptr = Wrap(&mut item_table.items);
    let id_prefsum_ptr = Wrap(&mut item_table.id_prefsum);

    let mutex_vec: Vec<_> = item_table
        .items
        .iter()
        .map(|x| Arc::new(Mutex::new(x)))
        .collect();

    let bp_len = Arc::new(AtomicU32::new(0));
    //let mut plus_strands: Vec<u32> = vec![0; rayon::current_num_threads()];
    data[..end].par_split(|&x| x == b',').for_each(|node| {
        let sid = *graph_aux
            .node2id
            .get(&node[0..node.len() - 1])
            .unwrap_or_else(|| panic!("unknown node {}", str::from_utf8(node).unwrap()));
        let o = node[node.len() - 1];
        assert!(
            o == b'-' || o == b'+',
            "unknown orientation of segment {}",
            str::from_utf8(node).unwrap()
        );
        //plus_strands[rayon::current_thread_index().unwrap()] += (o == b'+') as u32;

        let idx = (sid.0 as usize) % SIZE_T;

        if let Ok(_) = mutex_vec[idx].lock() {
            unsafe {
                (*items_ptr.0)[idx].push(sid.0);
                (*id_prefsum_ptr.0)[idx][num_path + 1] += 1;
            }
        }
        bp_len.fetch_add(graph_aux.node_len(&sid), Ordering::SeqCst);
    });
    let bp_len = bp_len.load(Ordering::SeqCst);

    // compute prefix sum
    let mut num_nodes_path = 0;
    for i in 0..SIZE_T {
        num_nodes_path += item_table.id_prefsum[i][num_path + 1];
        item_table.id_prefsum[i][num_path + 1] += item_table.id_prefsum[i][num_path];
    }

    // is exclude table is given, we assume that all nodes of the path are excluded
    if let Some(ex) = exclude_table {
        log::debug!("flagging nodes of path as excluded");
        for i in 0..SIZE_T {
            for j in (item_table.id_prefsum[i][num_path] as usize)
                ..(item_table.id_prefsum[i][num_path + 1] as usize)
            {
                ex.items[item_table.items[i][j] as usize] |= true;
            }
        }
    }

    log::debug!("..done");
    (num_nodes_path as u32, bp_len)
}

#[allow(dead_code)]
pub fn parse_graph_aux<R: Read>(
    data: &mut BufReader<R>,
    index_edges: bool,
) -> Result<
    (
        HashMap<Vec<u8>, ItemId>,
        Vec<ItemIdSize>,
        Option<Vec<Vec<u8>>>,
        Vec<PathSegment>,
    ),
    std::io::Error,
> {
    // let's start
    // IMPORTANT: id must be > 0, otherwise counting procedure will produce errors
    let mut node_id = 1;
    let mut node2id: HashMap<Vec<u8>, ItemId> = HashMap::default();
    let mut edges: Option<Vec<Vec<u8>>> = if index_edges { Some(Vec::new()) } else { None };
    let mut path_segments: Vec<PathSegment> = Vec::new();
    let mut node_len: Vec<ItemIdSize> = Vec::new();
    // add empty element to node_len to make it in sync with node_id
    node_len.push(ItemIdSize::MAX);

    let mut buf = vec![];
    let mut i = 1;
    while data.read_until(b'\n', &mut buf).unwrap_or(0) > 0 {
        // really really make sure that we hit a new line, which is not guaranteed when reading
        // from a compressed buffer
        while buf.last().unwrap() != &b'\n' {
            if data.read_until(b'\n', &mut buf).unwrap_or(0) == 0
                && buf.last().unwrap_or(&b' ') != &b'\n'
            {
                buf.push(b'\n')
            }
        }
        if buf[0] == b'S' {
            let mut iter = buf[2..].iter();
            let offset = iter.position(|&x| x == b'\t').ok_or_else(|| {
                std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!(
                        "segment in line {} stops prematurely before declaration of identifier: {}",
                        i,
                        str::from_utf8(&buf).unwrap()
                    ),
                )
            })?;
            if node2id
                .insert(buf[2..offset + 2].to_vec(), ItemId(node_id))
                .is_some()
            {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!(
                        "segment with ID {} occurs multiple times in GFA",
                        str::from_utf8(&buf[2..offset + 2]).unwrap()
                    ),
                ));
            }
            node_id += 1;
            let offset = iter
                .position(|&x| x == b'\t' || x == b'\n' || x == b'\r')
                .unwrap();
            node_len.push(offset as ItemIdSize);
        } else if index_edges && buf[0] == b'L' {
            edges.as_mut().unwrap().push(buf.to_vec());
        } else if buf[0] == b'P' {
            let (path_seg, _) = parse_path_identifier(&buf);
            path_segments.push(path_seg);
        } else if buf[0] == b'W' {
            let (path_seg, _) = parse_walk_identifier(&buf);
            path_segments.push(path_seg);
        }

        buf.clear();
        i += 1;
    }

    Ok((node2id, node_len, edges, path_segments))
}

// pub fn parse_cdbg_gfa_paths_walks<R: Read>(
//     data: &mut BufReader<R>,
//     abacus_aux: &AbacusAuxilliary,
//     graph_aux: &GraphAuxilliary,
//     k: usize,
// ) -> ItemTable {
//     let mut item_table = ItemTable::new(graph_aux.path_segments.len());
//     //let mut k_count = 0;
//     //let (mut subset_covered_bps, mut exclude_table, include_map, exclude_map) = abacus_aux.load_optional_subsetting(&graph_aux, &count);
//
//     let mut num_path = 0;
//     let mut buf = vec![];
//     while data.read_until(b'\n', &mut buf).unwrap_or(0) > 0 {
//         if buf[0] == b'P' {
//             let (_path_seg, buf_path_seg) = parse_path_identifier(&buf);
//             let sids = parse_path_seq_to_item_vec(&buf_path_seg, &graph_aux);
//             let mut u_sid = sids[0].0 .0 as usize - 1;
//             let mut u_ori = sids[0].1;
//             for i in 1..sids.len() {
//                 let v_sid = sids[i].0 .0 as usize - 1;
//                 let v_ori = sids[i].1;
//                 let k_plus_one_mer =
//                     graph_aux.get_k_plus_one_mer_edge(u_sid, u_ori, v_sid, v_ori, k);
//                 //println!("{}", bits2kmer(k_plus_one_mer, k+1));
//                 let infix = get_infix(k_plus_one_mer, k);
//                 let infix_rc = revcmp(infix, k - 1);
//                 if infix < infix_rc {
//                     let idx = (infix as usize) % SIZE_T;
//                     item_table.items[idx].push(k_plus_one_mer);
//                     item_table.id_prefsum[idx][num_path + 1] += 1;
//                 } else if infix > infix_rc {
//                     let idx = (infix_rc as usize) % SIZE_T;
//                     item_table.items[idx].push(revcmp(k_plus_one_mer, k + 1));
//                     item_table.id_prefsum[idx][num_path + 1] += 1;
//                 } // else ignore palindrome, since it always breaks the node
//
//                 u_sid = v_sid;
//                 u_ori = v_ori;
//             }
//
//             // compute prefix sum
//             for i in 0..SIZE_T {
//                 item_table.id_prefsum[i][num_path + 1] += item_table.id_prefsum[i][num_path];
//             }
//
//             num_path += 1;
//         }
//         buf.clear();
//     }
//
//     item_table
// }

pub fn parse_gfa_paths_walks<R: Read>(
    data: &mut BufReader<R>,
    abacus_aux: &AbacusAuxilliary,
    graph_aux: &GraphAuxilliary,
    count: &CountType,
) -> (
    ItemTable,
    Option<ActiveTable>,
    Option<IntervalContainer>,
    HashMap<PathSegment, (u32, u32)>,
) {
    log::info!("parsing path + walk sequences");
    let mut item_table = ItemTable::new(graph_aux.path_segments.len());
    let (mut subset_covered_bps, mut exclude_table, include_map, exclude_map) =
        abacus_aux.load_optional_subsetting(graph_aux, count);

    let mut num_path = 0;
    let complete: Vec<(usize, usize)> = vec![(0, usize::MAX)];
    let mut paths_len: HashMap<PathSegment, (u32, u32)> = HashMap::new();

    let mut buf = vec![];
    while data.read_until(b'\n', &mut buf).unwrap_or(0) > 0 {
        if buf[0] == b'P' || buf[0] == b'W' {
            let (path_seg, buf_path_seg) = match buf[0] {
                b'P' => parse_path_identifier(&buf),
                b'W' => parse_walk_identifier(&buf),
                _ => unreachable!(),
            };

            log::debug!("processing path {}", &path_seg);

            let include_coords = if abacus_aux.include_coords.is_none() {
                &complete[..]
            } else {
                match include_map.get(&path_seg.id()) {
                    None => &[],
                    Some(coords) => {
                        log::debug!(
                            "found include coords in interval {}..{} for path segment {}",
                            &coords.first().unwrap().0,
                            &coords.last().unwrap().1,
                            &path_seg.id()
                        );
                        &coords[..]
                    }
                }
            };
            let exclude_coords = if abacus_aux.exclude_coords.is_none() {
                &[]
            } else {
                match exclude_map.get(&path_seg.id()) {
                    None => &[],
                    Some(coords) => {
                        log::debug!(
                            "found exclude coords in interval {}..{} for path segment {}",
                            &coords.first().unwrap().0,
                            &coords.last().unwrap().1,
                            &path_seg.id()
                        );
                        &coords[..]
                    }
                }
            };

            let (start, end) = path_seg.coords().unwrap_or((0, usize::MAX));

            // do not process the path sequence if path is neither part of subset nor exclude
            if abacus_aux.include_coords.is_some()
                && !intersects(include_coords, &(start, end))
                && !intersects(exclude_coords, &(start, end))
            {
                log::debug!("path {} does not intersect with subset coordinates in interval {}..{} nor with exclude coordinates {}..{}  and therefore is skipped from processing", 
                    &path_seg, &include_coords.first().unwrap_or(&(0,0)).0, &include_coords.last().unwrap_or(&(0,0)).1, &exclude_coords.first().unwrap_or(&(0,0)).0, &exclude_coords.last().unwrap_or(&(0,0)).1);

                // update prefix sum
                for i in 0..SIZE_T {
                    item_table.id_prefsum[i][num_path + 1] += item_table.id_prefsum[i][num_path];
                }

                num_path += 1;
                buf.clear();
                continue;
            }

            if count != &CountType::Edge
                && (abacus_aux.include_coords.is_none()
                    || is_contained(include_coords, &(start, end)))
                && (abacus_aux.exclude_coords.is_none()
                    || is_contained(exclude_coords, &(start, end)))
            {
                log::debug!("path {} is fully contained within subset coordinates {:?} and is eligible for full parallel processing", path_seg, include_coords);
                let ex = if exclude_coords.is_empty() {
                    None
                } else {
                    exclude_table.as_mut()
                };

                let (num_added_nodes, bp_len) = match buf[0] {
                    b'P' => parse_path_seq_update_tables(
                        buf_path_seg,
                        graph_aux,
                        &mut item_table,
                        ex,
                        num_path,
                    ),
                    b'W' => parse_walk_seq_update_tables(
                        buf_path_seg,
                        graph_aux,
                        &mut item_table,
                        ex,
                        num_path,
                    ),
                    _ => unreachable!(),
                };
                paths_len.insert(path_seg, (num_added_nodes, bp_len));
            } else {
                let sids = match buf[0] {
                    b'P' => parse_path_seq_to_item_vec(buf_path_seg, graph_aux),
                    b'W' => parse_walk_seq_to_item_vec(buf_path_seg, graph_aux),
                    _ => unreachable!(),
                };

                match count {
                    CountType::Node | CountType::Bp => {
                        let (node_len, bp_len) = update_tables(
                            &mut item_table,
                            &mut subset_covered_bps.as_mut(),
                            &mut exclude_table.as_mut(),
                            num_path,
                            graph_aux,
                            sids,
                            include_coords,
                            exclude_coords,
                            start,
                        );
                        paths_len.insert(path_seg, (node_len as u32, bp_len as u32));
                    }
                    CountType::Edge => update_tables_edgecount(
                        &mut item_table,
                        &mut exclude_table.as_mut(),
                        num_path,
                        graph_aux,
                        sids,
                        include_coords,
                        exclude_coords,
                        start,
                    ),
                    CountType::All => unreachable!("inadmissable count type"),
                };
            }
            num_path += 1;
        }
        buf.clear();
    }
    (item_table, exclude_table, subset_covered_bps, paths_len)
}

fn update_tables(
    item_table: &mut ItemTable,
    subset_covered_bps: &mut Option<&mut IntervalContainer>,
    exclude_table: &mut Option<&mut ActiveTable>,
    num_path: usize,
    graph_aux: &GraphAuxilliary,
    path: Vec<(ItemId, Orientation)>,
    include_coords: &[(usize, usize)],
    exclude_coords: &[(usize, usize)],
    offset: usize,
) -> (usize, usize) {
    let mut i = 0;
    let mut j = 0;
    let mut p = offset;

    let mut included = 0;
    let mut included_bp = 0;
    let mut excluded = 0;

    log::debug!(
        "checking inclusion/exclusion criteria on {} nodes..",
        path.len()
    );
    if path.len() == 0 {
        return (included, included_bp);
    }

    for (sid, o) in &path {
        let l = graph_aux.node_len(&sid) as usize;

        // this implementation of include coords for bps is *not exact* as illustrated by the
        // following scenario:
        //
        //   subset intervals:           ____________________________
        //                ______________|_____________________________
        //               |
        //      ___________________________________________     ____
        //  ---|                some node                  |---|
        //      -------------------------------------------     ----
        //
        //
        //   what the following code does:
        //                ___________________________________________
        //               |
        //               |             coverage count
        //      ___________________________________________     ____
        //  ---|                some node                  |---|
        //      -------------------------------------------     ----
        //
        //
        // node count handling: node is only counted if *completely* covered by subset
        //
        //
        // update current pointer in include_coords list

        // end is not inclusive, so if end <= p (=offset) then advance to the next interval
        let mut stop_here = false;
        while i < include_coords.len() && include_coords[i].0 < p + l && !stop_here {
            if include_coords[i].1 > p {
                let mut a = if include_coords[i].0 > p {
                    include_coords[i].0 - p
                } else {
                    0
                };
                let mut b = if include_coords[i].1 < p + l {
                    // advance to the next interval
                    i += 1;
                    include_coords[i - 1].1 - p
                } else {
                    stop_here = true;
                    l
                };

                // reverse coverage interval in case of backward orientation
                if o == &Orientation::Backward {
                    (a, b) = (l - b, l - a);
                }

                let idx = (sid.0 as usize) % SIZE_T;
                item_table.items[idx].push(sid.0);
                item_table.id_prefsum[idx][num_path + 1] += 1;
                if let Some(int) = subset_covered_bps.as_mut() {
                    // if fully covered, we do not need to store anything in the map
                    if b - a == l {
                        if int.contains(sid) {
                            int.remove(sid);
                        }
                    } else {
                        int.add(*sid, a, b);
                    }
                }
                included += 1;
                included_bp += b - a;
            } else {
                // advance to the next interval
                i += 1;
            }
        }

        let mut stop_here = false;
        while j < exclude_coords.len() && exclude_coords[j].0 < p + l && !stop_here {
            if exclude_coords[j].1 > p {
                let mut a = if exclude_coords[j].0 > p {
                    exclude_coords[j].0 - p
                } else {
                    0
                };
                let mut b = if exclude_coords[j].1 < p + l {
                    // advance to the next interval for the next iteration
                    j += 1;
                    exclude_coords[j - 1].1 - p
                } else {
                    stop_here = true;
                    l
                };

                // reverse coverage interval in case of backward orientation
                if o == &Orientation::Backward {
                    (a, b) = (l - b, l - a);
                }

                if let Some(map) = exclude_table {
                    if map.with_annotation() {
                        map.activate_n_annotate(*sid, l, a, b)
                            .expect("this error should never occur");
                    } else {
                        map.activate(&sid);
                    }
                    excluded += 1;
                }
            } else {
                j += 1;
            }
        }

        if i >= include_coords.len() && j >= exclude_coords.len() {
            // terminate parse if all "include" and "exclude" coords are processed
            break;
        }
        p += l;
    }

    log::debug!(
        "found {} included nodes ({} included bps) and {} excluded nodes, and discarded the rest",
        included,
        included_bp,
        excluded,
    );

    // Compute prefix sum
    for i in 0..SIZE_T {
        item_table.id_prefsum[i][num_path + 1] += item_table.id_prefsum[i][num_path];
    }
    log::debug!("..done");
    (included, included_bp)
}

fn update_tables_edgecount(
    item_table: &mut ItemTable,
    exclude_table: &mut Option<&mut ActiveTable>,
    num_path: usize,
    graph_aux: &GraphAuxilliary,
    path: Vec<(ItemId, Orientation)>,
    include_coords: &[(usize, usize)],
    exclude_coords: &[(usize, usize)],
    offset: usize,
) {
    let mut i = 0;
    let mut j = 0;
    let mut p = offset;

    // edges are positioned between nodes, offset by the first node
    if !path.is_empty() {
        p += graph_aux.node_len(&path[0].0) as usize;
    }

    log::debug!("checking inclusion/exclusion criteria on {} nodes, inserting successful candidates to corresponding data structures..", path.len());

    for ((sid1, o1), (sid2, o2)) in path.into_iter().tuple_windows() {
        // update current pointer in include_coords list
        while i < include_coords.len() && include_coords[i].1 <= p {
            i += 1;
        }

        // update current pointer in exclude_coords list
        while j < exclude_coords.len() && exclude_coords[j].1 <= p {
            j += 1;
        }

        let l = graph_aux.node_len(&sid2) as usize;

        let e = Edge::canonical(sid1, o1, sid2, o2);
        let eid = graph_aux
            .edge2id
            .as_ref()
            .expect("update_tables_edgecount requires edge2id map in GraphAuxilliary")
            .get(&e)
            .unwrap_or_else(|| {
                panic!(
                    "unknown edge {}. Is flipped edge known? {}",
                    &e,
                    if graph_aux.edge2id.as_ref().unwrap().contains_key(&e.flip()) {
                        "Yes"
                    } else {
                        "No"
                    }
                )
            });
        // check if the current position fits within active segment
        if i < include_coords.len() && include_coords[i].0 < p + l {
            let idx = (eid.0 as usize) % SIZE_T;
            item_table.items[idx].push(eid.0);
            item_table.id_prefsum[idx][num_path + 1] += 1;
        }
        if exclude_table.is_some() && j < exclude_coords.len() && exclude_coords[j].0 < p + l {
            exclude_table.as_mut().unwrap().activate(eid);
        } else if i >= include_coords.len() && j >= exclude_coords.len() {
            // terminate parse if all "include" and "exclude" coords are processed
            break;
        }
        p += l;
    }
    // Compute prefix sum
    for i in 0..SIZE_T {
        item_table.id_prefsum[i][num_path + 1] += item_table.id_prefsum[i][num_path];
    }
    log::debug!("..done");
}

pub fn write_table<W: Write>(
    headers: &Vec<Vec<String>>,
    columns: &Vec<Vec<f64>>,
    out: &mut BufWriter<W>,
) -> Result<(), Error> {
    let n = headers.first().unwrap_or(&Vec::new()).len();

    for i in 0..n {
        for j in 0..headers.len() {
            if j > 0 {
                write!(out, "\t")?;
            }
            write!(out, "{:0}", headers[j][i])?;
        }
        writeln!(out)?;
    }
    let n = columns.first().unwrap_or(&Vec::new()).len();
    for i in 0..n {
        write!(out, "{}", i)?;
        for j in 0..columns.len() {
            write!(out, "\t{:0}", columns[j][i].floor())?;
        }
        writeln!(out)?;
    }

    Ok(())
}

pub fn write_ordered_table<W: Write>(
    headers: &Vec<Vec<String>>,
    columns: &Vec<Vec<f64>>,
    index: &Vec<String>,
    out: &mut BufWriter<W>,
) -> Result<(), std::io::Error> {
    let n = headers.first().unwrap_or(&Vec::new()).len();

    for i in 0..n {
        for j in 0..headers.len() {
            if j > 0 {
                write!(out, "\t")?;
            }
            write!(out, "{:0}", headers[j][i])?;
        }
        writeln!(out)?;
    }
    let n = columns.first().unwrap_or(&Vec::new()).len();
    for i in 1..n {
        write!(out, "{}", index[i - 1])?;
        for column in columns {
            write!(out, "\t{:0}", column[i].floor())?;
        }
        writeln!(out)?;
    }

    Ok(())
}

pub fn write_hist_table<W: Write>(hists: &[Hist], out: &mut BufWriter<W>) -> Result<(), Error> {
    log::info!("reporting hist table");
    writeln!(
        out,
        "# {}",
        std::env::args().collect::<Vec<String>>().join(" ")
    )?;

    let mut header_cols = vec![vec![
        "panacus".to_string(),
        "count".to_string(),
        String::new(),
        String::new(),
    ]];
    let mut output_columns = Vec::new();
    for h in hists.iter() {
        output_columns.push(h.coverage.iter().map(|x| *x as f64).collect());
        header_cols.push(vec![
            "hist".to_string(),
            h.count.to_string(),
            String::new(),
            String::new(),
        ])
    }
    write_table(&header_cols, &output_columns, out)
}

pub fn write_histgrowth_table<W: Write>(
    hists: &[Hist],
    growths: &Vec<(CountType, Vec<Vec<f64>>)>,
    hist_aux: &HistAuxilliary,
    out: &mut BufWriter<W>,
) -> Result<(), Error> {
    writeln!(
        out,
        "# {}",
        std::env::args().collect::<Vec<String>>().join(" ")
    )?;

    let mut header_cols = vec![vec![
        "panacus".to_string(),
        "count".to_string(),
        "coverage".to_string(),
        "quorum".to_string(),
    ]];
    let mut output_columns: Vec<Vec<f64>> = Vec::new();

    for h in hists.iter() {
        output_columns.push(h.coverage.iter().map(|x| *x as f64).collect());
        header_cols.push(vec![
            "hist".to_string(),
            h.count.to_string(),
            String::new(),
            String::new(),
        ])
    }

    for (count, g) in growths {
        output_columns.extend(g.clone());
        let m = hist_aux.coverage.len();
        header_cols.extend(
            std::iter::repeat("growth")
                .take(m)
                .zip(std::iter::repeat(count).take(m))
                .zip(hist_aux.coverage.iter())
                .zip(&hist_aux.quorum)
                .map(|(((p, t), c), q)| {
                    vec![p.to_string(), t.to_string(), c.get_string(), q.get_string()]
                }),
        );
    }
    write_table(&header_cols, &output_columns, out)
}

pub fn write_info<W: Write>(info: Info, out: &mut BufWriter<W>) -> Result<(), Error> {
    log::info!("reporting graph info table");
    writeln!(
        out,
        "# {}",
        std::env::args().collect::<Vec<String>>().join(" ")
    )?;
    writeln!(out, "{}", info)
}

pub fn write_ordered_histgrowth_table<W: Write>(
    abacus_group: &AbacusByGroup,
    hist_aux: &HistAuxilliary,
    out: &mut BufWriter<W>,
) -> Result<(), Error> {
    log::info!("reporting ordered-growth table");
    writeln!(
        out,
        "# {}",
        std::env::args().collect::<Vec<String>>().join(" ")
    )?;

    let mut output_columns: Vec<Vec<f64>> = hist_aux
        .coverage
        .par_iter()
        .zip(&hist_aux.quorum)
        .map(|(c, q)| {
            log::info!(
                "calculating ordered growth for coverage >= {} and quorum >= {}",
                &c,
                &q
            );
            abacus_group.calc_growth(c, q)
        })
        .collect();

    // insert empty row for 0 element
    for c in &mut output_columns {
        c.insert(0, f64::NAN);
    }
    let m = hist_aux.coverage.len();
    let mut header_cols = vec![vec![
        "panacus".to_string(),
        "count".to_string(),
        "coverage".to_string(),
        "quorum".to_string(),
    ]];
    header_cols.extend(
        std::iter::repeat("ordered-growth")
            .take(m)
            .zip(std::iter::repeat(abacus_group.count).take(m))
            .zip(hist_aux.coverage.iter())
            .zip(&hist_aux.quorum)
            .map(|(((p, t), c), q)| {
                vec![p.to_string(), t.to_string(), c.get_string(), q.get_string()]
            })
            .collect::<Vec<Vec<String>>>(),
    );
    write_ordered_table(&header_cols, &output_columns, &abacus_group.groups, out)
}

pub fn write_ordered_histgrowth_html<W: Write>(
    abacus_group: &AbacusByGroup,
    hist_aux: &HistAuxilliary,
    gfa_file: &str,
    count: CountType,
    info: Option<Info>,
    out: &mut BufWriter<W>,
) -> Result<(), Error> {
    let mut growths: Vec<Vec<f64>> = hist_aux
        .coverage
        .par_iter()
        .zip(&hist_aux.quorum)
        .map(|(c, q)| {
            log::info!(
                "calculating ordered growth for coverage >= {} and quorum >= {}",
                &c,
                &q
            );
            abacus_group.calc_growth(c, q)
        })
        .collect();
    // insert empty row for 0 element
    for c in &mut growths {
        c.insert(0, f64::NAN);
    }
    log::info!("reporting (hist-)growth table");

    write_histgrowth_html(
        &None,
        &[(count, growths)],
        hist_aux,
        Path::new(gfa_file).file_name().unwrap().to_str().unwrap(),
        Some(&abacus_group.groups),
        info,
        out,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::io::Cursor;
    use std::str::from_utf8;

    fn mock_graph_auxilliary() -> GraphAuxilliary {
        GraphAuxilliary {
            node2id: {
                let mut node2id = HashMap::new();
                node2id.insert(b"node1".to_vec(), ItemId(1));
                node2id.insert(b"node2".to_vec(), ItemId(2));
                node2id.insert(b"node3".to_vec(), ItemId(3));
                node2id
            },
            node_lens: Vec::new(),
            edge2id: None,
            path_segments: Vec::new(),
            node_count: 3,
            edge_count: 0,
            degree: Some(Vec::new()),
            //extremities: Some(Vec::new())
        }
    }

    // Test parse_walk_identifier function
    #[test]
    fn test_parse_walk_identifier() {
        let data = b"W\tG01\t0\tU00096.3\t3\t4641652\t>3>4>5>7>8>";
        let (path_segment, data) = parse_walk_identifier(data);
        dbg!(&path_segment);

        assert_eq!(path_segment.sample, "G01".to_string());
        assert_eq!(path_segment.haplotype, Some("0".to_string()));
        assert_eq!(path_segment.seqid, Some("U00096.3".to_string()));
        assert_eq!(path_segment.start, Some(3));
        assert_eq!(path_segment.end, Some(4641652));
        assert_eq!(from_utf8(data).unwrap(), ">3>4>5>7>8>");
    }

    #[test]
    #[should_panic(expected = "unwrap")]
    fn test_parse_walk_identifier_invalid_utf8() {
        let data = b"W\tG01\t0\tU00096.3\t3\t>3>4>5>7>8>";
        parse_walk_identifier(data);
    }

    // Test parse_path_identifier function
    #[test]
    fn test_parse_path_identifier() {
        let data = b"P\tGCF_000005845.2_ASM584v2_genomic.fna#0#contig1\t1+,2+,3+,4+\t*";
        let (path_segment, rest) = parse_path_identifier(data);

        assert_eq!(
            path_segment.to_string(),
            "GCF_000005845.2_ASM584v2_genomic.fna#0#contig1"
        );
        assert_eq!(from_utf8(rest).unwrap(), "1+,2+,3+,4+\t*");
    }

    //// Test parse_walk_seq_to_item_vec function
    //#[test]
    //fn test_parse_walk_seq_to_item_vec() {
    //    let data = b">node1<node2\t";
    //    let graph_aux = MockGraphAuxilliary::new();

    //    let result = parse_walk_seq_to_item_vec(data, &graph_aux);
    //    assert_eq!(result.len(), 2);
    //    assert_eq!(result[0], (1, Orientation::Forward));
    //    assert_eq!(result[1], (2, Orientation::Backward));
    //}

    // Test parse_path_seq_to_item_vec function
    #[test]
    fn test_parse_path_seq_to_item_vec() {
        let data = b"node1+,node2-\t*";
        let graph_aux = mock_graph_auxilliary();

        let result = parse_path_seq_to_item_vec(data, &graph_aux);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0], (ItemId(1), Orientation::Forward));
        assert_eq!(result[1], (ItemId(2), Orientation::Backward));
    }

    //#[test]
    //fn test_parse_cdbg_gfa_paths_walks() {
    //    let data = b"P\tpath1\tnode1+,node2-\n";
    //    let mut reader = BufReader::new(&data[..]);
    //    let graph_aux = mock_graph_auxilliary();

    //    let result = parse_cdbg_gfa_paths_walks(&mut reader, &graph_aux, 3);
    //    dbg!(&result);

    //    assert_eq!(result.items.len(), SIZE_T);
    //}

    // Test update_tables
    //#[test]
    //fn test_update_tables() {
    //    let mut item_table = ItemTable::new(10);
    //    let graph_aux = mock_graph_auxilliary();
    //    let path = vec![(1, Orientation::Forward), (2, Orientation::Backward)];
    //    let include_coords = &[];
    //    let exclude_coords = &[];
    //    let offset = 0;

    //    update_tables(
    //        &mut item_table,
    //        &mut None,
    //        &mut None,
    //        0,
    //        &graph_aux,
    //        path,
    //        include_coords,
    //        exclude_coords,
    //        offset,
    //    );

    //    dbg!(&item_table);
    //    assert!(item_table.items[1].contains(&1));
    //    assert!(item_table.items[2].contains(&2));
    //}

    // parse_bed_to_path_segments testing
    #[test]
    fn test_parse_bed_with_1_column() {
        let bed_data = b"chr1\nchr2";
        let mut reader = BufReader::new(Cursor::new(bed_data));
        let result = parse_bed_to_path_segments(&mut reader, true);
        assert_eq!(
            result,
            vec![PathSegment::from_str("chr1"), PathSegment::from_str("chr2"),]
        );
    }

    #[test]
    #[should_panic(
        expected = "error in line 1: row must have either 1, 3, or 12 columns, but has 2"
    )]
    fn test_parse_bed_with_2_columns() {
        let bed_data = b"chr1\t1000\n";
        let mut reader = BufReader::new(Cursor::new(bed_data));
        parse_bed_to_path_segments(&mut reader, false);
    }

    #[test]
    #[should_panic(expected = "error line 1: `100.5` is not an usize")]
    fn test_parse_bed_with_2_columns_no_usize() {
        let bed_data = b"chr1\t100.5\tACGT\n";
        let mut reader = BufReader::new(Cursor::new(bed_data));
        parse_bed_to_path_segments(&mut reader, false);
    }

    #[test]
    fn test_parse_bed_with_3_columns() {
        let bed_data = b"chr1\t1000\t2000\nchr2\t1500\t2500";
        let mut reader = BufReader::new(Cursor::new(bed_data));
        let result = parse_bed_to_path_segments(&mut reader, false);
        assert_eq!(
            result,
            vec![
                {
                    let mut tmp = PathSegment::from_str("chr1");
                    tmp.start = Some(1000);
                    tmp.end = Some(2000);
                    tmp
                },
                {
                    let mut tmp = PathSegment::from_str("chr2");
                    tmp.start = Some(1500);
                    tmp.end = Some(2500);
                    tmp
                }
            ]
        );
    }

    #[test]
    fn test_parse_bed_with_12_columns_no_block() {
        let bed_data = b"chr1\t1000\t2000\tname\t0\t+\t1000\t2000\t0\t2\t100,100\t0,900\n";
        let mut reader = BufReader::new(Cursor::new(bed_data));
        let result = parse_bed_to_path_segments(&mut reader, false);
        assert_eq!(
            result,
            vec![{
                let mut tmp = PathSegment::from_str("chr1");
                tmp.start = Some(1000);
                tmp.end = Some(2000);
                tmp
            }]
        );
    }

    #[test]
    fn test_parse_bed_with_12_columns_with_block() {
        let bed_data = b"chr1\t1000\t2000\tname\t0\t+\t1000\t2000\t0\t2\t100,100\t0,900\n";
        let mut reader = BufReader::new(Cursor::new(bed_data));
        let result = parse_bed_to_path_segments(&mut reader, true);
        assert_eq!(
            result,
            vec![
                {
                    let mut tmp = PathSegment::from_str("chr1");
                    tmp.start = Some(1000);
                    tmp.end = Some(1100);
                    tmp
                },
                {
                    let mut tmp = PathSegment::from_str("chr1");
                    tmp.start = Some(1900);
                    tmp.end = Some(2000);
                    tmp
                }
            ]
        );
    }

    #[test]
    fn test_parse_bed_with_header() {
        let bed_data = b"browser position chr1:1-1000\nbrowser position chr7:127471196-127495720\nbrowser hide all\ntrack name='ItemRGBDemo' description='Item RGB demonstration' visibility=2 itemRgb='On'\nchr1\t1000\t2000\nchr2\t1500\t2500\n";
        let mut reader = BufReader::new(Cursor::new(bed_data));
        let result = parse_bed_to_path_segments(&mut reader, false);
        assert_eq!(
            result,
            vec![
                {
                    let mut tmp = PathSegment::from_str("chr1");
                    tmp.start = Some(1000);
                    tmp.end = Some(2000);
                    tmp
                },
                {
                    let mut tmp = PathSegment::from_str("chr2");
                    tmp.start = Some(1500);
                    tmp.end = Some(2500);
                    tmp
                }
            ]
        );
    }

    #[test]
    fn test_parse_groups_with_valid_input() {
        //let (graph_aux, _, _) = setup_test_data();
        let file_name = "test/test_groups.txt";
        let test_path_segments = vec![
            PathSegment::from_str("a#0"),
            PathSegment::from_str("b#0"),
            PathSegment::from_str("c#0"),
            PathSegment::from_str("c#1"),
            PathSegment::from_str("d#0"),
        ];
        let test_groups = vec!["G1", "G1", "G2", "G2", "G2"];

        let mut data = BufReader::new(std::fs::File::open(file_name).unwrap());
        let result = parse_groups(&mut data);
        assert!(result.is_ok(), "Expected successful group loading");
        let path_segments_group = result.unwrap();
        assert!(
            path_segments_group.len() > 0,
            "Expected non-empty group assignments"
        );
        assert_eq!(path_segments_group.len(), 5); // number of paths == groups
        for (i, (path_seg, group)) in path_segments_group.into_iter().enumerate() {
            assert_eq!(path_seg, test_path_segments[i]);
            assert_eq!(group, test_groups[i]);
        }
    }
}
