/* standard use */
use std::fmt;
use std::str::{self, FromStr};

/* crate use */
use std::collections::{HashMap, HashSet};
//use rayon::prelude::*;
//use rayon::par_iter::{IntoParallelIterator, ParallelIterator};
use rayon::iter::{IntoParallelIterator, ParallelIterator};

pub mod io;

const SIZE_T: usize = 1024;
struct Wrap<T>(*mut T);
unsafe impl Sync for Wrap<Vec<u32>> {}
unsafe impl Sync for Wrap<Vec<usize>> {}
unsafe impl Sync for Wrap<[Vec<u32>; SIZE_T]> {}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CoverageThreshold {
    Relative(f64),
    Absolute(usize),
}

impl fmt::Display for CoverageThreshold {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        match self {
            CoverageThreshold::Relative(c) => write!(formatter, "{}R", c)?,
            CoverageThreshold::Absolute(c) => write!(formatter, "{}A", c)?,
        }
        Ok(())
    }
}

//#[derive(Debug, Clone, PartialEq, PartialOrd, Eq, Ord)]
//pub struct Node {
//    id: String,
//    len: u32,
//}

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Hash, Eq, Ord)]
pub struct Edge {
    uid: usize,
    u_is_reverse: bool,
    vid: usize,
    v_is_reverse: bool,
}

pub struct Prep {
    pub path_segments: Vec<PathSegment>,
    pub node2id: HashMap<Vec<u8>, u32>,
    pub groups: Vec<(PathSegment, String)>,
}

pub struct NodeTable {
    pub T: [Vec<u32>; SIZE_T],
    pub ts: [Vec<u32>; SIZE_T],
}

impl NodeTable {
    pub fn new(num_walks_paths: usize) -> Self {
        Self {
            T: [(); SIZE_T].map(|_| vec![]),
            ts: [(); SIZE_T].map(|_| vec![0; num_walks_paths + 1]),
        }
    }
}

#[derive(Debug, Clone, PartialEq, PartialOrd, Hash, Eq, Ord)]
pub struct PathSegment {
    sample: String,
    haplotype: Option<String>,
    seqid: Option<String>,
    start: Option<usize>,
    end: Option<usize>,
}

impl PathSegment {
    pub fn new(
        sample: String,
        haplotype: String,
        seqid: String,
        start: Option<usize>,
        end: Option<usize>,
    ) -> Self {
        PathSegment {
            sample: sample,
            haplotype: Some(haplotype),
            seqid: Some(seqid),
            start: start,
            end: end,
        }
    }

    pub fn from_str(s: &str) -> Self {
        let mut res = PathSegment {
            sample: s.to_string(),
            haplotype: None,
            seqid: None,
            start: None,
            end: None,
        };

        let segments = s.split('#').collect::<Vec<&str>>();
        match segments.len() {
            3 => {
                res.sample = segments[0].to_string();
                res.haplotype = Some(segments[1].to_string());
                let seq_coords = segments[2].split(':').collect::<Vec<&str>>();
                res.seqid = Some(seq_coords[0].to_string());
                if seq_coords.len() == 2 {
                    let start_end = seq_coords[1].split('-').collect::<Vec<&str>>();
                    res.start = usize::from_str(start_end[0]).ok();
                    res.end = usize::from_str(start_end[1]).ok();
                } else {
                    assert!(
                        seq_coords.len() == 1,
                        r"unknown format, expected string of kind \w#\w(#\w)?:\d-\d, but got {}",
                        &s
                    );
                }
            }
            2 => {
                res.sample = segments[0].to_string();
                let seq_coords = segments[1].split(':').collect::<Vec<&str>>();
                res.seqid = Some(seq_coords[0].to_string());
                if seq_coords.len() == 2 {
                    let start_end = seq_coords[1].split('-').collect::<Vec<&str>>();
                    res.start = usize::from_str(start_end[0]).ok();
                    res.end = usize::from_str(start_end[1]).ok();
                } else {
                    assert!(
                        seq_coords.len() == 1,
                        r"unknown format, expected string of kind \w#\w(#\w)?:\d-\d, but got {}",
                        &s
                    );
                }
            }
            1 => {
                res.sample = segments[0].to_string();
            }
            _ => (),
        }
        res
    }

    pub fn id(&self) -> String {
        if self.haplotype.is_some() {
            format!(
                "{}#{}{}",
                self.sample,
                self.haplotype.as_ref().unwrap(),
                if self.seqid.is_some() {
                    "#".to_owned() + self.seqid.as_ref().unwrap().as_str()
                } else {
                    "".to_string()
                }
            )
        } else if self.seqid.is_some() {
            format!(
                "{}#*#{}",
                self.sample,
                self.seqid.as_ref().unwrap().as_str()
            )
        } else {
            self.sample.clone()
        }
    }

    pub fn coords(&self) -> Option<(usize, usize)> {
        if self.start.is_some() {
            Some((self.start.unwrap(), self.end.unwrap()))
        } else {
            None
        }
    }
}

impl fmt::Display for PathSegment {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        if let Some((start, end)) = self.coords() {
            write!(formatter, "{}:{}-{}", self.id(), start, end)?;
        } else {
            write!(formatter, "{}", self.id())?;
        }
        Ok(())
    }
}

//impl Hash for PathSegment {
//    fn hash<H: Hasher>(&self, state: &mut H) {
//        format!(
//                "{}#{}#{}:{}-{}",
//                self.sample,
//                if let Some(hapid) = &self.haplotype {
//                    hapid.as_str()
//                } else {
//                    "*"
//                },
//                if let Some(seqid) = &self.seqid {
//                    seqid.as_str()
//                } else {
//                    "*"
//                },
//                if let Some(start) = &self.start {
//                    start.to_string()
//                } else {
//                    "*".to_string()
//                },
//                if let Some(end) = &self.end {
//                    end.to_string()
//                } else {
//                    "*".to_string()
//                }
//            ).hash(state);
//    }
//}

//impl Node {
//    pub fn new(id: String, len: u32) -> Self {
//        Self { id: id, len: len }
//    }
//
//    pub fn id(self) -> String {
//        self.id
//    }
//
//    pub fn len(self) -> u32 {
//        self.len
//    }
//}
//
//impl Hash for Node {
//    fn hash<H: Hasher>(&self, state: &mut H) {
//        self.id.hash(state);
//    }
//}

impl Edge {
    #[inline]
    pub fn new(id1: usize, is_reverse1: bool, id2: usize, is_reverse2: bool) -> Self {
        let (uid, u_is_reverse, vid, v_is_reverse) =
            Edge::canonize(id1, is_reverse1, id2, is_reverse2);
        Self {
            uid,
            u_is_reverse,
            vid,
            v_is_reverse,
        }
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
    pub countable: Vec<T>,
    pub groups: Vec<String>,
}

impl Abacus<u32> {
    pub fn from_gfa<R: std::io::Read>(data: &mut std::io::BufReader<R>, prep: Prep) -> Self {
        log::info!("parsing path + walk sequences");
        let node_table = io::parse_gfa_nodecount(data, &prep);
        log::info!("counting abacus entries..");
        let mut countable: Vec<u32> = vec![0; prep.node2id.len()];
        let mut last: Vec<usize> = vec![0; prep.node2id.len()];

        let mut order_path: Vec<usize> = vec![0; prep.path_segments.len()];
        // Dummy order
        for i in 0..order_path.len() {
            order_path[i] = i;
        }

        let mut groups = Vec::new();
        for (path_id, group_id) in Abacus::get_path_order(&prep) {
            if groups.is_empty() || groups.last().unwrap() != group_id {
                groups.push(group_id.to_string());
            }
            Abacus::count_nodes(
                &mut countable,
                &mut last,
                &node_table,
                path_id,
                groups.len() - 1,
            );
        }

        Abacus {
            countable: countable,
            groups: groups,
        }
    }

    fn count_nodes(
        countable: &mut Vec<u32>,
        last: &mut Vec<usize>,
        node_table: &NodeTable,
        path_id: usize,
        group_id: usize,
    ) {
        let countable_ptr = Wrap(countable);
        let last_ptr = Wrap(last);

        // Parallel node counting
        (0..SIZE_T).into_par_iter().for_each(|i| {
            //Abacus::add_count(i, path_id, &mut countable, &mut last, &node_table);
            let start = node_table.ts[i][path_id] as usize;
            let end = node_table.ts[i][path_id + 1] as usize;
            for j in start..end {
                let sid = node_table.T[i][j] as usize;
                unsafe {
                    if last[sid] != group_id {
                        (*countable_ptr.0)[sid] += 1;
                        (*last_ptr.0)[sid] = group_id;
                    }
                }
            }
        });
    }

    fn get_path_order<'a>(prep: &'a Prep) -> Vec<(usize, &'a str)> {
        // orders the pathsegments in prep.path_segments by the order given in prep.groups
        // the returned vector maps indices of prep_path_segments to the group identifier

        let mut group_order = Vec::new();
        let mut group_to_paths: HashMap<&str, Vec<&PathSegment>> = HashMap::default();

        let mut path_to_id: HashMap<&PathSegment, usize> = HashMap::default();
        prep.path_segments.iter().enumerate().for_each(|(i, s)| {
            path_to_id.insert(s, i);
        });

        prep.groups.iter().for_each(|(k, v)| {
            group_to_paths
                .entry(&v[..])
                .or_insert({
                    group_order.push(&v[..]);
                    Vec::new()
                })
                .push(k)
        });

        let mut res = Vec::with_capacity(prep.path_segments.len());
        let empty: Vec<&PathSegment> = Vec::new();
        for g in group_order.into_iter() {
            res.extend(
                group_to_paths
                    .get(g)
                    .unwrap()
                    .iter()
                    .map(|x| (*path_to_id.get(x).unwrap(), g)),
            );
        }

        res
    }

    pub fn construct_hist(&self) -> Vec<u32> {
        let mut hist: Vec<u32> = vec![0; self.groups.len()];
        for iter in self.countable.iter() {
            hist[*iter as usize] += 1;
        }
        hist
    }
}
