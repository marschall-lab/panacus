/* standard use */
use std::collections::HashMap;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::str::{self, FromStr};

/* private use */
use crate::io;

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ItemId(pub u32);

impl fmt::Display for ItemId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Default, Debug, Clone, PartialOrd, Ord)]
pub struct Edge(pub ItemId, pub Orientation, pub ItemId, pub Orientation);

impl Edge {
    pub fn from_link(data: &[u8], node2id: &HashMap<Vec<u8>, ItemId>) -> Self {
        let (mut start, mut iter) = match data[0] {
            b'L' => (2, data[2..].iter()),
            _ => (0, data.iter()),
        };

        let end = start + iter.position(|&x| x == b'\t').unwrap();
        let u = node2id.get(&data[start..end]).expect(&format!(
            "unknown node {}",
            str::from_utf8(&data[start..end]).unwrap()
        ));

        // we know that 3rd colum is either '+' or '-', so it has always length 1; still, we
        // need to advance in the buffer (and  therefore call iter.position(..))
        iter.position(|&x| x == b'\t');
        let o1 = Orientation::from_pm(data[end + 1]);

        let start = end + 3;
        let end = start + iter.position(|&x| x == b'\t').unwrap();

        let v = node2id.get(&data[start..end]).expect(&format!(
            "unknown node {}",
            str::from_utf8(&data[start..end]).unwrap()
        ));
        let o2 = Orientation::from_pm(data[end + 1]);

        Self(*u, o1, *v, o2)
    }

    pub fn normalize(&self) -> Self {
        if self.3 == Orientation::Backward && (self.1 == Orientation::Backward || self.0 > self.2) {
            self.flip()
        } else {
            self.clone()
        }
    }

    pub fn flip(&self) -> Self {
        Self(self.2, self.3.flip(), self.0, self.1.flip())
    }

    fn eq_individual(&self, other: &Self) -> bool {
        self.0 == other.0 && self.1 == other.1 && self.2 == other.2 && self.3 == other.3
    }
}

impl fmt::Display for Edge {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}{}{}{}", self.1, self.0, self.3, self.2)
    }
}

impl Hash for Edge {
    fn hash<H: Hasher>(&self, state: &mut H) {
        format!("{}", self.normalize()).hash(state);
    }
}

impl PartialEq for Edge {
    fn eq(&self, other: &Self) -> bool {
        self.eq_individual(other) || ((self.0 != self.2 || self.1 == self.3) && self.flip().eq_individual(other))
    }

}

impl Eq for Edge {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Orientation {
    Forward,
    Backward,
}

impl Default for Orientation {
    fn default() -> Self {
        Orientation::Forward
    }
}

impl Orientation {
    pub fn flip(&self) -> Self {
        match &self {
            Orientation::Forward => Orientation::Backward,
            Orientation::Backward => Orientation::Forward,
        }
    }
}

impl fmt::Display for Orientation {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Orientation::Forward => write!(f, ">"),
            Orientation::Backward => write!(f, ">"),
        }
    }
}

impl Orientation {
    pub fn from_pm(c: u8) -> Self {
        match c {
            b'+' => Orientation::Forward,
            b'-' => Orientation::Backward,
            _ => unreachable!("expected '+' or '-', but got {}", c as char),
        }
    }

    pub fn from_lg(c: u8) -> Self {
        match c {
            b'>' => Orientation::Forward,
            b'<' => Orientation::Backward,
            _ => unreachable!("expected '>' or '<'"),
        }
    }
}

pub struct GraphAuxilliary {
    pub node2id: HashMap<Vec<u8>, ItemId>,
    node_len_ary: Vec<u32>,
    pub edge2id: Option<HashMap<Edge, ItemId>>,
    pub path_segments: Vec<PathSegment>,
}

impl GraphAuxilliary {
    pub fn new(
        node2id: HashMap<Vec<u8>, ItemId>,
        node_len_ary: Vec<u32>,
        edge2id: Option<HashMap<Edge, ItemId>>,
        path_segments: Vec<PathSegment>,
    ) -> Self {
        Self {
            node2id,
            node_len_ary,
            edge2id,
            path_segments,
        }
    }

    pub fn from_gfa<R: std::io::Read>(data: &mut std::io::BufReader<R>, index_edges: bool) -> Self {
        let (node2id, node_len_ary, edges, path_segments) = io::parse_graph_aux(data, index_edges);
        let edge2id = Self::construct_edgemap(edges, &node2id);
        Self::new(node2id, node_len_ary, edge2id, path_segments)
    }

    pub fn node_len(&self, v: &ItemId) -> u32 {
        self.node_len_ary[v.0 as usize]
    }

    pub fn number_of_nodes(&self) -> usize {
        self.node_len_ary.len()
    }

    pub fn construct_edgemap(
        edges: Option<Vec<Vec<u8>>>,
        node2id: &HashMap<Vec<u8>, ItemId>,
    ) -> Option<HashMap<Edge, ItemId>> {
        edges.map(|es| {
            es.into_iter()
                .enumerate()
                .map(|(i, e)| (Edge::from_link(&e[..], node2id), ItemId(i as u32 + 1)))
                .collect()
        })
    }
}

#[derive(Debug, Clone, PartialEq, PartialOrd, Hash, Eq, Ord)]
pub struct PathSegment {
    pub sample: String,
    pub haplotype: Option<String>,
    pub seqid: Option<String>,
    pub start: Option<usize>,
    pub end: Option<usize>,
}

impl PathSegment {
    pub fn new(
        sample: String,
        haplotype: String,
        seqid: String,
        start: Option<usize>,
        end: Option<usize>,
    ) -> Self {
        Self {
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
                res.haplotype = Some(seq_coords[0].to_string());
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
                let seq_coords = segments[0].split(':').collect::<Vec<&str>>();
                if seq_coords.len() == 2 {
                    res.sample = seq_coords[0].to_string();
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

    pub fn covers(&self, other: &PathSegment) -> bool {
        self.sample == other.sample
            && (self.haplotype == other.haplotype
                || (self.haplotype.is_none()
                    && self.seqid.is_none()
                    && self.start.is_none()
                    && self.end.is_none()))
            && (self.seqid == other.seqid
                || (self.seqid.is_none() && self.start.is_none() && self.end.is_none()))
            && (self.start == other.start
                || self.start.is_none()
                || (other.start.is_some() && self.start.unwrap() <= other.start.unwrap()))
            && (self.end == other.end
                || self.end.is_none()
                || (other.end.is_some() && self.end.unwrap() >= other.end.unwrap()))
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
