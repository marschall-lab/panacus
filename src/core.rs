/* crate use */



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

    pub fn id(self) -> u64 {
        self.0 >> BITS_NODEID
    }

    pub fn len(self) -> u64 {
        self.0 & MASK_LEN
    }

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

    fn canonize(id1: u64, is_reverse1: bool, id2: u64, is_reverse2: bool) -> (u64, bool, u64, bool) {
        if (is_reverse1 && is_reverse2) || (is_reverse1 != is_reverse2 && id1 > id2) {
            (id2, !is_reverse2, id1, !is_reverse1)
        } else {
            (id1, is_reverse1, id2, is_reverse2)
        }
    }

    pub fn uid(self) -> u64 {
        (self.0 >> 32) & (u32::MAX - u32::pow(2, 31)) as u64
    }

    pub fn u_is_reverse(self) -> bool {
        (self.0 & u64::pow(2, 63)) > 0
    }

    pub fn vid(self) -> u64 {
        self.0 & ((u32::MAX as u64 - 2) ^ 32)
    }

    pub fn v_is_reverse(self) -> bool {
        (self.0 & u64::pow(2, 31)) > 0
    }

    pub fn hash(self) -> u64 {
        Countable::hash(self)
    }
}

pub struct Abacus<T: Countable>(Vec<T>);

impl Abacus<Node>{
    fn from_gfa<R: std::io::Read>(_data: &std::io::BufReader<R>) {
        
    }
}
