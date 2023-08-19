/* standard use */
use once_cell::sync::Lazy;
use regex::Regex;
use std::collections::HashMap;
use std::fmt;
use std::str::{self, FromStr};

/* private use */
use crate::io;
use crate::util::{CountType, ItemIdSize};

static PATHID_PANSN: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^([^#]+)(#[^#]+)?(#[^#]+)?$").unwrap());
static PATHID_COORDS: Lazy<Regex> = Lazy::new(|| Regex::new(r"^(.+):([0-9]+)-([0-9]+)$").unwrap());

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ItemId(pub ItemIdSize);

impl fmt::Display for ItemId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Edge(pub ItemId, pub Orientation, pub ItemId, pub Orientation);

impl Edge {
    pub fn from_link(data: &[u8], node2id: &HashMap<Vec<u8>, ItemId>, canonical: bool) -> Self {
        let (start, mut iter) = match data[0] {
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

        if canonical {
            Self::canonical(*u, o1, *v, o2)
        } else {
            Self(*u, o1, *v, o2)
        }
    }

    #[allow(dead_code)]
    pub fn normalize(&self) -> Self {
        Self::canonical(self.0, self.1, self.2, self.3)
    }

    pub fn canonical(u: ItemId, o1: Orientation, v: ItemId, o2: Orientation) -> Self {
        if u.0 > v.0 || (u.0 == v.0 && o1 == Orientation::Backward) {
            Self(v, o2.flip(), u, o1.flip())
        } else {
            Self(u, o1, v, o2)
        }
    }

    pub fn flip(&self) -> Self {
        Self(self.2, self.3.flip(), self.0, self.1.flip())
    }
}

impl fmt::Display for Edge {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}{}{}{}", self.1, self.0, self.3, self.2)
    }
}

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

impl fmt::Display for Orientation {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Orientation::Forward => write!(f, ">"),
            Orientation::Backward => write!(f, "<"),
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

    #[allow(dead_code)]
    pub fn from_lg(c: u8) -> Self {
        match c {
            b'>' => Orientation::Forward,
            b'<' => Orientation::Backward,
            _ => unreachable!("expected '>' or '<'"),
        }
    }

    #[allow(dead_code)]
    pub fn to_lg(&self) -> u8 {
        match self {
            &Orientation::Forward => b'>',
            &Orientation::Backward => b'<',
        }
    }

    pub fn flip(&self) -> Self {
        match self {
            &Orientation::Forward => Orientation::Backward,
            &Orientation::Backward => Orientation::Forward,
        }
    }
}

impl PartialEq<u8> for Orientation {
    fn eq(&self, other: &u8) -> bool {
        match other {
            &b'>' | &b'+' => self == &Orientation::Forward,
            &b'<' | &b'-' => self == &Orientation::Backward,
            _ => false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct GraphAuxilliary {
    pub node2id: HashMap<Vec<u8>, ItemId>,
    pub node_len_ary: Vec<ItemIdSize>,
    pub edge2id: Option<HashMap<Edge, ItemId>>,
    pub path_segments: Vec<PathSegment>,
    pub node_count: usize,
    pub edge_count: usize,
}

impl GraphAuxilliary {
    pub fn new(
        node2id: HashMap<Vec<u8>, ItemId>,
        node_len_ary: Vec<ItemIdSize>,
        edge2id: Option<HashMap<Edge, ItemId>>,
        path_segments: Vec<PathSegment>,
        node_count: usize,
        edge_count: usize,
    ) -> Self {
        Self {
            node2id,
            node_len_ary,
            edge2id,
            path_segments,
            node_count,
            edge_count,
        }
    }

    pub fn from_gfa<R: std::io::Read>(
        data: &mut std::io::BufReader<R>,
        index_edges: bool,
    ) -> Result<Self, std::io::Error> {
        let (node2id, node_len_ary, edges, path_segments) = io::parse_graph_aux(data, index_edges)?;
        // don't count "0" ID
        let nc = node_len_ary.len() - 1;
        let (edge2id, ec) = Self::construct_edgemap(edges, &node2id);
        Ok(Self::new(
            node2id,
            node_len_ary,
            edge2id,
            path_segments,
            nc,
            ec,
        ))
    }

    pub fn node_len(&self, v: &ItemId) -> ItemIdSize {
        self.node_len_ary[v.0 as usize]
    }

    #[allow(dead_code)]
    pub fn number_of_nodes(&self) -> usize {
        self.node_count
    }

    #[allow(dead_code)]
    pub fn number_of_edges(&self) -> usize {
        self.edge_count
    }

    pub fn number_of_items(&self, c: &CountType) -> usize {
        match c {
            &CountType::Node | &CountType::Bp => self.node_count,
            &CountType::Edge => self.edge_count,
            &CountType::All => unreachable!("inadmissible count type"),
        }
    }

    pub fn construct_edgemap(
        edges: Option<Vec<Vec<u8>>>,
        node2id: &HashMap<Vec<u8>, ItemId>,
    ) -> (Option<HashMap<Edge, ItemId>>, usize) {
        match edges {
            Some(es) => {
                let mut res = HashMap::default();
                let mut c: ItemIdSize = 0;
                for b in es {
                    let e = Edge::from_link(&b[..], node2id, true);
                    if res.contains_key(&e) {
                        log::error!("edge {} is duplicated in GFA", &e);
                    } else {
                        c += 1;
                        res.insert(e, ItemId(c));
                    }
                }
                (Some(res), c as usize)
            }
            None => (None, 0),
        }
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

        if let Some(c) = PATHID_PANSN.captures(s) {
            let segments: Vec<&str> = c.iter().filter_map(|x| x.map(|y| y.as_str())).collect();
            // first capture group is the string itself
            log::debug!(
                "path id {} can be decomposed into capture groups {:?}",
                s,
                segments
            );
            match segments.len() {
                4 => {
                    res.sample = segments[1].to_string();
                    res.haplotype = Some(segments[2][1..].to_string());
                    match PATHID_COORDS.captures(&segments[3][1..]) {
                        None => {
                            res.seqid = Some(segments[3][1..].to_string());
                        }
                        Some(cc) => {
                            log::debug!("path has coodinates {:?}", cc);
                            res.seqid = Some(cc.get(1).unwrap().as_str().to_string());
                            res.start = usize::from_str(cc.get(2).unwrap().as_str()).ok();
                            res.end = usize::from_str(cc.get(3).unwrap().as_str()).ok();
                        }
                    }
                }
                3 => {
                    res.sample = segments[1].to_string();
                    match PATHID_COORDS.captures(&segments[2][1..]) {
                        None => {
                            res.haplotype = Some(segments[2][1..].to_string());
                        }
                        Some(cc) => {
                            log::debug!("path has coodinates {:?}", cc);
                            res.haplotype = Some(cc.get(1).unwrap().as_str().to_string());
                            res.start = usize::from_str(cc.get(2).unwrap().as_str()).ok();
                            res.end = usize::from_str(cc.get(3).unwrap().as_str()).ok();
                        }
                    }
                }
                2 => {
                    if let Some(cc) = PATHID_COORDS.captures(segments[1]) {
                        log::debug!("path has coodinates {:?}", cc);
                        res.sample = cc.get(1).unwrap().as_str().to_string();
                        res.start = usize::from_str(cc.get(2).unwrap().as_str()).ok();
                        res.end = usize::from_str(cc.get(3).unwrap().as_str()).ok();
                    }
                }
                _ => (),
            }
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

    pub fn clear_coords(&self) -> Self {
        Self {
            sample: self.sample.clone(),
            haplotype: self.haplotype.clone(),
            seqid: self.seqid.clone(),
            start: None,
            end: None,
        }
    }

    pub fn coords(&self) -> Option<(usize, usize)> {
        if self.start.is_some() && self.end.is_some() {
            Some((self.start.unwrap(), self.end.unwrap()))
        } else {
            None
        }
    }

    #[allow(dead_code)]
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
