/* standard use */
use std::collections::{HashMap, HashSet};
use std::io::{BufRead, BufReader, BufWriter, Read, Write};
use std::iter::FromIterator;
use std::str::{self, FromStr};
use std::sync::{Arc, Mutex};

/* external use */
use itertools::Itertools;
use quick_csv::Csv;
use rayon::prelude::*;
use strum_macros::{EnumString, EnumVariantNames};

/* internal use */
use crate::abacus::*;
use crate::graph::*;
use crate::hist::*;
use crate::util::*;

#[derive(Debug, Clone, Copy, PartialEq, EnumString, EnumVariantNames)]
#[strum(serialize_all = "lowercase")]
pub enum OutputFormat {
    Table,
    Html,
}

pub fn parse_bed<R: Read>(data: &mut BufReader<R>) -> Vec<PathSegment> {
    // based on https://en.wikipedia.org/wiki/BED_(file_format)
    let mut res = Vec::new();

    let reader = Csv::from_reader(data)
        .delimiter(b'\t')
        .flexible(true)
        .has_header(false);
    let mut is_header = true;
    let mut is_full_bed = false;
    for (i, row) in reader.enumerate() {
        let row = row.unwrap();
        let mut row_it = row.bytes_columns();
        let path_name = str::from_utf8(row_it.next().unwrap()).unwrap().to_string();
        // recognize BED header
        if is_header
            && (path_name.starts_with("browser ")
                || path_name.starts_with("track ")
                || path_name.starts_with("#"))
        {
            continue;
        }
        is_header = false;
        let mut path_seg = PathSegment::from_str(&path_name);
        if let Some(start) = row_it.next() {
            if let Some(end) = row_it.next() {
                path_seg.start = usize::from_str(str::from_utf8(start).unwrap()).ok();
                path_seg.end = usize::from_str(str::from_utf8(end).unwrap()).ok();
            } else {
                panic!(
                    "erroneous input in line {}: row must have either 1, 3, or 12 columns, but has 2",
                    i
                );
            }
            if let Some(block_count_raw) = row_it.nth(6) {
                if !is_full_bed {
                    log::debug!("assuming from now (line {}) on that file is in full bed (12 columns) format", i);
                }
                let block_count =
                    usize::from_str(str::from_utf8(block_count_raw).unwrap()).unwrap();
                is_full_bed = true;
                let mut block_sizes = str::from_utf8(row_it.next().unwrap()).unwrap().split(',');
                let mut block_starts = str::from_utf8(row_it.next().unwrap()).unwrap().split(',');
                for _ in 0..block_count {
                    let size = usize::from_str(block_sizes.next().unwrap().trim()).unwrap();
                    let start = usize::from_str(block_starts.next().unwrap().trim()).unwrap();

                    let mut tmp = path_seg.clone();
                    if tmp.start.is_some() {
                        tmp.start = Some(tmp.start.unwrap() + start);
                    } else {
                        tmp.start = Some(start);
                    }
                    tmp.end = Some(start + size);
                    res.push(tmp);
                }
            }
        }
        if !is_full_bed {
            res.push(path_seg);
        }
    }

    res
}

pub fn parse_groups<R: Read>(
    data: &mut BufReader<R>,
) -> Result<Vec<(PathSegment, String)>, std::io::Error> {
    let mut res: Vec<(PathSegment, String)> = Vec::new();

    let reader = Csv::from_reader(data)
        .delimiter(b'\t')
        .flexible(true)
        .has_header(false);
    for (i, row) in reader.enumerate() {
        let row = row.unwrap();
        let mut row_it = row.bytes_columns();
        let path_seg =
            PathSegment::from_str(&str::from_utf8(row_it.next().unwrap()).unwrap().to_string());
        if let Some(col) = row_it.next() {
            res.push((path_seg, str::from_utf8(col).unwrap().to_string()));
        } else {
            let msg = format!("error in line {}: table must have two columns", i);
            log::error!("{}", &msg);
            return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, msg));
        }
    }

    Ok(res)
}

pub fn parse_tsv<R: Read>(
    data: &mut BufReader<R>,
) -> Result<(Vec<Vec<u8>>, Vec<Vec<Vec<u8>>>), std::io::Error> {
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
                std::io::Error::new(std::io::ErrorKind::Other, msg)
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

fn transpose_table<'a>(table: &'a Vec<Vec<Vec<u8>>>) -> Vec<Vec<&'a [u8]>> {
    let n = table.first().unwrap_or(&Vec::new()).len();

    let mut res = vec![vec![&table[0][0][..]; table.len()]; n];

    for j in 0..n {
        for i in 0..table.len() {
            res[j][i] = &table[i][j][..];
        }
    }

    res
}

fn parse_column(col: &Vec<&[u8]>, offset: usize) -> Result<Vec<usize>, std::io::Error> {
    let mut res = vec![0; col.len() - 4];

    for (i, e) in col[4..].iter().enumerate() {
        if let Ok(val) = usize::from_str(&str::from_utf8(e).unwrap()) {
            res[i] = val;
        } else {
            let msg = format!(
                "error in line {}: value must be integer, but is '{}'",
                i + 4 + offset,
                &str::from_utf8(e).unwrap()
            );
            log::error!("{}", &msg);
            Err(std::io::Error::new(std::io::ErrorKind::InvalidData, msg))?
        }
    }

    Ok(res)
}

pub fn parse_hists<R: Read>(
    data: &mut BufReader<R>,
) -> Result<(Vec<(CountType, Vec<usize>)>, Vec<Vec<u8>>), std::io::Error> {
    let (comments, raw_table) = parse_tsv(data)?;
    let raw_table = transpose_table(&raw_table);
    if raw_table.len() < 4 && b"panacus" != &raw_table[0][0][..] {
        let msg = format!(
            "error in line {}: table appears not to be generated by panacus",
            comments.len()
        );
        log::error!("{}", &msg);
        return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, msg));
    }

    let mut res = Vec::new();

    let index = parse_column(&raw_table[0], comments.len())?;
    let mx = index.iter().max().unwrap();
    for col in &raw_table[1..] {
        if b"hist" == &col[0] {
            let count = CountType::from_str(&str::from_utf8(&col[1]).unwrap()).or_else(|_| {
                let msg = format!(
                    "error in line {}: expected count type declaration, but got '{}'",
                    2 + comments.len(),
                    &str::from_utf8(&col[1]).unwrap()
                );
                log::error!("{}", &msg);
                Err(std::io::Error::new(std::io::ErrorKind::InvalidData, msg))
            })?;
            let mut cov = vec![0; mx + 1];
            for (i, c) in index.iter().zip(parse_column(&col, comments.len())?) {
                cov[*i] = c;
            }

            res.push((count, cov));
        }
    }

    if res.is_empty() {
        let msg = "table does not contain hist columns";
        log::error!("{}", msg);
        Err(std::io::Error::new(std::io::ErrorKind::InvalidData, msg))
    } else {
        Ok((res, comments))
    }
}

#[allow(dead_code)]
pub fn parse_threshold_file<R: Read>(
    data: &mut BufReader<R>,
) -> Result<Vec<Threshold>, std::io::Error> {
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
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
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

pub fn parse_walk_identifier<'a>(data: &'a [u8]) -> (PathSegment, &'a [u8]) {
    let mut six_col: Vec<&str> = Vec::with_capacity(6);

    let mut it = data.iter();
    let mut i = 0;
    for _ in 0..6 {
        let j = it.position(|x| x == &b'\t').unwrap();
        six_col.push(&str::from_utf8(&data[i..i + j]).unwrap());
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

pub fn parse_path_identifier<'a>(data: &'a [u8]) -> (PathSegment, &'a [u8]) {
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
        .unwrap();

    log::debug!("parsing path sequences of size {}..", end);

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
                let i = x.iter().position(|z| &s2 == z).unwrap_or_else(|| x.len());
                let sid = (
                    *graph_aux.node2id.get(&x[..i]).expect(&format!(
                        "walk contains unknown node {{{}}}'",
                        str::from_utf8(&x[..i]).unwrap()
                    )),
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
                                    if y.len() == 0 {
                                        vec![]
                                    } else {
                                        vec![(
                                            *graph_aux.node2id.get(&y[..]).expect(&format!(
                                                "walk contains unknown node {{{}}}",
                                                str::from_utf8(&y[..]).unwrap()
                                            )),
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
) {
    // later codes assumes that data is non-empty...
    if data.is_empty() {
        return;
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

    // ignore first > | < so that no empty is created for 1st node
    data[1..end]
        .par_split(|&x| x == b'>' || x == b'<')
        .for_each(|node| {
            let sid = *graph_aux.node2id.get(&node[..]).expect(&format!(
                "unknown node {}",
                &str::from_utf8(node).unwrap()[..]
            ));
            let idx = (sid.0 as usize) % SIZE_T;
            if let Ok(_) = mutex_vec[idx].lock() {
                unsafe {
                    (*items_ptr.0)[idx].push(sid.0);
                    (*id_prefsum_ptr.0)[idx][num_path + 1] += 1;
                }
            }
        });

    // compute prefix sum
    for i in 0..SIZE_T {
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
                .expect(&format!(
                    "unknown node {}",
                    str::from_utf8(&node[..node.len() - 1]).unwrap()
                ));
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
) {
    let mut it = data.iter();
    let end = it
        .position(|x| x == &b'\t' || x == &b'\n' || x == &b'\r')
        .unwrap();

    log::debug!("parsing path sequences of size {}..", end);

    let items_ptr = Wrap(&mut item_table.items);
    let id_prefsum_ptr = Wrap(&mut item_table.id_prefsum);

    let mutex_vec: Vec<_> = item_table
        .items
        .iter()
        .map(|x| Arc::new(Mutex::new(x)))
        .collect();

    data[..end].par_split(|&x| x == b',').for_each(|node| {
        let sid = *graph_aux
            .node2id
            .get(&node[0..node.len() - 1])
            .expect(&format!(
                "unknown node {}",
                &str::from_utf8(node).unwrap()[..]
            ));
        let o = node[node.len() - 1];
        assert!(
            o == b'-' || o == b'+',
            "unknown orientation of segment {}",
            str::from_utf8(&node).unwrap()
        );
        let idx = (sid.0 as usize) % SIZE_T;

        if let Ok(_) = mutex_vec[idx].lock() {
            unsafe {
                (*items_ptr.0)[idx].push(sid.0);
                (*id_prefsum_ptr.0)[idx][num_path + 1] += 1;
            }
        }
    });

    // compute prefix sum
    for i in 0..SIZE_T {
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
}

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

fn build_subpath_map(path_segments: &Vec<PathSegment>) -> HashMap<String, Vec<(usize, usize)>> {
    // intervals are 0-based, and [start, end), see https://en.wikipedia.org/wiki/BED_(file_format)
    let mut res: HashMap<String, HashSet<(usize, usize)>> = HashMap::default();

    path_segments.into_iter().for_each(|x| {
        res.entry(x.id())
            .or_insert(HashSet::default())
            .insert(match x.coords() {
                None => (0, usize::MAX),
                Some((i, j)) => (i, j),
            });
    });

    HashMap::from_iter(res.into_iter().map(|(pid, coords)| {
        let mut v: Vec<(usize, usize)> = coords.into_iter().collect();
        v.sort();
        let mut i = 1;
        // remove overlaps
        while i < v.len() {
            if v[i - 1].1 >= v[i].0 {
                let x = v.remove(i);
                v[i - 1].1 = x.1;
            } else {
                i += 1
            }
        }
        (pid, v)
    }))
}

pub fn parse_gfa_itemcount<R: Read>(
    data: &mut BufReader<R>,
    abacus_aux: &AbacusAuxilliary,
    graph_aux: &GraphAuxilliary,
    count: &CountType,
) -> (ItemTable, Option<ActiveTable>, Option<IntervalContainer>) {
    let mut item_table = ItemTable::new(graph_aux.path_segments.len());

    //
    // *only relevant for bps count in combination with subset option*
    //
    // this table stores the number of bps of nodes that are *partially* uncovered by subset
    // coodinates
    //
    let mut subset_covered_bps: Option<IntervalContainer> =
        if count == &CountType::Bp && abacus_aux.include_coords.is_some() {
            Some(IntervalContainer::new())
        } else {
            None
        };

    //
    // this table stores information about excluded nodes *if* the exclude setting is used
    //
    let mut exclude_table = abacus_aux.exclude_coords.as_ref().map(|_| {
        ActiveTable::new(
            graph_aux.number_of_items(count) + 1,
            count == &CountType::Bp,
        )
    });

    // build "include" lookup table
    let include_map = match &abacus_aux.include_coords {
        None => HashMap::default(),
        Some(coords) => build_subpath_map(coords),
    };

    // build "exclude" lookup table
    let exclude_map = match &abacus_aux.exclude_coords {
        None => HashMap::default(),
        Some(coords) => build_subpath_map(coords),
    };

    // reading GFA file searching for (P)aths and (W)alks
    let mut buf = vec![];
    let mut num_path = 0;
    let complete: Vec<(usize, usize)> = vec![(0, usize::MAX)];

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
                            "found include coords {:?} for path segment {}",
                            &coords[..],
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
                            "found exclude coords {:?} for path segment {}",
                            &coords[..],
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
                log::debug!("path {} does not intersect with subset coordinates {:?} nor with exclude coordinates {:?} and therefore is skipped from processing", &path_seg, &include_coords, &exclude_coords);

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

                match buf[0] {
                    b'P' => parse_path_seq_update_tables(
                        &buf_path_seg,
                        &graph_aux,
                        &mut item_table,
                        ex,
                        num_path,
                    ),
                    b'W' => parse_walk_seq_update_tables(
                        &buf_path_seg,
                        &graph_aux,
                        &mut item_table,
                        ex,
                        num_path,
                    ),
                    _ => unreachable!(),
                };
            } else {
                let sids = match buf[0] {
                    b'P' => parse_path_seq_to_item_vec(&buf_path_seg, &graph_aux),
                    b'W' => parse_walk_seq_to_item_vec(&buf_path_seg, &graph_aux),
                    _ => unreachable!(),
                };

                match count {
                    CountType::Node | CountType::Bp => update_tables(
                        &mut item_table,
                        &mut subset_covered_bps.as_mut(),
                        &mut exclude_table.as_mut(),
                        num_path,
                        &graph_aux,
                        sids,
                        include_coords,
                        exclude_coords,
                        start,
                    ),
                    CountType::Edge => update_tables_edgecount(
                        &mut item_table,
                        &mut exclude_table.as_mut(),
                        num_path,
                        &graph_aux,
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
    (item_table, exclude_table, subset_covered_bps)
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
) {
    let mut i = 0;
    let mut j = 0;
    let mut p = offset;

    let mut included = 0;
    let mut excluded = 0;

    log::debug!(
        "checking inclusion/exclusion criteria on {} nodes..",
        path.len()
    );

    for (sid, o) in path {
        // update current pointer in include_coords list
        // end is not inclusive, so if end <= p (=offset) then advance to the next interval
        while i < include_coords.len() && include_coords[i].1 <= p {
            i += 1;
        }

        // update current pointer in exclude_coords list
        while j < exclude_coords.len() && exclude_coords[j].1 <= p {
            j += 1;
        }

        let l = graph_aux.node_len(&sid) as usize;

        // this implementation of include coords for bps is *not exact* as illustrated by the
        // following scenario:
        //
        //   subset intervals:           ____________________________
        //                ______________|_____________________________
        //               |
        //      ___________________________________________     ____
        //     |                some node                  |---|
        //      -------------------------------------------     ----
        //
        //
        //   what the following code does:
        //                ___________________________________________
        //               |
        //               |             coverage count
        //      ___________________________________________     ____
        //     |                some node                  |---|
        //      -------------------------------------------     ----
        //
        //
        // in other words, the calculated bps coverage is an upper bound on the actual coverage,
        // for the sake of speed (and implementation effort)
        //
        //
        //
        // node count handling: node is only counted if *completely* covered by subset
        //
        //
        // check if the current position fits within active segment
        if i < include_coords.len() && include_coords[i].0 < p + l {
            let mut a = if include_coords[i].0 > p {
                include_coords[i].0 - p
            } else {
                0
            };
            let mut b = if include_coords[i].1 < p + l {
                include_coords[i].1 - p
            } else {
                l
            };

            // reverse coverage interval in case of backward orientation
            if o == Orientation::Backward {
                (a, b) = (l - b, l - a);
            }

            // only count nodes that are completely contained in "include" coords
            if subset_covered_bps.is_some() || b - a == l {
                let idx = (sid.0 as usize) % SIZE_T;
                item_table.items[idx].push(sid.0);
                item_table.id_prefsum[idx][num_path + 1] += 1;
                if let Some(int) = subset_covered_bps.as_mut() {
                    // if fully covered, we do not need to store anything in the map
                    if b - a == l {
                        if int.contains(&sid) {
                            int.remove(&sid);
                        }
                    } else {
                        int.add(sid, a, b);
                    }
                }
                included += 1;
            }
        }

        if j < exclude_coords.len() && exclude_coords[j].0 < p + l {
            let mut a = if exclude_coords[j].0 > p {
                exclude_coords[j].0 - p
            } else {
                0
            };
            let mut b = if exclude_coords[j].1 < p + l {
                exclude_coords[j].1 - p
            } else {
                l
            };

            // reverse coverage interval in case of backward orientation
            if o == Orientation::Backward {
                (a, b) = (l - b, l - a);
            }

            if let Some(map) = exclude_table {
                if map.with_annotation() {
                    map.activate_n_annotate(sid, l, a, b)
                        .expect("this error should never occur");
                } else if b - a == l {
                    map.activate(&sid);
                }
                excluded += 1;
            }
        }
        if i >= include_coords.len() && j >= exclude_coords.len() {
            // terminate parse if all "include" and "exclude" coords are processed
            break;
        }
        p += l;
    }

    log::debug!(
        "found {} included and {} excluded nodes, and discarded the rest",
        included,
        excluded
    );

    // Compute prefix sum
    for i in 0..SIZE_T {
        item_table.id_prefsum[i][num_path + 1] += item_table.id_prefsum[i][num_path];
    }
    log::debug!("..done");
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
    if path.len() > 0 {
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
            .expect(&format!(
                "unknown edge {}. Is flipped edge known? {}",
                &e,
                if graph_aux.edge2id.as_ref().unwrap().contains_key(&e.flip()) {
                    "Yes"
                } else {
                    "No"
                }
            ));
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
) -> Result<(), std::io::Error> {
    let n = headers.first().unwrap_or(&Vec::new()).len();

    for i in 0..n {
        for j in 0..headers.len() {
            if j > 0 {
                write!(out, "\t")?;
            }
            write!(out, "{:0}", headers[j][i])?;
        }
        writeln!(out, "")?;
    }
    let n = columns.first().unwrap_or(&Vec::new()).len();
    for i in 0..n {
        write!(out, "{}", i)?;
        for j in 0..columns.len() {
            write!(out, "\t{:0}", columns[j][i].floor())?;
        }
        writeln!(out, "")?;
    }

    Ok(())
}

pub fn write_hist_table<W: Write>(
    hists: &Vec<Hist>,
    out: &mut BufWriter<W>,
) -> Result<(), std::io::Error> {
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
    hists: &Option<Vec<Hist>>,
    growths: &Vec<(CountType, Vec<Vec<f64>>)>,
    hist_aux: &HistAuxilliary,
    out: &mut BufWriter<W>,
) -> Result<(), std::io::Error> {
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

    if let Some(hs) = hists {
        for h in hs.iter() {
            output_columns.push(h.coverage.iter().map(|x| *x as f64).collect());
            header_cols.push(vec![
                "hist".to_string(),
                h.count.to_string(),
                String::new(),
                String::new(),
            ])
        }
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
                    vec![p.to_string(), t.to_string(), c.to_string(), q.to_string()]
                }),
        );
    }
    write_table(&header_cols, &output_columns, out)
}

pub fn write_ordered_histgrowth_table<W: Write>(
    abacus_group: &AbacusByGroup,
    hist_aux: &HistAuxilliary,
    out: &mut BufWriter<W>,
) -> Result<(), std::io::Error> {
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
            abacus_group.calc_growth(&c, &q)
        })
        .collect();

    // insert empty row for 0 element
    for c in &mut output_columns {
        c.insert(0, std::f64::NAN);
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
                vec![p.to_string(), t.to_string(), c.to_string(), q.to_string()]
            })
            .collect::<Vec<Vec<String>>>(),
    );
    write_table(&header_cols, &output_columns, out)
}
