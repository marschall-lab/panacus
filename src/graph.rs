/* standard use */
use std::fmt;
use std::str::{self, FromStr};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Threshold {
    Relative(f64),
    Absolute(usize),
}

//#[derive(Debug, Clone, PartialEq, PartialOrd, Eq, Ord)]
//pub struct Node {
//    id: String,
//    len: u32,
//}

//#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Hash, Eq, Ord)]
//pub struct Edge {
//    uid: usize,
//    u_is_reverse: bool,
//    vid: usize,
//    v_is_reverse: bool,
//}

#[derive(Debug, Clone, PartialEq, PartialOrd, Hash, Eq, Ord)]
pub struct PathSegment {
    pub sample: String,
    pub haplotype: Option<String>,
    pub seqid: Option<String>,
    pub start: Option<usize>,
    pub end: Option<usize>,
}

impl fmt::Display for Threshold {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Threshold::Relative(c) => write!(formatter, "{}R", c)?,
            Threshold::Absolute(c) => write!(formatter, "{}A", c)?,
        }
        Ok(())
    }
}

//impl Edge {
//    #[inline]
//    pub fn new(id1: usize, is_reverse1: bool, id2: usize, is_reverse2: bool) -> Self {
//        let (uid, u_is_reverse, vid, v_is_reverse) =
//            Edge::canonize(id1, is_reverse1, id2, is_reverse2);
//        Self {
//            uid,
//            u_is_reverse,
//            vid,
//            v_is_reverse,
//        }
//    }
//
//    #[inline]
//    fn canonize(
//        id1: usize,
//        is_reverse1: bool,
//        id2: usize,
//        is_reverse2: bool,
//    ) -> (usize, bool, usize, bool) {
//        if (is_reverse1 && is_reverse2) || (is_reverse1 != is_reverse2 && id1 > id2) {
//            (id2, !is_reverse2, id1, !is_reverse1)
//        } else {
//            (id1, is_reverse1, id2, is_reverse2)
//        }
//    }
//
//    pub fn uid(self) -> usize {
//        self.uid
//    }
//
//    pub fn u_is_reverse(self) -> bool {
//        self.u_is_reverse
//    }
//
//    pub fn vid(self) -> usize {
//        self.vid
//    }
//
//    pub fn v_is_reverse(self) -> bool {
//        self.v_is_reverse
//    }
//}

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
