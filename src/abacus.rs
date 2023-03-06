/* standard use */
use std::fs;
use std::iter::FromIterator;

/* external crate*/
use rayon::prelude::*;
use std::collections::{HashMap, HashSet};
/* private use */
use crate::cli::Params;
use crate::graph::*;
use crate::io;
use crate::util::*;

pub struct AbacusAuxilliary {
    pub count: CountType,
    pub groups: HashMap<PathSegment, String>,
    pub include_coords: Option<Vec<PathSegment>>,
    pub exclude_coords: Option<Vec<PathSegment>>,
}

impl AbacusAuxilliary {
    pub fn from_params(
        params: &Params,
        graph_aux: &GraphAuxilliary,
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
                let groups = AbacusAuxilliary::load_groups(groupby, graph_aux)?;
                let include_coords = AbacusAuxilliary::complement_with_group_assignments(
                    AbacusAuxilliary::load_coord_list(positive_list)?,
                    &groups,
                )?;
                let exclude_coords = AbacusAuxilliary::complement_with_group_assignments(
                    AbacusAuxilliary::load_coord_list(negative_list)?,
                    &groups,
                )?;

                AbacusAuxilliary {
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
        coords: Option<HashSet<PathSegment>>,
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
                                let msg = format!("invalid coordinate \"{}\": group identifiers are not allowed to have start/stop information!", &p);
                                log::error!("{}", &msg);
                                Err(std::io::Error::new( std::io::ErrorKind::InvalidData, msg))
                            } else {
                                let paths = group2ps.get(&p.id()).unwrap().clone();
                                log::debug!("complementing coordinate list with {} paths associted with group {}", paths.len(), p.id());
                                Ok(paths)
                            }
                        } else {
                            let msg = format!("unknown path/group {}", &p);
                            log::error!("{}", &msg);
                            Err(std::io::Error::new(std::io::ErrorKind::InvalidData, msg))
                        }
                    })
                    .collect::<Result<Vec<Vec<PathSegment>>, std::io::Error>>().map(|x| Some(x[..]
                    .concat()))
            }
        }
    }

    fn load_coord_list(file_name: &str) -> Result<Option<HashSet<PathSegment>>, std::io::Error> {
        Ok(if file_name.is_empty() {
            None
        } else {
            log::info!("loading coordinates from {}", file_name);
            let mut data = std::io::BufReader::new(fs::File::open(file_name)?);
            let coords = HashSet::from_iter(io::parse_bed(&mut data).into_iter());
            log::debug!("loaded {} coordinates", coords.len());
            Some(coords)
        })
    }

    fn load_groups(
        file_name: &str,
        graph_aux: &GraphAuxilliary,
    ) -> Result<HashMap<PathSegment, String>, std::io::Error> {
        Ok(if file_name.is_empty() {
            log::info!("no explicit group file given, group paths by their IDs (sample ID+haplotype ID+seq ID)");
            graph_aux
                .path_segments
                .iter()
                .cloned()
                .zip(graph_aux.path_segments.iter().map(|x| x.id()))
                .collect()
        } else {
            log::info!("loading groups from {}", file_name);
            let mut data = std::io::BufReader::new(fs::File::open(file_name)?);
            let g = io::parse_groups(&mut data)?;
            log::debug!("loaded {} group assignments", g.len());
            let mut ps2group: HashMap<PathSegment, String> = g.into_iter().collect();
            graph_aux
                .path_segments
                .iter()
                .map(|x| (x.clone(), ps2group.remove(x).unwrap_or(x.id())))
                .collect()
        })
    }
}

#[derive(Debug, Clone)]
pub struct Abacus {
    pub count: CountType,
    pub countable: Vec<CountSize>,
    pub uncovered_bps: HashMap<ItemIdSize, usize>,
    pub groups: Vec<String>,
    pub graph_aux: GraphAuxilliary,
}

impl Abacus {
    pub fn from_gfa<R: std::io::Read>(
        data: &mut std::io::BufReader<R>,
        abacus_aux: AbacusAuxilliary,
        graph_aux: GraphAuxilliary,
    ) -> Self {
        log::info!("parsing path + walk sequences");
        let (item_table, exclude_table, subset_covered_bps) =
            io::parse_gfa_itemcount(data, &abacus_aux, &graph_aux);
        log::info!("counting abacus entries..");
        // first element in countable is the "zero" element--which should be ignored in
        // counting
        let mut countable: Vec<CountSize> =
            vec![0; graph_aux.number_of_items(&abacus_aux.count) + 1];
        // countable with ID "0" is special and should not be considered in coverage histogram
        countable[0] = ItemIdSize::MAX;
        let mut last: Vec<ItemIdSize> =
            vec![ItemIdSize::MAX; graph_aux.number_of_items(&abacus_aux.count) + 1];

        let mut groups = Vec::new();
        for (path_id, group_id) in Abacus::get_path_order(&abacus_aux, &graph_aux.path_segments) {
            if groups.is_empty() || groups.last().unwrap() != group_id {
                groups.push(group_id.to_string());
            }
            Abacus::coverage(
                &mut countable,
                &mut last,
                &item_table,
                &exclude_table,
                path_id,
                groups.len() as ItemIdSize - 1,
            );
        }

        Self {
            count: abacus_aux.count,
            countable: countable,
            uncovered_bps: Self::quantify_uncovered_bps(
                &exclude_table,
                &subset_covered_bps,
                &graph_aux,
            ),
            groups: groups,
            graph_aux: graph_aux,
        }
    }

    fn coverage(
        countable: &mut Vec<CountSize>,
        last: &mut Vec<ItemIdSize>,
        item_table: &ItemTable,
        exclude_table: &Option<ActiveTable>,
        path_id: ItemIdSize,
        group_id: ItemIdSize,
    ) {
        let countable_ptr = Wrap(countable);
        let last_ptr = Wrap(last);

        // Parallel node counting
        (0..SIZE_T).into_par_iter().for_each(|i| {
            let start = item_table.id_prefsum[i][path_id as usize] as usize;
            let end = item_table.id_prefsum[i][path_id as usize + 1] as usize;
            for j in start..end {
                let sid = item_table.items[i][j] as usize;
                unsafe {
                    if (exclude_table.is_none() || !exclude_table.as_ref().unwrap().items[sid])
                        && last[sid] != group_id
                    {
                        (*countable_ptr.0)[sid] += 1;
                        (*last_ptr.0)[sid] = group_id;
                    }
                }
            }
        });
    }

    fn coverage_by_group_id(
        countable: &mut Vec<Vec<CountSize>>,
        last: &mut Vec<ItemIdSize>,
        item_table: &ItemTable,
        exclude_table: &Option<ActiveTable>,
        path_id: ItemIdSize,
        group_id: ItemIdSize,
    ) {
        let countable_ptr = Wrap(countable);
        let last_ptr = Wrap(last);

        // Parallel node counting
        (0..SIZE_T).into_par_iter().for_each(|i| {
            let start = item_table.id_prefsum[i][path_id as usize] as usize;
            let end = item_table.id_prefsum[i][path_id as usize + 1] as usize;
            for j in start..end {
                let sid = item_table.items[i][j] as usize;
                unsafe {
                    if exclude_table.is_none() || !exclude_table.as_ref().unwrap().items[sid]
                    {
                        (*countable_ptr.0)[group_id as usize][sid] += 1;
                        (*last_ptr.0)[sid] = group_id;
                    }
                }
            }
        });
    }

    fn quantify_uncovered_bps(
        exclude_table: &Option<ActiveTable>,
        subset_covered_bps: &Option<IntervalContainer>,
        graph_aux: &GraphAuxilliary,
    ) -> HashMap<ItemIdSize, usize> {
        //
        // 1. if subset is specified, then the node-based coverage calculated by the coverage()
        //    function overestimates the total coverage, because even nodes that are only partially
        //    covered are counted, thus the coverage needs to be reduced by the amount of uncovered
        //    bps from partially covered nodes
        // 2. if exclude is specified, then the coverage is overestimated by the coverage()
        //    function because partially excluded nodes are not excluded in the coverage
        //    calculation, thus the bps coverage needs to be reduced by the amount of excluded bps
        //    from partially excluded nodes
        // 3. if subset AND exclude are specified, nodes that are COMPLETELY excluded have not been
        //    counted in coverage, so they should not be considered here; all other nodes that are
        //    partially excluded / subset have contributed to the overestimation of coverage, so
        //    the bps coverage needs to be reduced by the amount of excluded or not coverered by
        //    any subset interval
        let mut res = HashMap::default();

        if let Some(subset_map) = subset_covered_bps {
            for sid in subset_map.keys() {
                // ignore COMPETELY excluded nodes
                if exclude_table.is_none() || !exclude_table.as_ref().unwrap().items[sid.0 as usize]
                {
                    let l = graph_aux.node_len(sid) as usize;
                    let covered = subset_map.total_coverage(
                        sid,
                        &exclude_table
                            .as_ref()
                            .map(|ex| ex.get_active_intervals(sid, l)),
                    );
                    // report uncovered bps
                    res.insert(sid.0, l - covered);
                }
            }
        }
        res
    }

    //Why &self and not self? we could destroy abacus at this point.
    pub fn construct_hist(&self) -> Vec<usize> {
        // hist must be of size = num_groups + 1; having an index that starts from 1, instead of 0,
        // makes easier the calculation in hist2pangrowth.
        let mut hist: Vec<usize> = vec![0; self.groups.len() + 1];

        for (i, cov) in self.countable.iter().enumerate() {
            if *cov as usize >= hist.len() {
                log::info!("coverage {} of item {} exceeds the number of groups {}, it'll be ignored in the count", cov, i, self.groups.len());
            } else {
                hist[*cov as usize] += 1;
            }
        }
        hist
    }

    pub fn uncovered_items(&self) -> Vec<usize> {
        self.countable
            .iter()
            .enumerate()
            .filter_map(|(i, c)| match c {
                0 => Some(i),
                _ => None,
            })
            .collect()
    }

    pub fn construct_hist_bps(&self) -> Vec<usize> {
        // hist must be of size = num_groups + 1; having an index that starts from 1, instead of 0,
        // makes easier the calculation in hist2pangrowth.
        let mut hist: Vec<usize> = vec![0; self.groups.len() + 1];
        for (id, cov) in self.countable.iter().enumerate() {
            if *cov as usize >= hist.len() {
                log::info!("coverage {} of item {} exceeds the number of groups {}, it'll be ignored in the count", cov, id, self.groups.len());
            } else {
                hist[*cov as usize] += self.graph_aux.node_len_ary[id] as usize;
            }
        }

        // subtract uncovered bps
        for (id, uncov) in self.uncovered_bps.iter() {
            hist[self.countable[*id as usize] as usize] -= uncov;
            // add uncovered bps to 0-coverage count
            hist[0] += uncov;
        }
        hist
    }

    fn get_path_order<'a>(
        abacus_aux: &'a AbacusAuxilliary,
        path_segments: &Vec<PathSegment>,
    ) -> Vec<(ItemIdSize, &'a str)> {
        // orders elements of path_segments by the order in abacus_aux.groups; the returned vector
        // maps indices of path_segments to the group identifier

        let mut group_order = Vec::new();
        let mut group_to_paths: HashMap<&str, Vec<&PathSegment>> = HashMap::default();

        let mut path_to_id: HashMap<&PathSegment, ItemIdSize> = HashMap::default();
        path_segments.iter().enumerate().for_each(|(i, s)| {
            path_to_id.insert(s, i as ItemIdSize);
        });

        abacus_aux.groups.iter().for_each(|(k, v)| {
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

    pub fn get_coverage_table<R: std::io::Read>(
        data: &mut std::io::BufReader<R>,
        abacus_aux: &AbacusAuxilliary,
        graph_aux: &GraphAuxilliary,
    ) -> (Vec<Vec<CountSize>>, Vec<String>) {
        log::info!("parsing path + walk sequences");
        let (item_table, exclude_table, subset_covered_bps) =
            io::parse_gfa_itemcount(data, abacus_aux, graph_aux);

        log::info!("allocating storage for coverage table");
        // counting number of groups
        let mut groups = HashSet::new();
        abacus_aux.groups.values().for_each(|x| {groups.insert(x); });
        let mut countable = vec![vec![0; groups.len() ]; graph_aux.number_of_items(&abacus_aux.count) + 1];
        // first element in coverage table is the "zero" element--which should be ignored in
        // counting
        countable[0].iter_mut().for_each(|x| {*x = CountSize::MAX });

        let mut last: Vec<ItemIdSize> =
            vec![ItemIdSize::MAX; graph_aux.number_of_items(&abacus_aux.count) + 1];

        log::info!("producing absence / presence vector for each group");
        let mut groups = Vec::new();
        for (path_id, group_id) in Abacus::get_path_order(abacus_aux, &graph_aux.path_segments) {
            if groups.is_empty() || groups.last().unwrap() != group_id {
                groups.push(group_id.to_string());
            }

            Abacus::coverage_by_group_id(
                &mut countable,
                &mut last,
                &item_table,
                &exclude_table,
                path_id,
                groups.len() as ItemIdSize - 1,
            );
        }
        (countable, groups)
    }
}
