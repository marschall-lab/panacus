/* standard use */
use std::fmt;
use strum_macros::{EnumString, EnumVariantNames}; 

/* external crate */


pub const SIZE_T: usize = 1024;
pub struct Wrap<T>(pub *mut T);
unsafe impl Sync for Wrap<Vec<u32>> {}
unsafe impl Sync for Wrap<Vec<usize>> {}
unsafe impl Sync for Wrap<[Vec<u32>; SIZE_T]> {}


#[derive(Debug, Clone, Copy, PartialEq, EnumString, EnumVariantNames)]
#[strum(serialize_all = "lowercase")]
pub enum CountType {
    Nodes,
    Bps,
    Edges,
}

impl CountType {
    pub fn from_str(count_type_str: &str) -> Self {
        match count_type_str {
            "nodes" => CountType::Nodes,
            "edges" => CountType::Edges,
            "bps" => CountType::Bps,
            _ => unreachable!(),
        }
    }
}

impl fmt::Display for CountType {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        write!(
            formatter,
            "{}",
            match self {
                CountType::Nodes => "nodes",
                CountType::Edges => "edges",
                CountType::Bps => "bps",
            }
        )
    }
}

pub struct ItemTable {
    pub items: [Vec<u32>; SIZE_T],
    pub id_prefsum: [Vec<u32>; SIZE_T],
}

impl ItemTable {
    pub fn new(num_walks_paths: usize) -> Self {
        Self {
            items: [(); SIZE_T].map(|_| vec![]),
            id_prefsum: [(); SIZE_T].map(|_| vec![0; num_walks_paths + 1]),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Threshold {
    Relative(f64),
    Absolute(usize),
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
