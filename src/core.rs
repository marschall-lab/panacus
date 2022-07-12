pub trait Countable: Sized + Copy {

    #[inline]
    fn hash(self) -> u64;

}

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Hash, Eq, Ord)]
pub struct Node(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Hash, Eq, Ord)]
pub struct Edge(pub u64);

impl Countable for Node {

    fn hash(self) -> u64{
        self.0
    }
}

impl Node {
    fn new(id: u64, length: u64) -> Self{
        Self(id) // TODO
    }

    fn id(self) -> u64 {
        self.0 // TODO
    }

    fn len(self) -> u64 {
        self.0 // TODO
    }
}


impl Countable for Edge {

    fn hash(self) -> u64{
        self.0
    }
}

impl Edge {
    fn new(id1: u64, id2: u64) -> Self{
        Self(id1) // TODO
    }

    fn id1(self) -> u64 {
        self.0 // TODO
    }

    fn id2(self) -> u64 {
        self.0 // TODO
    }

}
