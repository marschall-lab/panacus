/* standard use */

use std::str::{self, FromStr};

/* crate use */
use quick_csv::columns::BytesColumns;

pub fn parse_walk_line<'a>(mut row_it: BytesColumns<'a>) -> (String, String, String, usize, usize, Vec<(String, bool)>){
    let sample_id = str::from_utf8(row_it.next().unwrap()).unwrap().to_string();
    let hap_id = str::from_utf8(row_it.next().unwrap()).unwrap().to_string();
    let seq_id = str::from_utf8(row_it.next().unwrap()).unwrap().to_string();
    let seq_start = usize::from_str(str::from_utf8(row_it.next().unwrap()).unwrap()).unwrap();
    let seq_end = usize::from_str(str::from_utf8(row_it.next().unwrap()).unwrap()).unwrap();
    log::info!("processing walk {}#{}#{}:{}-{}", &sample_id, &hap_id, &seq_id, &seq_start, &seq_end);

    let walk_data = row_it.next().unwrap();
    let walk = parse_walk(walk_data.to_vec());
    (sample_id, hap_id, seq_id, seq_start, seq_end, walk)
}

pub fn parse_path_line<'a>(mut row_it: BytesColumns<'a>)  -> (String, String, String, usize, usize, Vec<(String, bool)>) {
    let path_name = str::from_utf8(row_it.next().unwrap()).unwrap().to_string();
    log::info!("processing path {}", path_name);
    let segments = path_name.split('#').collect::<Vec<&str>>();
    let sample_id = segments[0].to_string();
    let hap_id: String = if segments.len() > 1 {
            segments[1].to_string()
    } else {
        "".to_string()
    };

    let path_data = row_it.next().unwrap();
    let path = parse_path(path_data.to_vec());

    let (seq_id, seq_start, seq_end) = if segments.len() > 2 {
        let seq_coords = segments[2].split(':').collect::<Vec<&str>>();
        if seq_coords.len() > 1 {
            let start_end = seq_coords[1].split('-').collect::<Vec<&str>>();
            (seq_coords[0].to_string(), usize::from_str(start_end[0]).unwrap(), usize::from_str(start_end[1]).unwrap())
        } else {
            (seq_coords[0].to_string(), 0, path.len())
        }
    } else {
        ("".to_string(), 0, path.len())
    };
    (sample_id, hap_id, seq_id, seq_start, seq_end, path)
}

fn parse_path(path_data: Vec<u8>) -> Vec<(String, bool)> {
    let mut path: Vec<(String, bool)> = Vec::new();

    let mut cur_el: Vec<u8> = Vec::new();
    for c in path_data {
        if c == b',' {
            let sid = str::from_utf8(&cur_el[..cur_el.len() - 1]).unwrap().to_string();
            let o = cur_el.last().expect(&format!("unable to parse orientation of node {}", &sid));
            assert!(o == &b'+' || o == &b'-', "unknown orientation {} or segment {}", o, &sid);
            path.push((sid, o == &b'-'));
            cur_el.clear();
        } else {
            cur_el.push(c);
        }
    }

    if !cur_el.is_empty() {
        let sid = str::from_utf8(&cur_el[..cur_el.len() - 1]).unwrap().to_string();
        let o = cur_el.last().expect(&format!("unable to parse orientation of node {}", sid));
        assert!(o == &b'+' || o == &b'-', "unknown orientation {} or segment {}", o, sid);
        path.push((sid, o == &b'-'));
    }
    path
}

fn parse_walk(walk_data: Vec<u8>) -> Vec<(String, bool)> {
    let mut walk: Vec<(String, bool)> = Vec::new();

    let mut cur_el: Vec<u8> = Vec::new();
    for c in walk_data {
        if (c == b'>' || c == b'<') && !cur_el.is_empty() {
            let sid = str::from_utf8(&cur_el[1..]).unwrap().to_string();
            assert!(cur_el[0] == b'>' || cur_el[0] == b'<', "unknown orientation {} or segment {}", cur_el[0], sid);
            walk.push((sid, cur_el[0] == b'<'));
            cur_el.clear();
        }
        cur_el.push(c);
    }

    if !cur_el.is_empty() {
        let sid = str::from_utf8(&cur_el[1..]).unwrap().to_string();
        assert!(cur_el[0] == b'>' || cur_el[0] == b'<', "unknown orientation {} or segment {}", cur_el[0], sid);
        walk.push((sid, cur_el[0] == b'<'));
    }
    walk
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
