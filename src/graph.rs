/* standard use */
use once_cell::sync::Lazy;
use regex::Regex;
use std::collections::HashMap;
use std::fmt;
use std::str::{self, FromStr};
use std::io::Error;

/* private use */
use crate::io;
use crate::util::{CountType, ItemIdSize};
use crate::util::*;

static PATHID_PANSN: Lazy<Regex> = Lazy::new(|| Regex::new(r"^([^#]+)(#[^#]+)?(#[^#]+)?$").unwrap());
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
    pub degree: Option<Vec<u32>>,
}

impl GraphAuxilliary {
    pub fn new(
        node2id: HashMap<Vec<u8>, ItemId>,
        node_len_ary: Vec<ItemIdSize>,
        edge2id: Option<HashMap<Edge, ItemId>>,
        path_segments: Vec<PathSegment>,
        node_count: usize,
        edge_count: usize,
        degree: Option<Vec<u32>>,
    ) -> Self {
        Self {
            node2id,
            node_len_ary,
            edge2id,
            path_segments,
            node_count,
            edge_count,
            degree,
        }
    }

    pub fn from_gfa<R: std::io::Read>(
        data: &mut std::io::BufReader<R>,
        count_type: CountType,
    ) -> Result<Self, Error> {
        log::info!("constructing indexes for node/edge IDs, node lengths, and P/W lines..");
        let (node2id, node_len_ary, edges, path_segments) = 
            io::parse_graph_aux(data,(count_type == CountType::Edge) | (count_type == CountType::All))?;
        let node_count = node2id.len();
        let (edge2id, edge_count, degree) = Self::construct_edgemap(edges, &node2id);

        log::info!(
            "found {} paths/walks and {} nodes{}",
            path_segments.len(),
            node_count,
            if edge_count != 0{
                format!(" {} edges", edge_count)
            } else {
                String::new()
            }
        );
        if path_segments.len() == 0 {
            log::warn!("graph does not contain any annotated paths (P/W lines)");
        }

        Ok(Self::new(
            node2id,
            node_len_ary,
            edge2id,
            path_segments,
            node_count,
            edge_count,
            degree,
        ))
    }

    pub fn node_len(&self, v: &ItemId) -> ItemIdSize {
        self.node_len_ary[v.0 as usize]
    }

    pub fn graph_info(&self) {
        let degree = self.degree.as_ref().unwrap();
        let mut node_len_sorted = self.node_len_ary[1..].to_vec();
        node_len_sorted.sort_by(|a, b| b.cmp(a)); // decreasing, for N50
        println!("Graph Info:");
        println!("\tNumber of Nodes: {}", self.node_count);
        println!("\tNumber of Edges: {}", self.edge_count);
        println!("\tAverage Degree (undirected): {}", average(&degree[1..]));
        println!(
            "\tMax Degree (undirected): {}",
            degree[1..].iter().max().unwrap()
        );
        println!(
            "\tMin Degree (undirected): {}",
            degree[1..].iter().min().unwrap()
        );
        println!(
            "\tNumber 0-degree Nodes: {}",
            degree[1..].iter().filter(|&x| *x == 0).count()
        );
        println!(
            "\tLargest Node (bp): {}",
            node_len_sorted.iter().max().unwrap()
        );
        println!(
            "\tShortest Node (bp): {}",
            node_len_sorted.iter().min().unwrap()
        );
        println!("\tAverage Node Length (bp): {}", average(&node_len_sorted));
        println!(
            "\tMedian Node Length (bp): {}",
            median_already_sorted(&node_len_sorted)
        );
        println!(
            "\tN50 Node Length (bp): {}",
            n50_already_sorted(&node_len_sorted).unwrap()
        );
        //println!("Edge-level Metrics:");
        //println!("\tEdge Length Distribution (bp): TODO");
        // DISTRIBUTIONS
        //println!("\tDegree Distribution:");
        //if let Some(hist) = graph.degree_distribution() {
        //    for i in 0..hist.len() {
        //        println!("{}:{} ",i, hist[i]);
        //    }
        //}
    }

    pub fn path_info(&self, paths_len: &Vec<u32>) {
        println!("Path/Walk Info:");
        println!("\tNumber of Paths/Walks: {}", paths_len.len());
        println!(
            "\tLongest Path/Walk (node): {}",
            paths_len.iter().max().unwrap()
        );
        println!(
            "\tShortest Path/Walk (node): {}",
            paths_len.iter().min().unwrap()
        );
        println!(
            "\tAverage Number of Nodes in Paths/Walks: {}",
            average(&paths_len)
        );
        //println!("\tDistribution of Strands in the Paths/Walks: TODO +/-");
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
    ) -> (Option<HashMap<Edge, ItemId>>, usize, Option<Vec<u32>>) {
        let mut degree: Vec<u32> = vec![0; node2id.len()+1];
        match edges {
            Some(es) => {
                let mut res = HashMap::default();
                let mut c: ItemIdSize = 1;
                for b in es {
                    let e = Edge::from_link(&b[..], node2id, true);
                    if res.contains_key(&e) {
                        log::warn!("edge {} is duplicated in GFA", &e);
                    } else {
                        degree[e.0.0 as usize] += 1;
                        //if e.0.0 != e.2.0 {
                        degree[e.2.0 as usize] += 1;
                        //}
                        res.insert(e, ItemId(c));
                        c += 1;
                    }
                }
                (Some(res), c as usize, Some(degree))
            }
            None => (None, 0, None),
        }
    }

    #[allow(dead_code)]
    pub fn degree_distribution(&self) -> Option<Vec<u32>> {
        match &self.degree {
            Some(degree) => {
                let mut hist: Vec<u32> = vec![0,1];
                for i in 1..self.node_count+1 {
                    if degree[i] as usize >= hist.len() {
                        hist.resize(degree[i] as usize +1, 0);
                    }
                    hist[degree[i] as usize] += 1
                }
                Some(hist)
            }
            None => None
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
            //log::debug!(
            //    "path id {} can be decomposed into capture groups {:?}",
            //    s,
            //    segments
            //);
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
