/* standard use */
use std::collections::HashMap;
use std::fmt;

/* external crate */
use strum_macros::{EnumString, EnumVariantNames};

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

pub struct ActiveTable<T> {
    pub items: Vec<bool>,
    pub annotation: Option<HashMap<usize, T>>,
}

impl<T> ActiveTable<T> {
    pub fn new(size: usize, with_annot: bool) -> Self {
        Self {
            items: vec![false; size],
            annotation: if with_annot {
                Some(HashMap::default())
            } else {
                None
            },
        }
    }

    pub fn activate(&mut self, id: usize) {
        self.items[id] |= true;
    }

    pub fn is_active(&self, id: usize) -> bool {
        self.items[id]
    }

    pub fn active_n_annotate(
        &mut self,
        id: usize,
        annot: T,
    ) -> Result<Option<T>, ActiveTableError> {
        match &mut self.annotation {
            None => Err(ActiveTableError::NoAnnotation),
            Some(m) => {
                self.items[id] |= true;
                Ok(m.insert(id, annot))
            }
        }
    }
}

#[derive(Debug)]
pub enum ActiveTableError {
    NoAnnotation,
}

impl std::error::Error for ActiveTableError {}

impl fmt::Display for ActiveTableError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ActiveTableError::NoAnnotation => write!(f, "Active Table has no annotations"),
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
