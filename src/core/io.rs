/* standard use */

use rustc_hash::FxHashMap;
use std::collections::HashMap;
use std::error::Error;
use std::io::{BufRead, Read, BufReader};
use std::str::{self, FromStr};

/* crate use */
use quick_csv::{columns::BytesColumns, Csv};

//use rayon::iter::{IntoParallelIterator, ParallelIterator};
use super::{Abacus, CoverageThreshold, Node, NodeTable, PathSegment, Prep, Wrap, SIZE_T};
use rayon::prelude::*;
use std::sync::{Arc, Mutex};

pub fn parse_bed<R: Read>(data: &mut BufReader<R>) -> Vec<PathSegment> {
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
) -> FxHashMap<PathSegment, String> {
    let mut res = FxHashMap::default();

    let reader = Csv::from_reader(data)
        .delimiter(b'\t')
        .flexible(true)
        .has_header(false);
    for (i, row) in reader.enumerate() {
        let row = row.unwrap();
        let mut row_it = row.bytes_columns();
        let path_seg =
            PathSegment::from_str(&str::from_utf8(row_it.next().unwrap()).unwrap().to_string());
        if path_seg.coords().is_some() {
            panic!(
                "Error in line {}: coordinates are not permitted in grouping paths",
                i
            );
        }
        if res
            .insert(
                path_seg,
                str::from_utf8(row_it.next().unwrap()).unwrap().to_string(),
            )
            .is_some()
        {
            panic!("Error in line {}: contains duplicate path entry", i);
        }
    }

    res
}

pub fn parse_coverage_threshold_file<R: Read>(
    data: &mut BufReader<R>,
) -> Vec<(String, CoverageThreshold)> {
    let mut res = Vec::new();

    let reader = Csv::from_reader(data)
        .delimiter(b'\t')
        .flexible(true)
        .has_header(false);
    for row in reader {
        let row = row.unwrap();
        let mut row_it = row.bytes_columns();
        let name = str::from_utf8(row_it.next().unwrap())
            .unwrap()
            .trim()
            .to_string();
        let threshold = if let Some(col) = row_it.next() {
            let threshold_str = str::from_utf8(col).unwrap();
            if let Some(t) = usize::from_str(threshold_str).ok() {
                CoverageThreshold::Absolute(t)
            } else {
                CoverageThreshold::Relative(f64::from_str(threshold_str).unwrap())
            }
        } else {
            if let Some(t) = usize::from_str(&name[..]).ok() {
                CoverageThreshold::Absolute(t)
            } else {
                CoverageThreshold::Relative(f64::from_str(&name[..]).unwrap())
            }
        };
        res.push((name, threshold));
    }

    res
}

pub fn count_pw_lines(pathfile: &str) -> Result<usize, Box<dyn Error>> {
    //let mut streams = HashMap::new();
    let mut br = BufReader::new(std::fs::File::open(pathfile)?);
    let mut buf = vec![];
    let mut count: usize = 0;
    while br.read_until(b'\n', &mut buf).unwrap_or(0) > 0 {
        //println!("{}", str::from_utf8(&buf).unwrap());
        if buf[0] == b'W' || buf[0] == b'P' {
            count += 1;
        }
        buf.clear();
    }

    Ok(count)
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

fn parse_walk_seq(
    data: &[u8],
    node2id: &HashMap<Vec<u8>, u32>,
    node_table: &mut NodeTable,
    num_walk: usize,
) {
    let mut it = data.iter();
    let end = it
        .position(|x| x == &b'\t' || x == &b'\n' || x == &b'\r')
        .unwrap();

    log::debug!("parsing path sequences of size {}..", end);

    let T_ptr = Wrap(&mut node_table.T);
    let ts_ptr = Wrap(&mut node_table.ts);

    let mut mutex_vec: Vec<_> = node_table.T.iter().map(|x| Arc::new(Mutex::new(x))).collect();

    data.par_split(|&x| x == b'<' || x == b'>').for_each( |node| { // Parallel
    //path_data.split(|&x| x == b',').for_each( |node| {  // Sequential
        if let Some(sid) = node2id.get(&node[0..node.len()]) {
            let sid = *sid;
            let idx = (sid as usize)%SIZE_T;

            if let Ok(lock) = mutex_vec[idx].lock() {
                unsafe{
                    (*T_ptr.0)[idx].push(sid);
                    (*ts_ptr.0)[idx][num_walk+1] += 1;
                }
            }
            //Sequential
            //node_table.T[idx].push(sid);
            //node_table.ts[idx][num_path+1] += 1;
        }
    });

    // Compute prefix sum
    for i in 0..SIZE_T {
        node_table.ts[i][num_walk+1] += node_table.ts[i][num_walk];
    }
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

fn modify_address<T>(start_vec: &[T], add: usize) -> &T {
    let start_ptr = start_vec.as_ptr() as usize;
    //let comma_ptr = node_ptr + node.len();
    let mod_ptr = start_ptr + add;
    let new_ptr = unsafe { &*(mod_ptr as *const T) };
    //println!("{:p} {:p}", a, node.as_ptr());
    //println!("{} {} {} {}", node[0], a, b',', node.len());
    new_ptr
}

fn parse_path_seq(
    data: &[u8],
    node2id: &HashMap<Vec<u8>, u32>,
    node_table: &mut NodeTable,
    num_path: usize,
) {
    let mut it = data.iter();
    let end = it
        .position(|x| x == &b'\t' || x == &b'\n' || x == &b'\r')
        .unwrap();

    log::debug!("parsing path sequences of size {}..", end);

    let num_path = num_path as usize;
    let T_ptr = Wrap(&mut node_table.T);
    let ts_ptr = Wrap(&mut node_table.ts);

    let mut mutex_vec: Vec<_> = node_table.T.iter().map(|x| Arc::new(Mutex::new(x))).collect();

    data.par_split(|&x| x == b',').for_each( |node| { // Parallel
    //path_data.split(|&x| x == b',').for_each( |node| {  // Sequential
        let sid = *node2id.get(&node[0..node.len()-1]).unwrap();
        let o = node[node.len() - 1];
        assert!(
            o == b'-' || o == b'+',
            "unknown orientation of segment {}",
            str::from_utf8(&node).unwrap()
        );
        let idx = (sid as usize)%SIZE_T;
        
        if let Ok(lock) = mutex_vec[idx].lock() {
            unsafe{
                (*T_ptr.0)[idx].push(sid);
                (*ts_ptr.0)[idx][num_path+1] += 1;
            }
        }

        //Sequential
        //node_table.T[idx].push(sid);
        //node_table.ts[idx][num_path+1] += 1;

    });

    // Compute prefix sum
    for i in 0..SIZE_T {
        node_table.ts[i][num_path + 1] += node_table.ts[i][num_path];
    }

    log::debug!("..done");
}

pub fn parse_gfa_nodecount<R: Read>(
    data: &mut BufReader<R>,
    prep: &Prep,
) -> NodeTable {
    let mut node_table = NodeTable::new(prep.path_segments.len());
    let mut path_segs: Vec<PathSegment> = vec![];

    // Reading GFA file searching for (P)aths and (W)alks
    let mut buf = vec![];
    let mut num_path = 0;
    while data.read_until(b'\n', &mut buf).unwrap_or(0) > 0 {
        if buf[0] == b'P' {
            let (path_seg, buf_path_seg) = parse_path_identifier(&buf);
            log::debug!("updating count data structure..");
            parse_path_seq(&buf_path_seg, &prep.node2id, &mut node_table, num_path);
            log::debug!("done");
            path_segs.push(path_seg);
            num_path += 1;
        } else if buf[0] == b'W' {
            let (path_seg, buf_walk_seq) = parse_walk_identifier(&buf);
            log::debug!("updating count data structure..");
            parse_walk_seq(&buf_walk_seq, &prep.node2id, &mut node_table, num_path);
            log::debug!("done");
            path_segs.push(path_seg);
            num_path += 1;
        }

        buf.clear();
    }
    node_table
}

pub fn preprocessing<R: Read>(data: &mut BufReader<R>) -> Prep {
    let mut node_count = 0;
    let mut node2id: HashMap<Vec<u8>, u32> = HashMap::default();
    let mut paths: Vec<PathSegment> = Vec::new();

    let mut buf = vec![];
    while data.read_until(b'\n', &mut buf).unwrap_or(0) > 0 {
        if buf[0] == b'S' {
            let mut iter = buf.iter();

            let start = iter.position(|&x| x == b'\t').unwrap() + 1;
            let offset = iter.position(|&x| x == b'\t').unwrap();
            let sid = buf[start..start + offset].to_vec();
            node2id.entry(sid).or_insert(node_count);
            node_count += 1;
        } else if buf[0] == b'P' {
            let (path_seg, _) = parse_path_identifier(&buf);
            paths.push(path_seg);
        } else if buf[0] == b'W' {
            let (path_seg, _) = parse_walk_identifier(&buf);
            paths.push(path_seg);
        }

        buf.clear();
    }

    Prep {
        path_segments: paths,
        node2id: node2id,
    }
}

//pub fn parse_walk_line(buf: &[u8], node2id: &HashMap<Vec<u8>, u32>)  -> (PathSegment, Vec<(u32, bool)>) {
//    let mut six_col : Vec<&str> = Vec::with_capacity(6);
//
//    let mut it = buf.iter();
//    let mut i = 0;
//    for _ in 0..6 {
//        let j = it.position(|x| x == &b'\t').unwrap();
//        six_col.push(&str::from_utf8(&buf[i..i+j]).unwrap());
//        i += j+1;
//    }
//
//    let seq_start = match six_col[4] {
//        "*" => None,
//        a => Some(usize::from_str(a).unwrap()),
//    };
//
//    let seq_end = match six_col[5] {
//        "*" => None,
//        a => Some(usize::from_str(a).unwrap()),
//    };
//
//    let path_seg = PathSegment::new(six_col[1].to_string(), six_col[2].to_string(), six_col[3].to_string(), seq_start, seq_end);
//
//    log::info!("processing walk {}", &path_seg);
//
//    let walk_end = it.position(|x| x == &b'\t' || x == &b'\n' || x == &b'\r').unwrap();
//    let walk = parse_walk(&buf[i..i+walk_end], node2id);
//    (path_seg, walk)
//}
//
//
//fn parse_walk(walk_data: &[u8], node2id: &HashMap<Vec<u8>, u32>) -> Vec<(u32, bool)> {
//    let mut walk: Vec<(u32, bool)> = Vec::new();
//
//    let mut i = 0;
//    for j in 0..walk_data.len() {
//        if (walk_data[j] == b'>' || walk_data[j] == b'<') && i < j {
//            assert!(
//                walk_data[i] == b'>' || walk_data[i] == b'<',
//                "unknown orientation of segment {}",
//                str::from_utf8(&walk_data[i..j]).unwrap()
//            );
//            walk.push((*node2id.get(&walk_data[i+1..j]).unwrap(), walk_data[i] == b'<'));
//            i = j;
//        }
//    }
//
//    if i < walk_data.len() {
//        assert!(
//            walk_data[i] == b'>' || walk_data[i] == b'<',
//            "unknown orientation of segment {}",
//            str::from_utf8(&walk_data[i..]).unwrap()
//        );
//        walk.push((*node2id.get(&walk_data[i+1..]).expect(&format!("cannot find node {} (position {} in walk) in node2id map", str::from_utf8(&walk_data[i..]).unwrap(), i)[..]), walk_data[i] == b'<'));
//    }
//
//    walk
//}


//pub fn count_path_walk_lines(data: &mut dyn Read) -> usize {
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

//pub fn parse_path_line<'a>(mut row_it: BytesColumns<'a>) -> (PathSegment, Vec<(String, bool)>) {
//let path_name = str::from_utf8(row_it.next().unwrap()).unwrap().to_string();
//pub fn parse_path_line(row_it: &str) -> (PathSegment, Vec<(String, bool)>) {
//    let mut records = row_it.split("\t");
//    let path_name = records.nth(1).unwrap();
//    let path_data = records.next().unwrap();
//    log::info!("processing path {}", path_name);
//
//    (
//        PathSegment::from_string(path_name),
//        parse_path(path_data),
//    )
//}

//fn parse_path(path_data: &str) -> Vec<(String, bool)> {
//    //let mut path: Vec<(String, bool)> = Vec::with_capacity(1_000_000);
//
//    log::debug!("parsing path string of size {}..", path_data.len());
//    let mut cur_pos = 0;
//    let path: Vec<(String, bool)> = path_data.split(",").map(|s| {
//        let o = s.chars().last().unwrap();
//        let mut chs = s.chars();
//        chs.next_back();
//        let sid = chs.as_str();
//        (sid.to_string(), o=='-')
//    }).collect();
//
//    //for node in path_data.split(",") {
//    //
//    //    assert!(
//    //        o == '+' || o == '-',
//    //        "unknown orientation {} or segment {}",
//    //        o,
//    //        &sid
//    //    );
//    //    //path.push((sid.to_owned(), o == '-'));
//    //    cur_pos += 1;
//    //}
//
//    //if cur_pos < path_data.len() {
//    //    let sid = str::from_utf8(&path_data[cur_pos..path_data.len() - 1])
//    //        .unwrap()
//    //        .to_string();
//    //    let o = path_data[path_data.len() - 1];
//    //    assert!(
//    //        o == '+' || o == '-',
//    //        "unknown orientation {} or segment {}",
//    //        o,
//    //        sid
//    //    );
//    //    path.push((sid, o == '-'));
//    //}
//    log::debug!("..done; path has {} elements", path.len());
//    path
//}

//pub fn count_pw_lines_old<R: Read>(data: &mut BufReader<R>) -> usize {
//    let mut count = 0;
//
//    let reader = Csv::from_reader(data)
//        .delimiter(b'\t')
//        .flexible(true)
//        .has_header(false);
//    for row in reader {
//        let row = row.unwrap();
//        let mut row_it = row.bytes_columns();
//        let fst_col = row_it.next().unwrap();
//        if fst_col == &[b'W'] || fst_col == &[b'P'] {
//            count += 1;
//        }
//    }
//
//    count
//}
//pub fn parse_gfa_nodecount2<R: Read>(
//    data: &mut BufReader<R>,
//) -> (FxHashMap<Node, Vec<usize>>, Vec<PathSegment>) {
//    let mut countable2path: FxHashMap<Node, Vec<usize>> = FxHashMap::default();
//    let mut paths: Vec<PathSegment> = Vec::new();
//
//    let mut node2id: FxHashMap<String, u32> = FxHashMap::default();
//    let mut node_count = 0;
//
//    let reader = Csv::from_reader(data)
//        .delimiter(b'\t')
//        .flexible(true)
//        .has_header(false);
//    for row in reader {
//        let row = row.unwrap();
//        let mut row_it = row.bytes_columns();
//        let fst_col = row_it.next().unwrap();
//        if fst_col == &[b'S'] {
//            let sid = row_it.next().expect("segment line has no segment ID");
//            node2id
//                .entry(str::from_utf8(sid).unwrap().to_string())
//                .or_insert({
//                    node_count += 1;
//                    node_count - 1
//                });
//            countable2path.insert(Node::new(node_count - 1, 1), Vec::new());
//        } else if fst_col == &[b'W'] {
//            let (path_seg, walk) = parse_walk_line(row_it);
//            paths.push(path_seg);
//            walk.into_iter().for_each(|(node, _)| {
//                countable2path
//                    .get_mut(&Node::new(
//                        *node2id
//                            .get(&node)
//                            .expect(&format!("unknown node {}", &node)),
//                        1,
//                    ))
//                    .expect(&format!("unknown node {}", &node))
//                    .push(paths.len());
//            });
//        } else if &[b'P'] == fst_col {
//            let (path_seg, path) = parse_path_line(row_it);
//            paths.push(path_seg);
//            let cur_len = countable2path.len();
//            log::debug!("updating count data structure..");
//            path.into_iter().for_each(|(node, _)| {
//                countable2path
//                    .get_mut(&Node::new(
//                        *node2id
//                            .get(&node)
//                            .expect(&format!("unknown node {}", &node)),
//                        1,
//                    ))
//                    .expect(&format!("unknown node {}", &node))
//                    .push(paths.len());
//            });
//            log::debug!(
//                "done; data structure has now {} more elements",
//                countable2path.len() - cur_len
//            );
//        }
//    }
//    (countable2path, paths)
//}

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
//    fn read_samples<R: Read>(mut data: BufReader<R>) -> Vec<String> {
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

//extern crate memmap;
//use self::memmap::{Mmap, Protection};
//extern crate byteorder;
//use self::byteorder::{ByteOrder, LittleEndian};
//pub fn count_pw(filepath: &str) -> Result<usize, Box<dyn Error>> {
//    let file = std::fs::File::open(filepath)?;
//    let mut reader = BufReader::new(file);
//    let mut newlineflag = false;
//    let mut count = 0;
//    loop {
//        let mut buf = [0u8; 8]; // size of u64
//        if reader.read(&mut buf)? == 0 {
//            break;
//        }
//        //let has_newlines = unsafe { 0x0a0a0a0a0a0a0a0a & std::mem::transmute::<_, u64>(buf) };
//        //if has_newlines != 0 {
//        //    for b in &buf {
//        //        //if *b == 0x0a { count += 1; }
//        //        if *b == 0x0a { newlineflag = true; }
//        //        else if newlineflag && (*b == 0x50) { count += 1; newlineflag = false; }
//        //        else { newlineflag = false; }
//        //    }
//        //}
//    }
//
//    Ok(count)
//}

//pub fn count_pw3(filepath: &str) -> Result<usize, Box<dyn Error>> {
//    let file = Mmap::open_path(filepath, Protection::Read)?;
//
//    let mut count = 0;
//
//    let bytes = unsafe { file.as_slice() };
//    for buf in bytes.chunks(std::mem::size_of::<u64>()) {
//        // AND the entire 8 byte buffer with a mask of \n bytes to see if there are any
//        // newlines in the buffer. If there are we search for them, if not, we skip the search
//        // all together.
//        let has_newlines = if buf.len() == std::mem::size_of::<u64>() {
//            0x0a0a0a0a0a0a0a0a & LittleEndian::read_u64(buf)
//        }
//        else {
//            1
//        };
//
//        if has_newlines != 0 {
//            for b in buf {
//                if *b == 0x0a { count += 1; }
//            }
//        }
//    }
//
//    Ok(count)
//}
//
