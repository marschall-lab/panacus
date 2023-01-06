/* standard use */
use std::fs;

/* external crate*/
use rayon::prelude::*;
use std::collections::HashMap;
/* private use */
use crate::cli::Params;
use crate::graph::*;
use crate::io;
use crate::util::*;

pub struct AbacusData {
    pub count: CountType,
    pub groups: HashMap<PathSegment, String>,
    pub include_coords: Option<Vec<PathSegment>>,
    pub exclude_coords: Option<Vec<PathSegment>>,
}

impl AbacusData {
    pub fn from_params(
        params: &Params,
        graph_marginals: &GraphData,
    ) -> Result<Self, std::io::Error> {
        Ok(match params {
            Params::Histgrowth {
                count,
                positive_list,
                negative_list,
                groupby,
                ..
            }
            | Params::Hist {
                count,
                positive_list,
                negative_list,
                groupby,
                ..
            } => {
                let groups = AbacusData::load_groups(groupby, graph_marginals)?;
                let include_coords = AbacusData::complement_with_group_assignments(
                    AbacusData::load_coord_list(positive_list)?,
                    &groups,
                )?;
                let exclude_coords = AbacusData::complement_with_group_assignments(
                    AbacusData::load_coord_list(negative_list)?,
                    &groups,
                )?;

                AbacusData {
                    count: count.clone(),
                    groups: groups,
                    include_coords: include_coords,
                    exclude_coords: exclude_coords,
                }
            }
            _ => unreachable!("cannot produce AbausData from other Param items"),
        })
    }

    fn complement_with_group_assignments(
        coords: Option<Vec<PathSegment>>,
        groups: &HashMap<PathSegment, String>,
    ) -> Result<Option<Vec<PathSegment>>, std::io::Error> {
        //
        // We allow coords to be defined via groups; the following code
        // 1. complements coords with path segments from group assignments
        // 2. checks that group-based coordinates don't have start/stop information
        //
        let mut group2ps: HashMap<String, Vec<PathSegment>> = HashMap::default();
        groups.iter().for_each(|(p, g)| {
            group2ps
                .entry(g.clone())
                .or_insert(Vec::new())
                .push(p.clone())
        });
        match coords {
            None => Ok(None),
            Some(v) => {
                v.into_iter()
                    .map(|p| {
                        // check if path segment defined in subset coords associated with a
                        // specific path segment (i.e., is not a group) by querying the
                        // keys of the "groups" hashmap
                        if groups.contains_key(&p) {
                            Ok(vec![p])
                        } else if group2ps.contains_key(&p.id()) {
                            if p.coords().is_some() {
                                Err(std::io::Error::new(
                                        std::io::ErrorKind::InvalidData, format!("invalid coordinate \"{}\": group identifiers are not allowed to have start/stop information!", &p)))
                            } else {
                                Ok(group2ps.get(&p.id()).unwrap().clone())
                            }
                        } else {
                            Ok(Vec::new())
                        }
                    })
                    .collect::<Result<Vec<Vec<PathSegment>>, std::io::Error>>().map(|x| Some(x[..]
                    .concat()))
            }
        }
    }

    fn load_coord_list(file_name: &str) -> Result<Option<Vec<PathSegment>>, std::io::Error> {
        Ok(if file_name.is_empty() {
            None
        } else {
            log::info!("loading coordinates from {}", file_name);
            let mut data = std::io::BufReader::new(fs::File::open(file_name)?);
            let coords = io::parse_bed(&mut data);
            log::debug!("loaded {} coordinates", coords.len());
            Some(coords)
        })
    }

    fn load_groups(
        file_name: &str,
        graph_marginals: &GraphData,
    ) -> Result<HashMap<PathSegment, String>, std::io::Error> {
        Ok(if file_name.is_empty() {
            log::info!("no explicit group file given, group paths by their IDs (sample ID+haplotype ID+seq ID)");
            graph_marginals
                .path_segments
                .iter()
                .cloned()
                .zip(graph_marginals.path_segments.iter().map(|x| x.id()))
                .collect()
        } else {
            log::info!("loading groups from {}", file_name);
            let mut data = std::io::BufReader::new(fs::File::open(file_name)?);
            let g = io::parse_groups(&mut data)?;
            log::debug!("loaded {} group assignments", g.len());
            let mut ps2group: HashMap<PathSegment, String> = g.into_iter().collect();
            graph_marginals
                .path_segments
                .iter()
                .map(|x| (x.clone(), ps2group.remove(x).unwrap_or(x.id())))
                .collect()
        })
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
        graph_marginals: GraphData,
    ) -> Self {
        log::info!("parsing path + walk sequences");
        let node_table = io::parse_gfa_nodecount(data, &abacus_data, &graph_marginals);
        log::info!("counting abacus entries..");
        let mut countable: Vec<u32> = vec![0; graph_marginals.node2id.len()];
        let mut last: Vec<usize> = vec![usize::MAX; graph_marginals.node2id.len()];

        let mut groups = Vec::new();
        for (path_id, group_id) in
            Abacus::get_path_order(&abacus_data, &graph_marginals.path_segments)
        {
            if groups.is_empty() || groups.last().unwrap() != group_id {
                groups.push(group_id.to_string());
            }
            Abacus::node_coverage(
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

    fn node_coverage(
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
        // hist must be of size = num_groups + 1; having an index that starts from 1, instead of 0,
        // makes easier the calculation in hist2pangrowth.
        //(Index 0 is ignored, i.e. no item is present in 0 groups)
        let mut hist: Vec<u32> = vec![0; self.groups.len() + 1];
        for iter in self.countable.iter() {
            hist[*iter as usize] += 1;
        }
        hist
    }

    fn get_path_order<'a>(
        abacus_data: &'a AbacusData,
        path_segments: &Vec<PathSegment>,
    ) -> Vec<(usize, &'a str)> {
        // orders elements of path_segments by the order in abacus_data.groups; the returned vector
        // maps indices of path_segments to the group identifier

        let mut group_order = Vec::new();
        let mut group_to_paths: HashMap<&str, Vec<&PathSegment>> = HashMap::default();

        let mut path_to_id: HashMap<&PathSegment, usize> = HashMap::default();
        path_segments.iter().enumerate().for_each(|(i, s)| {
            path_to_id.insert(s, i);
        });

        abacus_data.groups.iter().for_each(|(k, v)| {
            group_to_paths
                .entry(v)
                .or_insert_with(|| {
                    group_order.push(&v[..]);
                    Vec::new()
                })
                .push(k)
        });

        let mut res = Vec::with_capacity(path_segments.len());
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
