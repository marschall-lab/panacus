pub const MASK_LEN: u64 = 1073741823;
pub const BITS_NODEID: u8 = 64 - MASK_LEN.count_ones() as u8;

pub trait Countable: Sized + Copy {
    #[inline]
    fn hash(self) -> u64;
}

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Hash, Eq, Ord)]
pub struct Node(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Hash, Eq, Ord)]
pub struct Edge(pub u64);

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
        let mut hash = (id1 << 32) + id2;
        if is_reverse1 {
            hash += u64::pow(2, 63);
        }
        if is_reverse2 {
            hash += u64::pow(2, 31);
        }
        Self(hash)
    }

    pub fn id1(self) -> u64 {
        (self.0 >> 32) & (u32::MAX - u32::pow(2, 31)) as u64
    }

    pub fn is_reverse1(self) -> bool {
        (self.0 & u64::pow(2, 63)) > 0
    }

    pub fn id2(self) -> u64 {
        self.0 & (u32::MAX as u64 - 2 ^ 32)
    }

    pub fn is_reverse2(self) -> bool {
        (self.0 & u64::pow(2, 31)) > 0
    }

    pub fn hash(self) -> u64 {
        Countable::hash(self)
    }
}
