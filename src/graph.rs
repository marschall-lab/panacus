/* standard use */
use once_cell::sync::Lazy;
use regex::Regex;
use std::collections::{HashMap, HashSet};
use std::io::BufRead;
use std::str::{self, FromStr};
use std::{fmt, usize};

/* private use */
use crate::io::bufreader_from_compressed_gfa;
use crate::util::*;
use crate::util::{CountType, ItemIdSize};

static PATHID_PANSN: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^([^#]+)(#[^#]+)?(#[^#].*)?$").unwrap());
static PATHID_COORDS: Lazy<Regex> = Lazy::new(|| Regex::new(r"^(.+):([0-9]+)-([0-9]+)$").unwrap());

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
    pub fn to_lg(self) -> char {
        match self {
            Orientation::Forward => '>',
            Orientation::Backward => '<',
        }
    }

    pub fn flip(&self) -> Self {
        match *self {
            Orientation::Forward => Orientation::Backward,
            Orientation::Backward => Orientation::Forward,
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
        let u = node2id.get(&data[start..end]).unwrap_or_else(|| {
            panic!(
                "unknown node {}",
                str::from_utf8(&data[start..end]).unwrap()
            )
        });

        // we know that 3rd colum is either '+' or '-', so it has always length 1; still, we
        // need to advance in the buffer (and  therefore call iter.position(..))
        iter.position(|&x| x == b'\t');
        let o1 = Orientation::from_pm(data[end + 1]);

        let start = end + 3;
        let end = start + iter.position(|&x| x == b'\t').unwrap();

        let v = node2id.get(&data[start..end]).unwrap_or_else(|| {
            panic!(
                "unknown node {}",
                str::from_utf8(&data[start..end]).unwrap()
            )
        });
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

pub fn get_extremities(node_dna: &[u8], k: usize) -> (u64, u64) {
    let left = kmer_u8_to_u64(&node_dna[0..k]);
    let right = kmer_u8_to_u64(&node_dna[node_dna.len() - k..node_dna.len()]);
    (left, right)
}

#[derive(Debug, Clone)]
pub struct GraphAuxilliary {
    pub node2id: HashMap<Vec<u8>, ItemId>,
    pub node_lens: Vec<u32>,
    pub edge2id: Option<HashMap<Edge, ItemId>>,
    pub path_segments: Vec<PathSegment>,
    pub node_count: usize,
    pub edge_count: usize,
    pub degree: Option<Vec<u32>>,
    // pub extremities: Option<Vec<(u64, u64)>>,
}

impl GraphAuxilliary {
    pub fn from_gfa(gfa_file: &str, count_type: CountType) -> Self {
        let (node2id, path_segments, node_lens, _extremities) =
            Self::parse_nodes_gfa(gfa_file, None);
        let index_edges: bool = (count_type == CountType::Edge) | (count_type == CountType::All);
        let (edge2id, edge_count, degree) = if index_edges {
            let (edge2id, edge_count, degree) = Self::parse_edge_gfa(gfa_file, &node2id);
            (Some(edge2id), edge_count, Some(degree))
        } else {
            (None, 0, None)
        };
        let node_count = node2id.len();

        Self {
            node2id,
            node_lens,
            edge2id,
            path_segments,
            node_count,
            edge_count,
            degree,
            // extremities,
        }
    }

    // pub fn from_cdbg_gfa(gfa_file: &str, k: usize) -> Self {
    //     let (node2id, path_segments, node_lens, extremities) =
    //         Self::parse_nodes_gfa(gfa_file, Some(k));
    //     let (edge2id, edge_count, degree) = (None, 0, None);
    //     let node_count = node2id.len();

    //     Self {
    //         node2id,
    //         node_lens,
    //         edge2id,
    //         path_segments,
    //         node_count,
    //         edge_count,
    //         degree,
    //         extremities,
    //     }
    // }

    pub fn node_len(&self, v: &ItemId) -> u32 {
        self.node_lens[v.0 as usize]
    }

    pub fn info(
        &self,
        paths_len: &HashMap<PathSegment, (u32, u32)>,
        groups: &HashMap<PathSegment, String>,
        has_groups: bool,
    ) -> Info {
        if has_groups {
            Info {
                graph_info: self.graph_info(groups),
                path_info: self.path_info(paths_len),
                group_info: Some(self.group_info(paths_len, groups)),
            }
        } else {
            Info {
                graph_info: self.graph_info(groups),
                path_info: self.path_info(paths_len),
                group_info: None,
            }
        }
    }

    pub fn group_info(
        &self,
        paths_len: &HashMap<PathSegment, (u32, u32)>,
        groups: &HashMap<PathSegment, String>,
    ) -> GroupInfo {
        let mut group_map: HashMap<String, (u32, u32)> = HashMap::new();
        for (k, v) in paths_len {
            let group = if !groups.contains_key(k) {
                let k = k.clear_coords();
                if !groups.contains_key(&k) {
                    log::debug!("Path segment {:?} not in groups", k);
                    continue;
                }
                groups[&k].clone()
            } else {
                groups[k].clone()
            };
            let tmp = group_map.entry(group).or_insert((0, 0));
            tmp.0 += v.0;
            tmp.1 += v.1;
        }

        GroupInfo { groups: group_map }
    }

    fn connected_components(&self) -> Vec<u32> {
        let mut component_lengths = Vec::new();
        let mut visited: HashSet<ItemId> = HashSet::new();
        let edges: HashMap<ItemId, Vec<ItemId>> = match &self.edge2id {
            Some(edge_map) => edge_map
                .keys()
                .map(|x| (x.0, x.2))
                .chain(edge_map.keys().map(|x| (x.2, x.0)))
                .fold(HashMap::new(), |mut acc, (k, v)| {
                    acc.entry(k).and_modify(|x| x.push(v)).or_insert(vec![v]);
                    acc
                }),
            None => HashMap::new(),
        };
        let nodes: Vec<ItemId> = self.node2id.values().copied().collect();
        for node in &nodes {
            if !visited.contains(node) {
                component_lengths.push(Self::dfs(&edges, *node, &mut visited));
            }
        }
        component_lengths
    }

    fn dfs(
        edges: &HashMap<ItemId, Vec<ItemId>>,
        node: ItemId,
        visited: &mut HashSet<ItemId>,
    ) -> u32 {
        let mut s = Vec::new();
        let mut length = 0;
        s.push(node);
        while let Some(v) = s.pop() {
            if visited.contains(&v) {
                continue;
            }
            visited.insert(v);
            length += 1;
            if !edges.contains_key(&v) {
                continue;
            }
            for neigh in &edges[&v] {
                if !visited.contains(neigh) {
                    s.push(*neigh);
                }
            }
        }
        length
    }

    pub fn graph_info(&self, groups: &HashMap<PathSegment, String>) -> GraphInfo {
        let degree = self.degree.as_ref().unwrap();
        let mut node_lens_sorted = self.node_lens[1..].to_vec();
        node_lens_sorted.sort_by(|a, b| b.cmp(a)); // decreasing, for N50
        let mut components = self.connected_components();
        components.sort();

        GraphInfo {
            node_count: self.node_count,
            edge_count: self.edge_count,
            average_degree: averageu32(&degree[1..]),
            max_degree: *degree[1..].iter().max().unwrap(),
            min_degree: *degree[1..].iter().min().unwrap(),
            number_0_degree: degree[1..].iter().filter(|&x| *x == 0).count(),
            connected_components: components.len() as u32,
            largest_component: *components.iter().max().unwrap_or(&0),
            smallest_component: *components.iter().min().unwrap_or(&0),
            median_component: median_already_sorted(&components),
            largest_node: *node_lens_sorted.iter().max().unwrap(),
            shortest_node: *node_lens_sorted.iter().min().unwrap(),
            average_node: averageu32(&node_lens_sorted),
            median_node: median_already_sorted(&node_lens_sorted),
            n50_node: n50_already_sorted(&node_lens_sorted).unwrap(),
            basepairs: self.node_lens.iter().sum(),
            group_count: groups.values().collect::<HashSet<_>>().len(),
        }
    }

    pub fn path_info(&self, paths_len: &HashMap<PathSegment, (u32, u32)>) -> PathInfo {
        //println!("\tDistribution of Strands in the Paths/Walks: TODO +/-");
        let paths_bp_len: Vec<_> = paths_len.values().map(|x| x.1).collect();
        let paths_len: Vec<_> = paths_len.values().map(|x| x.0).collect();
        PathInfo {
            no_paths: paths_len.len(),
            node_len: LenInfo {
                longest: *paths_len.iter().max().unwrap(),
                shortest: *paths_len.iter().min().unwrap(),
                average: averageu32(&paths_len),
            },
            bp_len: LenInfo {
                longest: *paths_bp_len.iter().max().unwrap(),
                shortest: *paths_bp_len.iter().min().unwrap(),
                average: averageu32(&paths_bp_len),
            },
        }
    }

    pub fn number_of_items(&self, c: &CountType) -> usize {
        match c {
            &CountType::Node | &CountType::Bp => self.node_count,
            &CountType::Edge => self.edge_count,
            &CountType::All => unreachable!("inadmissible count type"),
        }
    }

    pub fn parse_edge_gfa(
        gfa_file: &str,
        node2id: &HashMap<Vec<u8>, ItemId>,
    ) -> (HashMap<Edge, ItemId>, usize, Vec<u32>) {
        let mut edge2id = HashMap::default();
        let mut degree: Vec<u32> = vec![0; node2id.len() + 1];
        let mut edge_id: ItemIdSize = 1;

        let mut buf = vec![];
        let mut data = bufreader_from_compressed_gfa(gfa_file);
        while data.read_until(b'\n', &mut buf).unwrap_or(0) > 0 {
            if buf[0] == b'L' {
                let edge = Edge::from_link(&buf[..], node2id, true);
                if let std::collections::hash_map::Entry::Vacant(e) = edge2id.entry(edge) {
                    degree[edge.0 .0 as usize] += 1;
                    //if e.0.0 != e.2.0 {
                    degree[edge.2 .0 as usize] += 1;
                    //}
                    e.insert(ItemId(edge_id));
                    edge_id += 1;
                } else {
                    log::warn!("edge {} is duplicated in GFA", &edge);
                }
            }
            buf.clear();
        }
        let edge_count = edge2id.len();
        log::info!("found: {} edges", edge_count);

        (edge2id, edge_count, degree)
    }

    pub fn parse_nodes_gfa(
        gfa_file: &str,
        k: Option<usize>,
    ) -> (
        HashMap<Vec<u8>, ItemId>,
        Vec<PathSegment>,
        Vec<u32>,
        Option<Vec<(u64, u64)>>,
    ) {
        let mut node2id: HashMap<Vec<u8>, ItemId> = HashMap::default();
        let mut path_segments: Vec<PathSegment> = Vec::new();
        let mut node_lens: Vec<u32> = Vec::new();
        let mut extremities: Vec<(u64, u64)> = Vec::new();

        log::info!("constructing indexes for node/edge IDs, node lengths, and P/W lines..");
        node_lens.push(u32::MIN); // add empty element to node_lens to make it in sync with node_id
        let mut node_id = 1; // important: id must be > 0, otherwise counting procedure will produce errors

        let mut buf = vec![];
        let mut data = bufreader_from_compressed_gfa(gfa_file);
        while data.read_until(b'\n', &mut buf).unwrap_or(0) > 0 {
            if buf[0] == b'S' {
                let mut iter = buf[2..].iter();
                let offset = iter.position(|&x| x == b'\t').unwrap();
                if node2id
                    .insert(buf[2..offset + 2].to_vec(), ItemId(node_id))
                    .is_some()
                {
                    panic!(
                        "Segment with ID {} occurs multiple times in GFA",
                        str::from_utf8(&buf[2..offset + 2]).unwrap()
                    )
                }
                let start_sequence = offset + 3;
                let offset = iter
                    .position(|&x| x == b'\t' || x == b'\n' || x == b'\r')
                    .unwrap();
                if k.is_some() {
                    let (left, right) =
                        get_extremities(&buf[start_sequence..start_sequence + offset], k.unwrap());
                    extremities.push((left, right));
                }
                node_lens.push(offset as u32);
                node_id += 1;
            } else if buf[0] == b'P' {
                path_segments.push(Self::parse_path_segment(&buf));
            } else if buf[0] == b'W' {
                path_segments.push(Self::parse_walk_segment(&buf));
            }
            buf.clear();
        }

        log::info!(
            "found: {} paths/walks, {} nodes",
            path_segments.len(),
            node2id.len()
        );
        if path_segments.is_empty() {
            log::warn!("graph does not contain any annotated paths (P/W lines)");
        }

        (
            node2id,
            path_segments,
            node_lens,
            if k.is_none() { None } else { Some(extremities) },
        )
    }

    pub fn parse_path_segment(data: &[u8]) -> PathSegment {
        let mut iter = data.iter();
        let start = iter.position(|&x| x == b'\t').unwrap() + 1;
        let offset = iter.position(|&x| x == b'\t').unwrap();
        let path_name = str::from_utf8(&data[start..start + offset]).unwrap();
        PathSegment::from_str(path_name)
    }

    pub fn parse_walk_segment(data: &[u8]) -> PathSegment {
        let mut six_col: Vec<&str> = Vec::with_capacity(6);

        let mut it = data.iter();
        let mut i = 0;
        for _ in 0..6 {
            let j = it.position(|x| x == &b'\t').unwrap();
            six_col.push(str::from_utf8(&data[i..i + j]).unwrap());
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

        PathSegment::new(
            six_col[1].to_string(),
            six_col[2].to_string(),
            six_col[3].to_string(),
            seq_start,
            seq_end,
        )
    }

    // pub fn get_k_plus_one_mer_edge(
    //     &self,
    //     u: usize,
    //     o1: Orientation,
    //     v: usize,
    //     o2: Orientation,
    //     k: usize,
    // ) -> u64 {
    //     let extremities = self.extremities.as_ref().unwrap();

    //     let left = if o1 == Orientation::Forward {
    //         extremities[u].1
    //     } else {
    //         revcmp(extremities[u].0, k)
    //     };
    //     let right = if o2 == Orientation::Forward {
    //         extremities[v].0 & 0b11
    //     } else {
    //         revcmp(extremities[v].1, k) & 0b11
    //     };

    //     (left << 2) | right
    // }

    // pub fn get_k_plus_one_mer_right_telomer(&self, u: usize, o1: Orientation, k: usize) -> u64 {
    //     let extremities = self.extremities.as_ref().unwrap();

    //     let left = if o1 == Orientation::Forward  {
    //         extremities[u].1
    //     } else {
    //         revcmp(extremities[u].0, k)
    //     };

    //     (left << 2) | right
    // }

    //#[allow(dead_code)]
    //pub fn degree_distribution(&self) -> Option<Vec<u32>> {
    //    match &self.degree {
    //        Some(degree) => {
    //            let mut hist: Vec<u32> = vec![0,1];
    //            for i in 1..self.node_count+1 {
    //                if degree[i] as usize >= hist.len() {
    //                    hist.resize(degree[i] as usize +1, 0);
    //                }
    //                hist[degree[i] as usize] += 1;
    //            }
    //            Some(hist)
    //        }
    //        None => None
    //    }
    //}
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
            sample,
            haplotype: Some(haplotype),
            seqid: Some(seqid),
            start,
            end,
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
            match segments.len() {
                4 => {
                    res.sample = segments[1].to_string();
                    res.haplotype = Some(segments[2][1..].to_string());
                    match PATHID_COORDS.captures(&segments[3][1..]) {
                        None => {
                            res.seqid = Some(segments[3][1..].to_string());
                        }
                        Some(cc) => {
                            res.seqid = Some(cc.get(1).unwrap().as_str().to_string());
                            res.start = usize::from_str(cc.get(2).unwrap().as_str()).ok();
                            res.end = usize::from_str(cc.get(3).unwrap().as_str()).ok();
                            log::debug!("path has coordinates {} ", res);
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
                            res.haplotype = Some(cc.get(1).unwrap().as_str().to_string());
                            res.start = usize::from_str(cc.get(2).unwrap().as_str()).ok();
                            res.end = usize::from_str(cc.get(3).unwrap().as_str()).ok();
                            log::debug!("path has coordinates {} ", res);
                        }
                    }
                }
                2 => {
                    if let Some(cc) = PATHID_COORDS.captures(segments[1]) {
                        res.sample = cc.get(1).unwrap().as_str().to_string();
                        res.start = usize::from_str(cc.get(2).unwrap().as_str()).ok();
                        res.end = usize::from_str(cc.get(3).unwrap().as_str()).ok();
                        log::debug!("path has coordinates {}", res);
                    }
                }
                _ => (),
            }
        }
        res
    }

    pub fn from_str_start_end(s: &str, start: usize, end: usize) -> Self {
        let mut segment = Self::from_str(s);
        segment.start = Some(start);
        segment.end = Some(end);
        segment
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

    //#[allow(dead_code)]
    //pub fn covers(&self, other: &PathSegment) -> bool {
    //    self.sample == other.sample
    //        && (self.haplotype == other.haplotype
    //            || (self.haplotype.is_none()
    //                && self.seqid.is_none()
    //                && self.start.is_none()
    //                && self.end.is_none()))
    //        && (self.seqid == other.seqid
    //            || (self.seqid.is_none() && self.start.is_none() && self.end.is_none()))
    //        && (self.start == other.start
    //            || self.start.is_none()
    //            || (other.start.is_some() && self.start.unwrap() <= other.start.unwrap()))
    //        && (self.end == other.end
    //            || self.end.is_none()
    //            || (other.end.is_some() && self.end.unwrap() >= other.end.unwrap()))
    //}
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

pub struct GraphInfo {
    pub node_count: usize,
    pub edge_count: usize,
    pub average_degree: f32,
    pub max_degree: u32,
    pub min_degree: u32,
    pub number_0_degree: usize,
    pub connected_components: u32,
    pub largest_component: u32,
    pub smallest_component: u32,
    pub median_component: f64,
    pub largest_node: u32,
    pub shortest_node: u32,
    pub average_node: f32,
    pub median_node: f64,
    pub n50_node: u32,
    pub basepairs: u32,
    pub group_count: usize,
}

pub struct PathInfo {
    pub no_paths: usize,
    pub node_len: LenInfo,
    pub bp_len: LenInfo,
}

pub struct LenInfo {
    pub longest: u32,
    pub shortest: u32,
    pub average: f32,
}

pub struct GroupInfo {
    pub groups: HashMap<String, (u32, u32)>,
}

pub struct Info {
    pub graph_info: GraphInfo,
    pub path_info: PathInfo,
    pub group_info: Option<GroupInfo>,
}

impl fmt::Display for Info {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "feature\tcategory\tcountable\tvalue")?;
        writeln!(f, "graph\ttotal\tnode\t{}", self.graph_info.node_count)?;
        writeln!(f, "graph\ttotal\tbp\t{}", self.graph_info.basepairs)?;
        writeln!(f, "graph\ttotal\tedge\t{}", self.graph_info.edge_count)?;
        writeln!(f, "graph\ttotal\tpath\t{}", self.path_info.no_paths)?;
        writeln!(f, "graph\ttotal\tgroup\t{}", self.graph_info.group_count)?;
        writeln!(
            f,
            "graph\ttotal\t0-degree node\t{}",
            self.graph_info.number_0_degree
        )?;
        writeln!(
            f,
            "graph\ttotal\tcomponent\t{}",
            self.graph_info.connected_components
        )?;
        writeln!(
            f,
            "graph\tlargest\tcomponent\t{}",
            self.graph_info.largest_component
        )?;
        writeln!(
            f,
            "graph\tsmallest\tcomponent\t{}",
            self.graph_info.smallest_component
        )?;
        writeln!(
            f,
            "graph\tmedian\tcomponent\t{}",
            self.graph_info.median_component
        )?;
        writeln!(f, "node\taverage\tbp\t{}", self.graph_info.average_node)?;
        writeln!(
            f,
            "node\taverage\tdegree\t{}",
            self.graph_info.average_degree
        )?;
        writeln!(f, "node\tlongest\tbp\t{}", self.graph_info.largest_node)?;
        writeln!(f, "node\tshortest\tbp\t{}", self.graph_info.shortest_node)?;
        writeln!(f, "node\tmedian\tbp\t{}", self.graph_info.median_node)?;
        writeln!(f, "node\tN50 node\tbp\t{}", self.graph_info.n50_node)?;
        writeln!(f, "node\tmax\tdegree\t{}", self.graph_info.max_degree)?;
        writeln!(f, "node\tmin\tdegree\t{}", self.graph_info.min_degree)?;
        writeln!(f, "path\taverage\tbp\t{}", self.path_info.bp_len.average)?;
        writeln!(
            f,
            "path\taverage\tnode\t{}",
            self.path_info.node_len.average
        )?;
        writeln!(f, "path\tlongest\tbp\t{}", self.path_info.bp_len.longest)?;
        writeln!(
            f,
            "path\tlongest\tnode\t{}",
            self.path_info.node_len.longest
        )?;
        writeln!(f, "path\tshortest\tbp\t{}", self.path_info.bp_len.shortest)?;
        write!(
            f,
            "path\tshortest\tnode\t{}",
            self.path_info.node_len.shortest
        )?;
        if let Some(group_info) = &self.group_info {
            let mut sorted: Vec<_> = group_info.groups.clone().into_iter().collect();
            sorted.sort_by(|(k0, _v0), (k1, _v1)| k0.cmp(k1));
            for (k, v) in sorted {
                write!(f, "\ngroup\t{}\tbp\t{}\n", k, v.1)?;
                write!(f, "group\t{}\tnode\t{}", k, v.0)?;
            }
        }
        Ok(())
    }
}
