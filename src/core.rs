/* standard use */
use std::hash::{Hash, Hasher};
use std::str;

/* crate use */
use quick_csv::Csv;
use rustc_hash::{FxHashMap, FxHashSet};
use std::collections::HashMap;

mod io;

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Eq, Ord)]
pub struct Node {
    id: u32,
    len: u32
}

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Hash, Eq, Ord)]
pub struct Edge {
    uid: usize,
    u_is_reverse: bool,
    vid: usize,
    v_is_reverse: bool
}

impl Hash for Node {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}


impl Node {
    pub fn new(id: u32, length: u32) -> Self {
        Self { id: id, len: length}
    }

    pub fn id(self) -> u32 {
        self.id
    }

    pub fn len(self) -> u32 {
        self.len
    }
}

impl Edge {
    #[inline]
    pub fn new(id1: usize, is_reverse1: bool, id2: usize, is_reverse2: bool) -> Self {
        let (uid, u_is_reverse, vid, v_is_reverse) =
            Edge::canonize(id1, is_reverse1, id2, is_reverse2);
        Self{ uid, u_is_reverse, vid, v_is_reverse }
    }

    #[inline]
    fn canonize(
        id1: usize,
        is_reverse1: bool,
        id2: usize,
        is_reverse2: bool,
    ) -> (usize, bool, usize, bool) {
        if (is_reverse1 && is_reverse2) || (is_reverse1 != is_reverse2 && id1 > id2) {
            (id2, !is_reverse2, id1, !is_reverse1)
        } else {
            (id1, is_reverse1, id2, is_reverse2)
        }
    }

    pub fn uid(self) -> usize {
        self.uid
    }

    pub fn u_is_reverse(self) -> bool {
        self.u_is_reverse
    }

    pub fn vid(self) -> usize {
        self.vid
    }

    pub fn v_is_reverse(self) -> bool {
        self.v_is_reverse
    }

}

#[derive(Debug, Clone)]
pub struct Abacus<T> {
    pub countable2path: FxHashMap<T, Vec<usize>>,
    pub paths: Vec<(String, String, String, usize, usize)>,
}

impl Abacus<Node> {
    pub fn from_gfa<R: std::io::Read>(data: &mut std::io::BufReader<R>) -> Self {
        let mut countable2path: FxHashMap<Node, Vec<usize>> = FxHashMap::default();
        let mut paths: Vec<(String, String, String, usize, usize)> = Vec::new();

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
                countable2path.insert(Node::new(node_count -1, 1), Vec::new());
            } else if fst_col == &[b'W'] {
                let (sample_id, hap_id, seq_id, seq_start, seq_end, walk) =
                    io::parse_walk_line(row_it);
                paths.push((sample_id, hap_id, seq_id, seq_start, seq_end));
                walk.into_iter().for_each(|(node, _)| {
                    countable2path
                        .get_mut(&Node::new(
                            *node2id.get(&node).expect(&format!("unknown node {}", &node)),
                            1,
                        )).expect(&format!("unknown node {}", &node))
                        .push(paths.len());
                });
            } else if &[b'P'] == fst_col {
                let (sample_id, hap_id, seq_id, seq_start, seq_end, path) =
                    io::parse_path_line(row_it);
                paths.push((sample_id, hap_id, seq_id, seq_start, seq_end));
                let l = paths.len();
                let cur_len = countable2path.len();
                log::debug!("updating count data structure..");
                for (node, _) in path.into_iter() {
                    countable2path
                        .entry(Node::new(
                            *node2id.get(&node).expect(&format!("unkown node {}", &node)),
                            1,
                        ))
                        .or_insert(Vec::new())
                        .push(l);
                }
                log::debug!(
                    "done; data structure has now {} more elements",
                    countable2path.len() - cur_len
                );
            }
        }

        Abacus {
            countable2path,
            paths,
        }
    }
}

pub fn count_path_walk_lines<R: std::io::Read>(data: &mut std::io::BufReader<R>) -> usize {
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
