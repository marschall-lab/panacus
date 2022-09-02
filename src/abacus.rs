/* standard use */
//use std::fmt;
//use std::iter::FromIterator;
//use std::str::{self, FromSt};
/* external crate*/
use rayon::prelude::*;
//use rayon::par_iter::{IntoParallelIterator, ParallelIterator};
use std::collections::{HashMap};
/* private use */
use crate::io;
use crate::graph::{*};

pub const SIZE_T: usize = 1024;
struct Wrap<T>(*mut T);
unsafe impl Sync for Wrap<Vec<u32>> {}
unsafe impl Sync for Wrap<Vec<usize>> {}
unsafe impl Sync for Wrap<[Vec<u32>; SIZE_T]> {}

pub struct Prep {
    pub path_segments: Vec<PathSegment>,
    pub node_len: Vec<u32>,
    pub node2id: HashMap<Vec<u8>, u32>,
    pub groups: Vec<(PathSegment, String)>,
    pub subset_coords: Option<Vec<PathSegment>>,
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

#[derive(Debug, Clone)]
pub struct Abacus<T> {
    pub countable: Vec<T>,
    pub groups: Vec<String>,
}

impl Abacus<u32> {
    pub fn from_gfa<R: std::io::Read>(data: &mut std::io::BufReader<R>, prep: Prep) -> Self {
        log::info!("parsing path + walk sequences");
        let node_table = io::parse_gfa_nodecount(data, &prep);
        log::info!("counting abacus entries..");
        let mut countable: Vec<u32> = vec![0; prep.node2id.len()];
        let mut last: Vec<usize> = vec![usize::MAX; prep.node2id.len()];

        let mut groups = Vec::new();
        for (path_id, group_id) in Abacus::get_path_order(&prep) {
            if groups.is_empty() || groups.last().unwrap() != group_id {
                groups.push(group_id.to_string());
            }
            Abacus::count_nodes(
                &mut countable,
                &mut last,
                &node_table,
                path_id,
                groups.len() - 1,
            );
        }

        Abacus {
            countable: countable,
            groups: groups,
        }
    }

    fn count_nodes(
        countable: &mut Vec<u32>,
        last: &mut Vec<usize>,
        node_table: &ItemTable,
        path_id: usize,
        group_id: usize,
    ) {
        let countable_ptr = Wrap(countable);
        let last_ptr = Wrap(last);

        // Parallel node counting
        (0..SIZE_T).into_par_iter().for_each(|i| {
            //Abacus::add_count(i, path_id, &mut countable, &mut last, &node_table);
            let start = node_table.id_prefsum[i][path_id] as usize;
            let end = node_table.id_prefsum[i][path_id + 1] as usize;
            for j in start..end {
                let sid = node_table.items[i][j] as usize;
                unsafe {
                    if last[sid] != group_id {
                        (*countable_ptr.0)[sid] += 1;
                        (*last_ptr.0)[sid] = group_id;
                    }
                }
            }
        });
    }

    fn get_path_order<'a>(prep: &'a Prep) -> Vec<(usize, &'a str)> {
        // orders the pathsegments in prep.path_segments by the order given 
        // in prep.groups. The returned vector maps indices of prep_path_segments 
        // to the group identifier

        let mut group_order = Vec::new();
        let mut group_to_paths: HashMap<&str, Vec<&PathSegment>> = HashMap::default();

        let mut path_to_id: HashMap<&PathSegment, usize> = HashMap::default();
        prep.path_segments.iter().enumerate().for_each(|(i, s)| {
            path_to_id.insert(s, i);
        });

        prep.groups.iter().for_each(|(k, v)| {
            group_to_paths
                .entry(&v[..])
                .or_insert({
                    group_order.push(&v[..]);
                    Vec::new()
                })
                .push(k)
        });

        let mut res = Vec::with_capacity(prep.path_segments.len());
        //let empty: Vec<&PathSegment> = Vec::new();
        for g in group_order.into_iter() {
            res.extend(
                group_to_paths
                    .get(g)
                    .unwrap()
                    .iter()
                    .map(|x| (*path_to_id.get(x).unwrap(), g)),
            );
        }

        res
    }

    //Why &self and not self? we could destroy abacus at this point.
    pub fn construct_hist(&self) -> Vec<u32> {
        //Hist must be of size = num_groups + 1.
        //Having an index that starts from 1, instead of 0, 
        //makes easier the calculation in hist2pangrowth. 
        //(Index 0 is ignored, i.e. no item is present in 0 groups)
        let mut hist: Vec<u32> = vec![0; self.groups.len() + 1];
        for iter in self.countable.iter() {
            hist[*iter as usize] += 1;
        }
        hist
    }

    pub fn hist2pangrowth(hist: Vec<u32>) -> Vec<u32> {
        let n = hist.len()-1; // hist has length n+1: from 0..n (both included)
        let pangrowth: Vec<u32> = Vec::with_capacity(n + 1);
        let mut n_fall_m = rug::Integer::from(1);
        let tot = rug::Integer::from(hist.iter().sum::<u32>());

        // perc_mult[i] contains the percentage of combinations that
        // have an item of multiplicity i
        let mut perc_mult: Vec<rug::Integer> = Vec::with_capacity(n + 1);
        perc_mult.resize(n + 1, rug::Integer::from(1));

        for m in 1..n + 1 {
            let mut y = rug::Integer::from(0);
            for i in 1..n - m + 1 {
                perc_mult[i] *= n - m - i + 1;
                y += hist[i] * &perc_mult[i];
            }
            n_fall_m *= n - m + 1;

            let dividend: rug::Integer = rug::Integer::from(&n_fall_m * &tot - &y);
            let divisor: rug::Integer = rug::Integer::from(&n_fall_m);
            let (pang_m, _) = dividend.div_rem(rug::Integer::from(divisor));
            println!("{} {}", m, pang_m);
        }
        //println!("tot: {}", tot);
        pangrowth
    }

}
