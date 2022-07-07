/* standard use */

pub mod io {

    use std::str::{self, FromStr};

    /* crate use */
//    use gfa::{gfa::GFA, parser::GFAParser};
    use handlegraph::handle::Handle;
    use quick_csv::Csv;
    use rustc_hash::{FxHashMap};

    pub fn parse_gfa<R: std::io::Read>(
        mut data: std::io::BufReader<R>,
        merge_chr: bool,
        walk_field_sep: &String,
    ) -> FxHashMap<String, FxHashMap<String, Vec<Vec<Handle>>>> {
        let mut res: FxHashMap<String, FxHashMap<String, Vec<Vec<Handle>>>> = FxHashMap::default();
        let reader = Csv::from_reader(&mut data)
            .delimiter(b'\t')
            .flexible(true)
            .has_header(false);

        for row in reader.into_iter() {
            let row = row.unwrap();
            let mut row_it = row.bytes_columns();
            let fst_col = row_it.next().unwrap();
            if &[b'W'] == fst_col {
                let sample_id = str::from_utf8(row_it.next().unwrap()).unwrap();
                let mut hap_id = str::from_utf8(row_it.next().unwrap()).unwrap();
                if merge_chr && hap_id.to_lowercase().starts_with("chr") {
                    hap_id = "chromosomes";
                }
                let seq_id = str::from_utf8(row_it.next().unwrap()).unwrap();
                let seq_start = str::from_utf8(row_it.next().unwrap()).unwrap();
                let seq_end = str::from_utf8(row_it.next().unwrap()).unwrap();
                let walk_ident = format!(
                    "{}{}{}{}{}:{}-{}",
                    sample_id, walk_field_sep, hap_id, walk_field_sep, seq_id, seq_start, seq_end
                );
                log::info!("processing walk {}", walk_ident);

                let walk_data = row_it.next().unwrap();
                let walk = parse_walk(walk_data.to_vec())
                    .unwrap_or_else(|e| panic!("Unable to parse walk for {}: {}", &walk_ident, e));
                res.entry(sample_id.to_lowercase().to_string())
                    .or_insert(FxHashMap::default())
                    .entry(hap_id.to_string())
                    .or_insert(Vec::new())
                    .push(walk);
            } else if &[b'P'] == fst_col {
                let path_name = str::from_utf8(row_it.next().unwrap()).unwrap();
                let segments = path_name.split(walk_field_sep).collect::<Vec<&str>>();
                let sample_id =
                    segments[0].to_string().split(".").collect::<Vec<&str>>()[0].to_string();
                let hap_id: String = if segments.len() > 1 {
                    if merge_chr && segments[1].to_lowercase().starts_with("chr") {
                        "chromosomes".to_string()
                    } else {
                        segments[1].to_string()
                    }
                } else {
                    "".to_string()
                };
                let path_data = row_it.next().unwrap();

                log::info!("processing path {}", path_name);
                let path = parse_path(path_data.to_vec())
                    .unwrap_or_else(|e| panic!("Unable to parse walk for {}: {}", &path_name, e));
                res.entry(sample_id.to_lowercase().to_string())
                    .or_insert(FxHashMap::default())
                    .entry(hap_id.to_string())
                    .or_insert(Vec::new())
                    .push(path);
            }
        }

        res
    }

    fn parse_path(path_data: Vec<u8>) -> Result<Vec<Handle>, String> {
        let mut path: Vec<Handle> = Vec::new();

        let mut cur_el: Vec<u8> = Vec::new();
        for c in path_data {
            if c == b',' {
                let sid =
                    usize::from_str(str::from_utf8(&cur_el[..cur_el.len() - 1]).unwrap()).unwrap();
                let is_rev = match cur_el.last().unwrap() {
                    b'+' => Ok(false),
                    b'-' => Ok(true),
                    _ => Err(format!(
                        "unknown orientation '{}' of segment {}",
                        cur_el.last().unwrap(),
                        sid
                    )),
                };
                if is_rev.is_ok() {
                    path.push(Handle::pack(sid, is_rev.unwrap()));
                } else {
                    return Err(is_rev.err().unwrap());
                }
                cur_el.clear();
            } else {
                cur_el.push(c);
            }
        }

        if !cur_el.is_empty() {
            let sid =
                usize::from_str(str::from_utf8(&cur_el[..cur_el.len() - 1]).unwrap()).unwrap();
            let is_rev = match cur_el.last().unwrap() {
                b'+' => Ok(false),
                b'-' => Ok(true),
                _ => Err(format!(
                    "unknown orientation '{}' of segment {}",
                    cur_el.last().unwrap(),
                    sid
                )),
            };
            if is_rev.is_ok() {
                path.push(Handle::pack(sid, is_rev.unwrap()));
            } else {
                return Err(is_rev.err().unwrap());
            }
        }
        Ok(path)
    }

    fn parse_walk(walk_data: Vec<u8>) -> Result<Vec<Handle>, String> {
        let mut walk: Vec<Handle> = Vec::new();

        let mut cur_el: Vec<u8> = Vec::new();
        for c in walk_data {
            if (c == b'>' || c == b'<') && !cur_el.is_empty() {
                let sid = usize::from_str(str::from_utf8(&cur_el[1..]).unwrap()).unwrap();
                let is_rev = match cur_el[0] {
                    b'>' => Ok(false),
                    b'<' => Ok(true),
                    _ => Err(format!(
                        "unknown orientation '{}' of segment {}",
                        cur_el[0], sid
                    )),
                };
                if is_rev.is_ok() {
                    walk.push(Handle::pack(sid, is_rev.unwrap()));
                } else {
                    return Err(is_rev.err().unwrap());
                }
                cur_el.clear();
            }
            cur_el.push(c);
        }

        if !cur_el.is_empty() {
            let sid = usize::from_str(str::from_utf8(&cur_el[1..]).unwrap()).unwrap();
            let is_rev = match cur_el[0] {
                b'>' => Ok(false),
                b'<' => Ok(true),
                _ => Err(format!(
                    "unknown orientation '{}' of segment {}",
                    cur_el[0], sid
                )),
            };
            if is_rev.is_ok() {
                walk.push(Handle::pack(sid, is_rev.unwrap()));
            } else {
                return Err(is_rev.err().unwrap());
            }
        }
        Ok(walk)
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
}
