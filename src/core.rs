/* standard use */
use std::str;

/* crate use */
use rustc_hash::FxHashMap;
use quick_csv::Csv;

mod io;

pub const MASK_LEN: u64 = 1073741823;
pub const BITS_NODEID: u8 = 64 - MASK_LEN.count_ones() as u8;

pub trait Countable: Sized + Copy {
    fn hash(self) -> u64;
}

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Hash, Eq, Ord)]
pub struct Node(u64);

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Hash, Eq, Ord)]
pub struct Edge(u64);

impl Countable for Node {
    fn hash(self) -> u64 {
        self.0
    }
}

impl Node {
    #[inline]
    pub fn new(id: u64, length: u64) -> Self {
        assert!(
            length < MASK_LEN,
            "length ({}) of node {} is >= {}, which is not permissible with this program",
            length,
            id,
            MASK_LEN
        );
        assert!(
            id < u64::MAX - MASK_LEN,
            "node id ({}) >= {}, which is not permissible with this program",
            id,
            u64::MAX - MASK_LEN
        );
        Self((id << BITS_NODEID) + length)
    }

    #[inline]
    pub fn id(self) -> u64 {
        self.0 >> BITS_NODEID
    }

    #[inline]
    pub fn len(self) -> u64 {
        self.0 & MASK_LEN
    }

    #[inline]
    pub fn hash(self) -> u64 {
        Countable::hash(self)
    }
}

impl Countable for Edge {
    fn hash(self) -> u64 {
        self.0
    }
}

impl Edge {
    #[inline]
    pub fn new(id1: u64, is_reverse1: bool, id2: u64, is_reverse2: bool) -> Self {
        assert!(
            id1 < (u32::MAX - u32::pow(2, 31)).into(),
            "node id ({}) >= {}, which is not permissible with this program",
            id1,
            u32::MAX - u32::pow(2, 31)
        );
        assert!(
            id2 < (u32::MAX - u32::pow(2, 31)).into(),
            "node id ({}) >= {}, which is not permissible with this program",
            id2,
            u32::MAX - u32::pow(2, 31)
        );

        let (uid, u_is_reverse, vid, v_is_reverse) = Edge::canonize(id1, is_reverse1, id2, is_reverse2);
        
        let mut hash = (uid << 32) + vid;
        if u_is_reverse {
            hash += u64::pow(2, 63);
        }
        if v_is_reverse {
            hash += u64::pow(2, 31);
        }
        Self(hash)
    }

    #[inline]
    fn canonize(id1: u64, is_reverse1: bool, id2: u64, is_reverse2: bool) -> (u64, bool, u64, bool) {
        if (is_reverse1 && is_reverse2) || (is_reverse1 != is_reverse2 && id1 > id2) {
            (id2, !is_reverse2, id1, !is_reverse1)
        } else {
            (id1, is_reverse1, id2, is_reverse2)
        }
    }

    #[inline]
    pub fn uid(self) -> u64 {
        (self.0 >> 32) & (u32::MAX - u32::pow(2, 31)) as u64
    }

    #[inline]
    pub fn u_is_reverse(self) -> bool {
        (self.0 & u64::pow(2, 63)) > 0
    }

    #[inline]
    pub fn vid(self) -> u64 {
        self.0 & ((u32::MAX as u64 - 2) ^ 32)
    }

    #[inline]
    pub fn v_is_reverse(self) -> bool {
        (self.0 & u64::pow(2, 31)) > 0
    }

    #[inline]
    pub fn hash(self) -> u64 {
        Countable::hash(self)
    }
}


#[derive(Debug, Clone)]
pub struct Abacus<T: Countable> {
    pub countable2path: FxHashMap<T, Vec<usize>>,
    pub paths: Vec<(String, String, String, usize, usize)>,

}


impl Abacus<Node>{
    pub fn from_gfa<R: std::io::Read>(data: &mut std::io::BufReader<R>) -> Self {

        let mut countable2path: FxHashMap<Node, Vec<usize>> = FxHashMap::default();
        let mut paths: Vec<(String, String, String, usize, usize)> = Vec::new();

        let mut node2id : FxHashMap<String, u64> = FxHashMap::default();
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
                node2id.entry(str::from_utf8(sid).unwrap().to_string()).or_insert({node_count += 1; node_count-1});
            } else if fst_col == &[b'W'] {
                let (sample_id, hap_id, seq_id, seq_start, seq_end, walk) = io::parse_walk_line(row_it); 
                paths.push((sample_id, hap_id, seq_id, seq_start, seq_end));
                walk.into_iter().for_each(|(node, _)| {
                    countable2path.entry(Node::new(*node2id.get(&node).expect(&format!("unkown node {}", &node)), 1)).or_insert(Vec::new()).push(paths.len());
                });
            } else if &[b'P'] == fst_col {
                let (sample_id, hap_id, seq_id, seq_start, seq_end, path) = io::parse_path_line(row_it); 
                paths.push((sample_id, hap_id, seq_id, seq_start, seq_end));
                path.into_iter().for_each(|(node, _)| {
                    countable2path.entry(Node::new(*node2id.get(&node).expect(&format!("unkown node {}", &node)), 1)).or_insert(Vec::new()).push(paths.len());
                });
            }
        }

        Abacus { countable2path, paths }
    }
}
