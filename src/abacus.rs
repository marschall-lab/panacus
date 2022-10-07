/* standard use */
//use std::fmt;
//use std::iter::FromIterator;
//use std::str::{self, FromSt};
use std::fs;
use std::io::Write;

/* external crate*/
use rayon::prelude::*;
//use rayon::par_iter::{IntoParallelIterator, ParallelIterator};
use std::collections::HashMap;
/* private use */
use crate::cli;
use crate::graph::*;
use crate::io;

pub const SIZE_T: usize = 1024;
struct Wrap<T>(*mut T);
unsafe impl Sync for Wrap<Vec<u32>> {}
unsafe impl Sync for Wrap<Vec<usize>> {}
unsafe impl Sync for Wrap<[Vec<u32>; SIZE_T]> {}

pub enum CountType {
    Node,
    BasePair,
    Edge,
}

pub struct AbacusData {
    pub path_segments: Vec<PathSegment>,
    pub count: CountType,
    pub node_len: Vec<u32>,
    pub node2id: HashMap<Vec<u8>, u32>,
    pub groups: Option<Vec<(PathSegment, String)>>,
    pub subset_coords: Option<Vec<PathSegment>>,
    pub exclude_coords: Option<Vec<PathSegment>>,
}

impl AbacusData {
    pub fn from_params(params: &cli::Params) -> Result<Self, std::io::Error> {
        match params {
            cli::Params::Growth {
                gfa_file,
                count,
                positive_list,
                negative_list,
                groupby,
                ..
            }
            | cli::Params::HistOnly {
                gfa_file,
                count,
                positive_list,
                negative_list,
                groupby,
                ..
            } => Self::load(gfa_file, count, positive_list, negative_list, groupby),
            _ => Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "not implemented",
            )),
        }
    }

    fn load(
        gfa_file: &str,
        count: &str,
        positive_list: &str,
        negative_list: &str,
        groupby: &str,
    ) -> Result<Self, std::io::Error> {
        let mut data = std::io::BufReader::new(fs::File::open(gfa_file)?);
        log::info!("preprocessing: constructing indexes for node IDs, node lengths, and P/W lines");
        let (node2id, node_len, path_segments) = io::parse_graph_marginals(&mut data);

        let count_type = match count {
            "nodes" => CountType::Node,
            "edges" => CountType::Edge,
            "bp" => CountType::BasePair,
            _ => unreachable!(),
        };

        let mut subset_coords = None;
        if !positive_list.is_empty() {
            log::info!("loading subset coordinates from {}", positive_list);
            let mut data = std::io::BufReader::new(fs::File::open(positive_list)?);
            subset_coords = Some(io::parse_bed(&mut data));
            log::debug!(
                "loaded {} coordinates",
                subset_coords.as_ref().unwrap().len()
            );
        }

        let mut exclude_coords = None;
        if !negative_list.is_empty() {
            log::info!("loading exclusion coordinates from {}", negative_list);
            let mut data = std::io::BufReader::new(fs::File::open(negative_list)?);
            exclude_coords = Some(io::parse_bed(&mut data));
            log::debug!(
                "loaded {} coordinates",
                exclude_coords.as_ref().unwrap().len()
            );
        }

        let mut groups = None;
        if !groupby.is_empty() {
            log::info!("loading groups from {}", groupby);
            let mut data = std::io::BufReader::new(fs::File::open(groupby)?);
            groups = Some(io::parse_groups(&mut data));
            log::debug!(
                "loaded {} group assignments ",
                groups.as_ref().unwrap().len()
            );
        }

        Ok(Self {
            path_segments: path_segments,
            count: count_type,
            node_len: node_len,
            node2id: node2id,
            groups: groups,
            subset_coords: subset_coords,
            exclude_coords: exclude_coords,
        })
    }
}

pub struct HistData {
    pub intersection: Option<Vec<(String, Threshold)>>,
    pub coverage: Option<Vec<(String, Threshold)>>,
}

impl HistData {
    pub fn from_params(params: &cli::Params) -> Result<Self, std::io::Error> {
        match params {
            cli::Params::Growth {
                intersection,
                coverage,
                ..
            }
            | cli::Params::GrowthOnly {
                intersection,
                coverage,
                ..
            } => Self::load(intersection, coverage),
            _ => Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "not implemented",
            )),
        }
    }

    fn load(intersection: &str, coverage: &str) -> Result<Self, std::io::Error> {
        let mut intersection_thresholds = None;
        if !intersection.is_empty() {
            if std::path::Path::new(intersection).exists() {
                log::info!("loading intersection thresholds from {}", intersection);
                let mut data = std::io::BufReader::new(fs::File::open(intersection)?);
                intersection_thresholds = Some(io::parse_coverage_threshold_file(&mut data));
            } else {
                intersection_thresholds =
                    Some(cli::parse_coverage_threshold_cli(&intersection[..]));
            }
            log::debug!(
                "loaded {} intersection thresholds:\n{}",
                intersection_thresholds.as_ref().unwrap().len(),
                intersection_thresholds
                    .as_ref()
                    .unwrap()
                    .iter()
                    .map(|(n, t)| format!("\t{}: {}", n, t))
                    .collect::<Vec<String>>()
                    .join("\n")
            );
        }

        let mut coverage_thresholds = None;
        if !coverage.is_empty() {
            if std::path::Path::new(&coverage).exists() {
                log::info!("loading coverage thresholds from {}", coverage);
                let mut data = std::io::BufReader::new(fs::File::open(coverage)?);
                coverage_thresholds = Some(io::parse_coverage_threshold_file(&mut data));
            } else {
                coverage_thresholds = Some(cli::parse_coverage_threshold_cli(&coverage[..]));
            }
            log::debug!(
                "loaded {} coverage thresholds:\n{}",
                coverage_thresholds.as_ref().unwrap().len(),
                coverage_thresholds
                    .as_ref()
                    .unwrap()
                    .iter()
                    .map(|(n, t)| format!("\t{}: {}", n, t))
                    .collect::<Vec<String>>()
                    .join("\n")
            );
        }

        Ok(Self {
            intersection: intersection_thresholds,
            coverage: coverage_thresholds,
        })
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

#[derive(Debug, Clone)]
pub struct Abacus<T> {
    pub countable: Vec<T>,
    pub groups: Vec<String>,
}

impl Abacus<u32> {
    pub fn from_gfa<R: std::io::Read>(
        data: &mut std::io::BufReader<R>,
        abacus_data: AbacusData,
    ) -> Self {
        log::info!("parsing path + walk sequences");
        let node_table = io::parse_gfa_nodecount(data, &abacus_data);
        log::info!("counting abacus entries..");
        let mut countable: Vec<u32> = vec![0; abacus_data.node2id.len()];
        let mut last: Vec<usize> = vec![usize::MAX; abacus_data.node2id.len()];

        let mut groups = Vec::new();
        for (path_id, group_id) in Abacus::get_path_order(&abacus_data) {
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

        Self {
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

    fn get_path_order<'a>(abacus_data: &'a AbacusData) -> Vec<(usize, &'a str)> {
        // orders the pathsegments in abacus_data.path_segments by the order given
        // in abacus_data.groups. The returned vector maps indices of abacus_data_path_segments
        // to the group identifier

        let mut group_order = Vec::new();
        let mut group_to_paths: HashMap<&str, Vec<&PathSegment>> = HashMap::default();

        let mut path_to_id: HashMap<&PathSegment, usize> = HashMap::default();
        abacus_data
            .path_segments
            .iter()
            .enumerate()
            .for_each(|(i, s)| {
                path_to_id.insert(s, i);
            });

        abacus_data
            .groups
            .as_ref()
            .unwrap()
            .iter()
            .for_each(|(k, v)| {
                group_to_paths
                    .entry(&v[..])
                    .or_insert({
                        group_order.push(&v[..]);
                        Vec::new()
                    })
                    .push(k)
            });

        let mut res = Vec::with_capacity(abacus_data.path_segments.len());
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
}

#[derive(Debug, Clone)]
pub struct Hist {
    pub ary: Vec<u32>,
    pub groups: Vec<String>,
}

impl Hist {
    pub fn from_tsv<R: std::io::Read>(data: &mut std::io::BufReader<R>) -> Self {
        // XXX TODO
        Self {
            ary: Vec::new(),
            groups: Vec::new(),
        }
    }

    pub fn from_abacus(abacus: &Abacus<u32>) -> Self {
        Self {
            ary: abacus.construct_hist(),
            groups: abacus.groups.clone(),
        }
    }

    pub fn calc_growth(&self) -> Vec<u32> {
        let n = self.ary.len() - 1; // hist array has length n+1: from 0..n (both included)
        let mut pangrowth: Vec<u32> = Vec::with_capacity(n + 1);
        let mut n_fall_m = rug::Integer::from(1);
        let tot = rug::Integer::from(self.ary.iter().sum::<u32>());

        // perc_mult[i] contains the percentage of combinations that
        // have an item of multiplicity i
        let mut perc_mult: Vec<rug::Integer> = Vec::with_capacity(n + 1);
        perc_mult.resize(n + 1, rug::Integer::from(1));

        for m in 1..n + 1 {
            let mut y = rug::Integer::from(0);
            for i in 1..n - m + 1 {
                perc_mult[i] *= n - m - i + 1;
                y += self.ary[i] * &perc_mult[i];
            }
            n_fall_m *= n - m + 1;

            let dividend: rug::Integer = rug::Integer::from(&n_fall_m * &tot - &y);
            let divisor: rug::Integer = rug::Integer::from(&n_fall_m);
            let (pang_m, _) = dividend.div_rem(rug::Integer::from(divisor));
            pangrowth.push(pang_m.to_u32().unwrap());
        }
        pangrowth
    }

    pub fn to_tsv<W: std::io::Write>(
        &self,
        out: &mut std::io::BufWriter<W>,
    ) -> Result<(), std::io::Error> {
        writeln!(out, "\t{}", self.ary[0])?;
        for i in 0..self.groups.len() {
            writeln!(out, "{}\t{}", self.groups[i], self.ary[i + 1])?;
        }

        Ok(())
    }
}
