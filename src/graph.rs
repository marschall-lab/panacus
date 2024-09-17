/* standard use */
use std::collections::HashMap;
use std::fmt;
use std::io::{BufRead};
use std::str::{self};

/* private use */
use crate::io::bufreader_from_compressed_gfa;
use crate::util::*;
use crate::util::{CountType};

/* private use */
use crate::path::*;
use crate::path_parser::*;

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
    pub fn to_lg(&self) -> char {
        match self {
            &Orientation::Forward => '>',
            &Orientation::Backward => '<',
        }
    }

    pub fn to_pm(&self) -> char {
        match self {
            &Orientation::Forward => '+',
            &Orientation::Backward => '-',
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

pub type ItemId = u64;

//#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
//pub struct ItemId(pub ItemId);

//impl fmt::Display for ItemId {
//    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
//        write!(f, "{}", self.0)
//    }
//}

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
        if u > v || (u == v && o1 == Orientation::Backward) {
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
    pub extremities: Option<Vec<(u64, u64)>>,
}

impl GraphAuxilliary {
    pub fn from_gfa(gfa_file: &str, index_edges: bool) -> Self {
        //Nodes
        let (node2id, path_segments, node_lens, extremities) = Self::parse_nodes_gfa(gfa_file, None);
        let node_count = node2id.len();
        //Edges
        let (edge2id, edge_count, degree) = if index_edges {
            let (edge2id, edge_count, degree) = Self::parse_edge_gfa(gfa_file, &node2id);
            (Some(edge2id), edge_count, Some(degree))
        } else {
            (None, 0, None)
        };

        Self {
            node2id,
            node_lens,
            edge2id,
            path_segments,
            node_count,
            edge_count,
            degree,
            extremities,
        }
    }

    pub fn from_cdbg_gfa(gfa_file: &str, k: usize) -> Self {
        let (node2id, path_segments, node_lens, extremities) = Self::parse_nodes_gfa(gfa_file, Some(k));
        let (edge2id, edge_count, degree) = (None, 0, None);
        let node_count = node2id.len();

        Self {
            node2id,
            node_lens,
            edge2id,
            path_segments,
            node_count,
            edge_count,
            degree,
            extremities,
        }
    }

    pub fn node_len(&self, v: ItemId) -> u32 {
        self.node_lens[v as usize]
    }

    pub fn stats(&self, paths_len: &Vec<u32>) -> Stats {
        Stats {
            graph_info: self.graph_info(),
            path_info: self.path_info(paths_len),
        }
    }

    pub fn graph_info(&self) -> GraphInfo {
        let degree = self.degree.as_ref().unwrap();
        let mut node_lens_sorted = self.node_lens[1..].to_vec();
        node_lens_sorted.sort_by(|a, b| b.cmp(a)); // decreasing, for N50

        GraphInfo {
            node_count: self.node_count,
            edge_count: self.edge_count,
            average_degree: averageu32(&degree[1..]),
            max_degree: *degree[1..].iter().max().unwrap(),
            min_degree: *degree[1..].iter().min().unwrap(),
            number_0_degree: degree[1..].iter().filter(|&x| *x == 0).count(),
            largest_node: *node_lens_sorted.iter().max().unwrap(),
            shortest_node: *node_lens_sorted.iter().min().unwrap(),
            average_node: averageu32(&node_lens_sorted),
            median_node: median_already_sorted(&node_lens_sorted),
            n50_node: n50_already_sorted(&node_lens_sorted).unwrap(),
        }
    }

    pub fn path_info(&self, paths_len: &Vec<u32>) -> PathInfo {
        //println!("\tDistribution of Strands in the Paths/Walks: TODO +/-");
        PathInfo {
            no_paths: paths_len.len(),
            longest_path: *paths_len.iter().max().unwrap(),
            shortest_path: *paths_len.iter().min().unwrap(),
            average_path: averageu32(&paths_len),
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
        let mut edge_id: ItemId = 1;

        let mut buf = vec![];
        let mut data = bufreader_from_compressed_gfa(gfa_file);
        while data.read_until(b'\n', &mut buf).unwrap_or(0) > 0 {
            if buf[0] == b'L' {
                let edge = Edge::from_link(&buf[..], node2id, true);
                if edge2id.contains_key(&edge) {
                    log::warn!("edge {} is duplicated in GFA", &edge);
                } else {
                    degree[edge.0 as usize] += 1;
                    if edge.0 != edge.2 {
                        degree[edge.2 as usize] += 1;
                    }
                    edge2id.insert(edge, edge_id as ItemId);
                    edge_id += 1;
                }
            }
            buf.clear();
        }
        let edge_count = edge2id.len() as usize;
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
                    .insert(buf[2..offset + 2].to_vec(), node_id as ItemId)
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
                let (path_segment, _buf_path_seg) = parse_path_identifier(&buf);
                path_segments.push(path_segment);
            } else if buf[0] == b'W' {
                let (path_segment, _buf_path_seg) = parse_walk_identifier(&buf);
                path_segments.push(path_segment);
            }
            buf.clear();
        }

        log::info!(
            "found: {} paths/walks, {} nodes",
            path_segments.len(),
            node2id.len()
        );
        if path_segments.len() == 0 {
            log::warn!("graph does not contain any annotated paths (P/W lines)");
        }

        (
            node2id,
            path_segments,
            node_lens,
            if k.is_none() { None } else { Some(extremities) },
        )
    }

    pub fn get_k_plus_one_mer_edge(
        &self,
        u: usize,
        o1: Orientation,
        v: usize,
        o2: Orientation,
        k: usize,
    ) -> u64 {
        let extremities = self.extremities.as_ref().unwrap();

        let left = if o1 == Orientation::Forward {
            extremities[u].1
        } else {
            revcmp(extremities[u].0, k)
        };
        let right = if o2 == Orientation::Forward {
            extremities[v].0 & 0b11
        } else {
            revcmp(extremities[v].1, k) & 0b11
        };

        (left << 2) | right
    }

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

pub struct GraphInfo {
    pub node_count: usize,
    pub edge_count: usize,
    pub average_degree: f32,
    pub max_degree: u32,
    pub min_degree: u32,
    pub number_0_degree: usize,
    pub largest_node: u32,
    pub shortest_node: u32,
    pub average_node: f32,
    pub median_node: f64,
    pub n50_node: u32,
}

pub struct PathInfo {
    pub no_paths: usize,
    pub longest_path: u32,
    pub shortest_path: u32,
    pub average_path: f32,
}

pub struct Stats {
    pub graph_info: GraphInfo,
    pub path_info: PathInfo,
}

impl fmt::Display for Stats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Graph Info\tNode Count\t{}\n",
            self.graph_info.node_count
        )?;
        write!(f, "\tEdge Count\t{}\n", self.graph_info.edge_count)?;
        write!(f, "\tPath Count\t{}\n", self.path_info.no_paths)?;
        write!(
            f,
            "\t0-degree Node Count\t{}\n",
            self.graph_info.number_0_degree
        )?;
        write!(
            f,
            "Node Info\tAverage Degree\t{}\n",
            self.graph_info.average_degree
        )?;
        write!(f, "\tMax Degree\t{}\n", self.graph_info.max_degree)?;
        write!(f, "\tMin Degree\t{}\n", self.graph_info.min_degree)?;
        write!(f, "\tLargest\t{}\n", self.graph_info.largest_node)?;
        write!(f, "\tShortest\t{}\n", self.graph_info.shortest_node)?;
        write!(f, "\tAverage Length\t{}\n", self.graph_info.average_node)?;
        write!(f, "\tMedian Length\t{}\n", self.graph_info.median_node)?;
        write!(f, "\tN50 Node Length\t{}\n", self.graph_info.n50_node)?;
        write!(f, "Path Info\tLongest\t{}\n", self.path_info.longest_path)?;
        write!(f, "\tShortest\t{}\n", self.path_info.shortest_path)?;
        write!(f, "\tAverage Node Count\t{}\n", self.path_info.average_path)
    }
}
