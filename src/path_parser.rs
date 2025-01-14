/* standard use */
use std::io::{BufRead, BufReader, Read};
use std::str::{self, FromStr};
use std::sync::{Arc, Mutex};

use std::cell::RefCell;
use memchr::memchr_iter;
use memchr::memchr2_iter;
use memchr::memchr;

/* external use */
use itertools::Itertools;
use rayon::prelude::*;

/* internal use */
use crate::graph::*;
use crate::path::*;
use crate::util::*;

use crate::Bench;

pub fn parse_walk_identifier(data: &[u8]) -> (PathSegment, &[u8]) {
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
        .unwrap();

    log::debug!("parsing walk sequences of size {}..", end);
    if end == 0 {
        log::debug!("empty walk, skipping.");
        return Vec::new();
    }

    // ignore first > | < so that no empty is created for 1st node
    let sids: Vec<(ItemId, Orientation)> = data[..end]
        .par_split(|x| &s1 == x)
        .map(|x| {
            if x.is_empty() {
                // not nice... but Rust expects struct `std::iter::Once<(ItemId, util::Orientation)>`
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
                    // not nice... but Rust expects struct `std::iter::Once<(ItemId, util::Orientation)>`
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
) -> u32 {
    // later codes assumes that data is non-empty...
    if data.is_empty() {
        return 0;
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
    if end == 0 {
        log::debug!("empty walk, skipping.");
        return 0;
    }

    // ignore first > | < so that no empty is created for 1st node
    data[1..end]
        .par_split(|&x| x == b'>' || x == b'<')
        .for_each(|node| {
            let sid = *graph_aux.node2id.get(&node[..]).expect(&format!(
                "unknown node {}",
                &str::from_utf8(node).unwrap()[..]
            ));
            let idx = (sid as usize) % SIZE_T;
            if let Ok(_) = mutex_vec[idx].lock() {
                unsafe {
                    (*items_ptr.0)[idx].push(sid);
                    (*id_prefsum_ptr.0)[idx][num_path + 1] += 1;
                }
            }
        });

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
    num_nodes_path as u32
}

pub fn parse_path_seq_to_item_vec(
    data: &[u8],
    graph_aux: &GraphAuxilliary,
) -> Vec<(ItemId, Orientation)> {
    let mut it = data.iter();
    let end = it
        .position(|x| x == &b'\t' || x == &b'\n' || x == &b'\r')
        .unwrap();

    log::debug!("parsing path sequences of size {}..", end);
    if end == 0 {
        log::debug!("empty path, skipping.");
        return Vec::new();
    }

    let sids: Vec<(ItemId, Orientation)> = data[..end]
        .par_split(|&x| x == b',')
        .map(|node| {
            // Parallel
            let sid = *graph_aux
                .node2id
                .get(&node[..node.len() - 1])
                .expect("unknown node");
                    //&format!(
                    //"unknown node",
                    //str::from_utf8(&node[..node.len() - 1]).unwrap()
                    //));
            (sid, Orientation::from_pm(node[node.len() - 1]))
        })
        .collect();

    log::debug!("..done");

    sids
}

const CHUNK_SIZE: usize = 4*1024; // 4 KB
fn sid_from_bytes_path(data: &[u8], graph_aux: &GraphAuxilliary, start: usize, end: usize) -> ItemId{
    let node = &data[start..end];
    let node_name = &node[..node.len() - 1]; // Exclude orientation
    let sid = *graph_aux
        .node2id
        .get(node_name)
        //.expect("unknown node");
        .expect(&format!("unknown node `{}`", str::from_utf8(&node_name).unwrap()));
    sid
}

pub fn parse_path_seq_to_item_vec_fast(
    data: &[u8],
    graph_aux: &GraphAuxilliary,
) -> Vec<ItemId> {
    //if data is "\t\t*" it is an empty path
    if data.len() <= 3 {
        return Vec::new();
    }
    let all_nodes = if memchr(b',',&data).is_some() {
        data.par_chunks(CHUNK_SIZE)
            .enumerate()
            .map(|(i, chunk)| {
                let mut nodes: Vec<u64> = Vec::with_capacity(2048); //closest power of 2 to ceil(4*1024/3)
                //0+,10-,43|53-\t*,
                let mut start = memchr_iter(b',',&data[..i*CHUNK_SIZE]).rev().next().unwrap_or(0);
                if start > 0 {
                    start += 1;
                }
                let end = memchr(b',',&data[i*CHUNK_SIZE..]);
                if let Some(end) = end {
                    let sid = sid_from_bytes_path(&data, &graph_aux, start, i*CHUNK_SIZE+end);
                    nodes.push(sid);
                    let first_comma = memchr(b',', chunk);
                    if let Some(first_comma) = first_comma {
                        let mut start = 0;
                        let chunk = &chunk[first_comma+1..];
                        for end in memchr_iter(b',', chunk) {
                            let sid = sid_from_bytes_path(&chunk, &graph_aux, start, end);
                            nodes.push(sid);
                            start = end + 1;
                        }
                    }
                }
                if i == ((data.len()/CHUNK_SIZE)-1 + (data.len()%CHUNK_SIZE != 0) as usize) {
                    let mut start = memchr_iter(b',',&data[..data.len()]).rev().next();
                    if let Some(mut start) = start {
                        start += 1;
                        let end = memchr_iter(b'\t',&data[start..]).next()
                            .expect(&format!("error parsing `{}`", str::from_utf8(&data[start..]).unwrap()));
                        let sid = sid_from_bytes_path(&data, &graph_aux, start, start+end);
                        nodes.push(sid);
                    }
                }
                nodes
            })
            .reduce(Vec::new, |mut acc, nodes| {
                acc.extend(nodes);
                acc
            })
    } else {
        let end = memchr(b'\t',&data)
            .expect(&format!("error parsing `{}`", str::from_utf8(&data).unwrap()));
        let sid = sid_from_bytes_path(&data, &graph_aux, 0, end);

        vec![sid; 1]
    };

    log::debug!("..done");

    all_nodes
}

//    let items_ptr = Wrap(&mut item_table.items);
//    let id_prefsum_ptr = Wrap(&mut item_table.id_prefsum);
//    let mutex_vec: Vec<_> = item_table
//
//        pub items: [Vec<ItemId>; SIZE_T],
//    pub id_prefsum: [Vec<ItemId>; SIZE_T],

fn parse_path_seq_update_tables(
    data: &[u8],
    graph_aux: &GraphAuxilliary,
    item_table: &mut ItemTable,
    items_ptr: &Wrap<[Vec<ItemId>; SIZE_T]>,
    id_prefsum_ptr: &Wrap<[Vec<ItemId>; SIZE_T]>,
    //mutex_vec: Arc<Mutex<Vec<usize>>>,
    exclude_table: Option<&mut ActiveTable>,
    num_path: usize,
) -> u32 {
    let mut it = data.iter();
    let end = it
        .position(|x| x == &b'\t' || x == &b'\n' || x == &b'\r')
        .unwrap();

    log::debug!("parsing path sequences of size {} bytes..", end);
    if end == 0 {
        log::debug!("empty path, skipping.");
        return 0;
    }

    //let mutex_vec = Arc::new(Mutex::new(vec![0; SIZE_T]));

    ////let items_ptr = Wrap(&mut item_table.items);
    ////let id_prefsum_ptr = Wrap(&mut item_table.id_prefsum);

    ////let mutex_vec: Vec<_> = item_table
    ////    .items
    ////    .iter()
    ////    .map(|x| Arc::new(Mutex::new(x)))
    ////    .collect();

    ////let mut plus_strands: Vec<u32> = vec![0; rayon::current_num_threads()];
    //data[..end].par_split(|&x| x == b',').for_each(|node| {
    //    let sid = *graph_aux
    //        .node2id
    //        .get(&node[0..node.len() - 1])
    //        .expect(&format!(
    //            "unknown node {}",
    //            &str::from_utf8(node).unwrap()[..]
    //        ));
    //    let o = node[node.len() - 1];
    //    assert!(
    //        o == b'-' || o == b'+',
    //        "unknown orientation of segment {}",
    //        str::from_utf8(&node).unwrap()
    //    );
    //    //plus_strands[rayon::current_thread_index().unwrap()] += (o == b'+') as u32;

    //    let idx = (sid as usize) % SIZE_T;

    //    if let Ok(_) = mutex_vec[idx].lock() {
    //        unsafe {
    //            (*items_ptr.0)[idx].push(sid);
    //            (*id_prefsum_ptr.0)[idx][num_path + 1] += 1;
    //        }
    //    }
    //});

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
    num_nodes_path as u32
}

pub fn parse_cdbg_gfa_paths_walks<R: Read>(
    data: &mut BufReader<R>,
    //_path_aux: &PathAuxilliary, it should be added to allow subsetting
    graph_aux: &GraphAuxilliary,
    k: usize,
) -> ItemTable {
    let mut item_table = ItemTable::new(graph_aux.path_segments.len());
    //let mut k_count = 0;
    //let (mut subset_covered_bps, mut exclude_table, include_map, exclude_map) = path_aux.load_optional_subsetting(&graph_aux, &count);

    let mut num_path = 0;
    let mut buf = vec![];
    while data.read_until(b'\n', &mut buf).unwrap_or(0) > 0 {
        if buf[0] == b'P' {
            let (_path_seg, buf_path_seg) = parse_path_identifier(&buf);
            let sids = parse_path_seq_to_item_vec(&buf_path_seg, &graph_aux);
            let mut u_sid = sids[0].0 as usize - 1;
            let mut u_ori = sids[0].1;
            for i in 1..sids.len() {
                let v_sid = sids[i].0 as usize - 1;
                let v_ori = sids[i].1;
                let k_plus_one_mer =
                    graph_aux.get_k_plus_one_mer_edge(u_sid, u_ori, v_sid, v_ori, k);
                //println!("{}", bits2kmer(k_plus_one_mer, k+1));
                let infix = get_infix(k_plus_one_mer, k);
                let infix_rc = revcmp(infix, k - 1);
                if infix < infix_rc {
                    let idx = (infix as usize) % SIZE_T;
                    item_table.items[idx].push(k_plus_one_mer);
                    item_table.id_prefsum[idx][num_path + 1] += 1;
                } else if infix > infix_rc {
                    let idx = (infix_rc as usize) % SIZE_T;
                    item_table.items[idx].push(revcmp(k_plus_one_mer, k + 1));
                    item_table.id_prefsum[idx][num_path + 1] += 1;
                } // else ignore palindrome, since it always breaks the node

                u_sid = v_sid;
                u_ori = v_ori;
            }

            // compute prefix sum
            for i in 0..SIZE_T {
                item_table.id_prefsum[i][num_path + 1] += item_table.id_prefsum[i][num_path];
            }

            num_path += 1;
        }
        buf.clear();
    }

    item_table
}

pub fn parse_gfa_paths_walks<R: Read>(
    data: &mut BufReader<R>,
    path_aux: &PathAuxilliary,
    graph_aux: &GraphAuxilliary,
    count: &CountType,
) -> (
    Vec<ItemId>,
    Vec<usize>,
    ItemTable,
    Option<ActiveTable>,
    Option<IntervalContainer>,
    Vec<u32>,
) {
    log::info!("parsing path + walk sequences");
    let mut items: Vec<ItemId> = vec![];
    let mut path_to_items: Vec<usize> = vec![0; graph_aux.path_segments.len()+1];

    let mut item_table = ItemTable::new(graph_aux.path_segments.len());
    let (mut subset_covered_bps, mut exclude_table, include_map, exclude_map) =
        path_aux.load_optional_subsetting(&graph_aux, &count);

    let mut num_path = 0;
    let complete: Vec<(usize, usize)> = vec![(0, usize::MAX)];
    let mut paths_len: Vec<u32> = Vec::new();

    //prepare mutex for item_table and prefix sum 
    let items_ptr = Wrap(&mut item_table.items);
    let id_prefsum_ptr = Wrap(&mut item_table.id_prefsum);
    //let mutex_vec = Arc::new(Mutex::new((0..SIZE_T).collect()));

    let mut buf = vec![];
    while data.read_until(b'\n', &mut buf).unwrap_or(0) > 0 {
        if buf[0] == b'P' || buf[0] == b'W' {
            let (path_seg, buf_path_seg) = match buf[0] {
                b'P' => parse_path_identifier(&buf),
                b'W' => parse_walk_identifier(&buf),
                _ => unreachable!(),
            };

            log::debug!("processing path {}", &path_seg);

            let include_coords = if path_aux.include_coords.is_none() {
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
            let exclude_coords = if path_aux.exclude_coords.is_none() {
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
            if path_aux.include_coords.is_some()
                && !intersects(include_coords, &(start, end))
                && !intersects(exclude_coords, &(start, end)) //This is superfluos
            {
                log::debug!("path {} does not intersect with subset coordinates {:?} nor with exclude coordinates {:?} and therefore is skipped from processing", 
                    &path_seg, &include_coords, &exclude_coords);

                // update prefix sum
                for i in 0..SIZE_T {
                    item_table.id_prefsum[i][num_path + 1] += item_table.id_prefsum[i][num_path];
                }

                num_path += 1;
                buf.clear();
                continue;
            }

            if count != &CountType::Edge
                && (path_aux.include_coords.is_none() || is_contained(include_coords, &(start, end)))
                && (path_aux.exclude_coords.is_none() || is_contained(exclude_coords, &(start, end)))
            {
                log::debug!("path {} is fully contained within subset coordinates {:?} and is eligible for full parallel processing", path_seg, include_coords);
                let ex = if exclude_coords.is_empty() {
                    None
                } else {
                    exclude_table.as_mut()
                };

                let sids = match buf[0] {
                    b'P' => parse_path_seq_to_item_vec_fast(&buf_path_seg, &graph_aux),
                    //b'W' => parse_walk_seq_to_item_vec(&buf_path_seg, &graph_aux),
                    _ => unreachable!(),
                };
                path_to_items[num_path+1] = path_to_items[num_path] + sids.len();
                paths_len.push(sids.len() as u32);
                items.extend(sids);

                //let num_added_nodes = match buf[0] {
                //    b'P' => parse_path_seq_update_tables(
                //        &buf_path_seg,
                //        &graph_aux,
                //        &mut item_table,
                //        &items_ptr,
                //        &id_prefsum_ptr,
                //        ex,
                //        num_path,
                //    ),
                //    b'W' => parse_walk_seq_update_tables(
                //        &buf_path_seg,
                //        &graph_aux,
                //        &mut item_table,
                //        ex,
                //        num_path,
                //    ),
                //    _ => unreachable!(),
                //};
                //paths_len.push(num_added_nodes as u32);
            } else {
                let sids = match buf[0] {
                    b'P' => parse_path_seq_to_item_vec(&buf_path_seg, &graph_aux),
                    b'W' => parse_walk_seq_to_item_vec(&buf_path_seg, &graph_aux),
                    _ => unreachable!(),
                };

                paths_len.push(sids.len() as u32);

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
    (items, path_to_items, item_table, exclude_table, subset_covered_bps, paths_len)
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

        let l = graph_aux.node_len(sid) as usize;

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
                let idx = (sid as usize) % SIZE_T;
                item_table.items[idx].push(sid);
                item_table.id_prefsum[idx][num_path + 1] += 1;
                if let Some(int) = subset_covered_bps.as_mut() {
                    // if fully covered, we do not need to store anything in the map
                    if b - a == l {
                        if int.contains(sid) {
                            int.remove(sid);
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
                    map.activate(sid);
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
        p += graph_aux.node_len(path[0].0) as usize;
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

        let l = graph_aux.node_len(sid2) as usize;

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
            let idx = (*eid as usize) % SIZE_T;
            item_table.items[idx].push(*eid);
            item_table.id_prefsum[idx][num_path + 1] += 1;
        }
        if exclude_table.is_some() && j < exclude_coords.len() && exclude_coords[j].0 < p + l {
            exclude_table.as_mut().unwrap().activate(*eid);
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

pub fn parse_bed_to_path_segments<R: Read>(data: &mut BufReader<R>, use_block_info: bool) -> Vec<PathSegment> {
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
            let mut fields: Vec<&str>  = line.split('\t').collect();
            if fields.is_empty() {
                fields = vec![&line];
            }
            fields
        };
        let path_name = fields[0];
        
        if path_name.starts_with("browser ") || path_name.starts_with("track ") || path_name.starts_with("#") {
            continue;
        }

        if fields.len() == 1 {
            segments.push(PathSegment::from_str(path_name));
        } else if fields.len() >= 3 {
            let start = usize::from_str(fields[1]).expect(&format!("error line {}: `{}` is not an usize",i+1, fields[1]));
            let end = usize::from_str(fields[2]).expect(&format!("error line {}: `{}` is not an usize",i+1, fields[2]));

            if use_block_info && fields.len() == 12 {
                let block_count = fields[9].parse::<usize>().unwrap_or(0);
                let block_sizes: Vec<usize> = fields[10].split(',')
                    .filter_map(|s| usize::from_str(s.trim()).ok())
                    .collect();
                let block_starts: Vec<usize> = fields[11].split(',')
                    .filter_map(|s| usize::from_str(s.trim()).ok())
                    .collect();

                if block_count == block_sizes.len() && block_count == block_starts.len() {
                    for (size, start_offset) in block_sizes.iter().zip(block_starts.iter()) {
                        let block_start = start + start_offset;
                        let block_end = block_start + size;
                        segments.push(PathSegment::from_str_start_end(path_name, block_start, block_end));
                    }
                } else {
                    panic!("error in block sizes/starts in line {}: counts do not match", i + 1);
                }
            } else {
                segments.push(PathSegment::from_str_start_end(path_name, start, end));
            }
        } else {
            panic!("error in line {}: row must have either 1, 3, or 12 columns, but has 2", i + 1);
        }
    }

    segments
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::str::from_utf8;
    use std::io::Cursor;

    fn mock_graph_auxilliary() -> GraphAuxilliary {
        GraphAuxilliary {
            node2id: {
                let mut node2id = HashMap::new();
                node2id.insert(b"node1".to_vec(), 1);
                node2id.insert(b"node2".to_vec(), 2);
                node2id.insert(b"node3".to_vec(), 3);
                node2id
            },
            node_lens: Vec::new(),
            edge2id: None,
            path_segments: Vec::new(),
            node_count: 3,
            edge_count: 0,
            degree: Some(Vec::new()),
            extremities: Some(Vec::new())
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

        assert_eq!(path_segment.to_string(), "GCF_000005845.2_ASM584v2_genomic.fna#0#contig1");
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
        assert_eq!(result[0], (1, Orientation::Forward));
        assert_eq!(result[1], (2, Orientation::Backward));
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
            vec![
                PathSegment::from_str("chr1"),
                PathSegment::from_str("chr2"),
            ]
        );
    }

    #[test]
    #[should_panic(expected = "error in line 1: row must have either 1, 3, or 12 columns, but has 2")]
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
            vec![
                { 
                    let mut tmp = PathSegment::from_str("chr1");
                    tmp.start = Some(1000);
                    tmp.end = Some(2000);
                    tmp
                }
            ]
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
}
