/* standard use */

use rustc_hash::FxHashMap;
use std::str::{self, FromStr};

/* crate use */
use quick_csv::{columns::BytesColumns, Csv};

use super::{Node, PathSegment};

pub fn parse_walk_line(mut row_it: BytesColumns) -> (PathSegment, Vec<(String, bool)>) {
    let sample_id = str::from_utf8(row_it.next().unwrap()).unwrap().to_string();
    let hap_id = str::from_utf8(row_it.next().unwrap()).unwrap().to_string();
    let seq_id = str::from_utf8(row_it.next().unwrap()).unwrap().to_string();
    let seq_start = match str::from_utf8(row_it.next().unwrap()).unwrap() {
        "*" => None,
        a => Some(usize::from_str(a).unwrap()),
    };

    let seq_end = match str::from_utf8(row_it.next().unwrap()).unwrap() {
        "*" => None,
        a => Some(usize::from_str(a).unwrap()),
    };

    let path_seg = PathSegment::new(sample_id, hap_id, seq_id, seq_start, seq_end);

    log::info!("processing walk {}", &path_seg);

    let walk_data = row_it.next().unwrap();
    let walk = parse_walk(walk_data.to_vec());
    (path_seg, walk)
}

pub fn parse_path_line<'a>(mut row_it: BytesColumns<'a>) -> (PathSegment, Vec<(String, bool)>) {
    let path_name = str::from_utf8(row_it.next().unwrap()).unwrap().to_string();
    log::info!("processing path {}", path_name);

    (
        PathSegment::from_string(&path_name),
        parse_path(row_it.next().unwrap()),
    )
}

fn parse_path(path_data: &[u8]) -> Vec<(String, bool)> {
    let mut path: Vec<(String, bool)> = Vec::with_capacity(1_000_000);

    log::debug!("parsing path string of size {}..", path_data.len());
    let mut cur_pos = 0;
    for (i, c) in path_data.iter().enumerate() {
        if c == &b',' {
            let sid = str::from_utf8(&path_data[cur_pos..i - 1])
                .unwrap()
                .to_string();
            let o = path_data[i - 1];
            assert!(
                o == b'+' || o == b'-',
                "unknown orientation {} or segment {}",
                o,
                &sid
            );
            path.push((sid, o == b'-'));
            cur_pos = i + 1;
        }
    }

    if cur_pos < path_data.len() {
        let sid = str::from_utf8(&path_data[cur_pos..path_data.len() - 1])
            .unwrap()
            .to_string();
        let o = path_data[path_data.len() - 1];
        assert!(
            o == b'+' || o == b'-',
            "unknown orientation {} or segment {}",
            o,
            sid
        );
        path.push((sid, o == b'-'));
    }
    log::debug!("..done; path has {} elements", path.len());
    path
}

fn parse_walk(walk_data: Vec<u8>) -> Vec<(String, bool)> {
    let mut walk: Vec<(String, bool)> = Vec::new();

    let mut cur_el: Vec<u8> = Vec::new();
    for c in walk_data {
        if (c == b'>' || c == b'<') && !cur_el.is_empty() {
            let sid = str::from_utf8(&cur_el[1..]).unwrap().to_string();
            assert!(
                cur_el[0] == b'>' || cur_el[0] == b'<',
                "unknown orientation {} or segment {}",
                cur_el[0],
                sid
            );
            walk.push((sid, cur_el[0] == b'<'));
            cur_el.clear();
        }
        cur_el.push(c);
    }

    if !cur_el.is_empty() {
        let sid = str::from_utf8(&cur_el[1..]).unwrap().to_string();
        assert!(
            cur_el[0] == b'>' || cur_el[0] == b'<',
            "unknown orientation {} or segment {}",
            cur_el[0],
            sid
        );
        walk.push((sid, cur_el[0] == b'<'));
    }
    walk
}

pub fn count_pw_lines<R: std::io::Read>(data: &mut std::io::BufReader<R>) -> usize {
    let mut count = 0;

    let reader = Csv::from_reader(data)
        .delimiter(b'\t')
        .flexible(true)
        .has_header(false);
    for row in reader {
        let row = row.unwrap();
        let mut row_it = row.bytes_columns();
        let fst_col = row_it.next().unwrap();
        if fst_col == &[b'W'] || fst_col == &[b'P'] {
            count += 1;
        }
    }

    count
}

//pub fn count_path_walk_lines(data: &mut dyn std::io::Read) -> usize {
//    let mut count = 0;
//
//    let mut it = data.bytes();
//    let mut b = it.next();
//    while b.is_some() {
//        if let Some(res) = &b {
//            let c = res.as_ref().unwrap();
//            if c == &b'\n' || c == &b'\r' {
//                b = it.next();
//                if let Some(res) = &b {
//                    let c = res.as_ref().unwrap();
//                    if c == &b'P' || c == &b'W' {
//                        count += 1;
//                        b = it.next();
//                    }
//                }
//            }
//        } else {
//            b = it.next();
//        }
//    }
//
//    count
//
//}

pub fn parse_bed<R: std::io::Read>(data: &mut std::io::BufReader<R>) -> Vec<PathSegment> {
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
        let mut path_seg = PathSegment::from_string(&path_name);
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

pub fn parse_groups<R: std::io::Read>(data: &mut std::io::BufReader<R>) -> FxHashMap<PathSegment, String> {
    let mut res = FxHashMap::default();
    
    let reader = Csv::from_reader(data)
        .delimiter(b'\t')
        .flexible(true)
        .has_header(false);
    for (i, row) in reader.enumerate() {
        let row = row.unwrap();
        let mut row_it = row.bytes_columns();
        let path_seg = PathSegment::from_string(&str::from_utf8(row_it.next().unwrap()).unwrap().to_string());
        if path_seg.coords().is_some() {
            panic!("Error in line {}: coordinates are not permitted in grouping paths", i);
        }
        if res.insert(path_seg, str::from_utf8(row_it.next().unwrap()).unwrap().to_string()).is_some() {
            panic!("Error in line {}: contains duplicate path entry", i);
        }
    }

    res
}

pub fn parse_gfa_nodecount<R: std::io::Read>(
    data: &mut std::io::BufReader<R>,
) -> (FxHashMap<Node, Vec<usize>>, Vec<PathSegment>) {
    let mut countable2path: FxHashMap<Node, Vec<usize>> = FxHashMap::default();
    let mut paths: Vec<PathSegment> = Vec::new();

    let mut node2id: FxHashMap<String, u32> = FxHashMap::default();
    let mut node_count = 0;

    let reader = Csv::from_reader(data)
        .delimiter(b'\t')
        .flexible(true)
        .has_header(false);
    for row in reader {
        let row = row.unwrap();
        let mut row_it = row.bytes_columns();
        let fst_col = row_it.next().unwrap();
        if fst_col == &[b'S'] {
            let sid = row_it.next().expect("segment line has no segment ID");
            node2id
                .entry(str::from_utf8(sid).unwrap().to_string())
                .or_insert({
                    node_count += 1;
                    node_count - 1
                });
            countable2path.insert(Node::new(node_count - 1, 1), Vec::new());
        } else if fst_col == &[b'W'] {
            let (path_seg, walk) = parse_walk_line(row_it);
            paths.push(path_seg);
            walk.into_iter().for_each(|(node, _)| {
                countable2path
                    .get_mut(&Node::new(
                        *node2id
                            .get(&node)
                            .expect(&format!("unknown node {}", &node)),
                        1,
                    ))
                    .expect(&format!("unknown node {}", &node))
                    .push(paths.len());
            });
        } else if &[b'P'] == fst_col {
            let (path_seg, path) = parse_path_line(row_it);
            paths.push(path_seg);
            let cur_len = countable2path.len();
            log::debug!("updating count data structure..");
            path.into_iter().for_each(|(node, _)| {
                countable2path
                    .get_mut(&Node::new(
                        *node2id
                            .get(&node)
                            .expect(&format!("unknown node {}", &node)),
                        1,
                    ))
                    .expect(&format!("unknown node {}", &node))
                    .push(paths.len());
            });
            log::debug!(
                "done; data structure has now {} more elements",
                countable2path.len() - cur_len
            );
        }
    }
    (countable2path, paths)
}

//    fn parse_length(gfa_file: &str) -> FxHashMap<Handle, usize> {
//        let mut res: FxHashMap<Handle, usize> = FxHashMap::default();
//
//        let parser = GFAParser::new();
//        let gfa: GFA<usize, ()> = parser.parse_file(gfa_file).unwrap();
//        for s in gfa.segments.iter() {
//            res.insert(Handle::pack(s.name, false), s.sequence.len());
//        }
//
//        res
//    }
//
//    fn read_samples<R: std::io::Read>(mut data: std::io::BufReader<R>) -> Vec<String> {
//        let mut res = Vec::new();
//
//        let reader = Csv::from_reader(&mut data)
//            .delimiter(b'\t')
//            .flexible(true)
//            .has_header(false);
//        for row in reader.into_iter() {
//            let row = row.unwrap();
//            let mut row_it = row.bytes_columns();
//            res.push(str::from_utf8(row_it.next().unwrap()).unwrap().to_string());
//        }
//
//        res
//    }
