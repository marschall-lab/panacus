/* standard use */
use std::fs;
use std::io::{BufReader, BufWriter, Write};
use std::io::{Error, ErrorKind};
use std::iter::FromIterator;
//use std::sync::{Arc, Mutex};

/* external crate*/
use itertools::Itertools;
use rayon::prelude::*;
use std::collections::{HashMap, HashSet};
use strum::IntoEnumIterator;

/* private use */
use crate::cli::Params;
use crate::graph::*;
use crate::io::*;
use crate::util::*;

pub struct AbacusAuxilliary {
    pub groups: HashMap<PathSegment, String>,
    pub include_coords: Option<Vec<PathSegment>>,
    pub exclude_coords: Option<Vec<PathSegment>>,
    pub order: Option<Vec<PathSegment>>,
}

impl AbacusAuxilliary {
    pub fn from_params(
        params: &Params,
        graph_aux: &GraphAuxilliary,
    ) -> Result<Self, Error> {
        match params {
            Params::Histgrowth          { positive_list, negative_list, groupby, 
                                          groupby_sample, groupby_haplotype, .. }
            | Params::Hist              { positive_list, negative_list, groupby, 
                                          groupby_sample, groupby_haplotype, .. }
            | Params::Stats             { positive_list, negative_list, groupby, 
                                          groupby_sample, groupby_haplotype, .. }
            | Params::Subset             { positive_list, negative_list, groupby, 
                                          groupby_sample, groupby_haplotype, .. }
            | Params::OrderedHistgrowth { positive_list, negative_list, groupby, 
                                          groupby_sample, groupby_haplotype, .. } 
            | Params::Table             { positive_list, negative_list, groupby, 
                                          groupby_sample, groupby_haplotype, .. } 
            | Params::Cdbg              { positive_list, negative_list, groupby, 
                                          groupby_sample, groupby_haplotype, .. } 
            => {
                let groups = AbacusAuxilliary::load_groups(
                    groupby,
                    *groupby_haplotype,
                    *groupby_sample,
                    graph_aux,
                )?;
                let include_coords = AbacusAuxilliary::complement_with_group_assignments(
                    AbacusAuxilliary::load_coord_list(positive_list)?,
                    &groups,
                )?;
                let exclude_coords = AbacusAuxilliary::complement_with_group_assignments(
                    AbacusAuxilliary::load_coord_list(negative_list)?,
                    &groups,
                )?;

                let order = if let Params::OrderedHistgrowth { order, .. } = params {
                    let maybe_order = AbacusAuxilliary::complement_with_group_assignments(
                        AbacusAuxilliary::load_coord_list(order)?,
                        &groups,
                    )?;
                    if let Some(o) = &maybe_order {
                        // if order is given, check that it comprises all included coords
                        let all_included_paths: Vec<PathSegment> = match &include_coords {
                            None => {
                                let exclude: HashSet<&PathSegment> = match &exclude_coords {
                                    Some(e) => e.iter().collect(),
                                    None => HashSet::new(),
                                };
                                graph_aux
                                    .path_segments
                                    .iter()
                                    .filter_map(|x| {
                                        if !exclude.contains(x) {
                                            Some(x.clear_coords())
                                        } else {
                                            None
                                        }
                                    })
                                    .collect()
                            }
                            Some(include) => include.iter().map(|x| x.clear_coords()).collect()
                        };
                        let order_set: HashSet<&PathSegment> = HashSet::from_iter(o.iter());

                        for p in all_included_paths.iter() {
                            if !order_set.contains(p) {
                                let msg = format!(
                                    "order list does not contain information about path {}",
                                    p
                                );
                                log::error!("{}", &msg);
                                // let's not be that harsh, shall we?
                                // return Err(Error::new( ErrorKind::InvalidData, msg));
                            }
                        }

                        // check that groups are not scrambled in include
                        let mut visited: HashSet<&str> = HashSet::new();
                        let mut cur: &str = groups.get(&o[0]).unwrap();
                        for p in o.iter() {
                            let g: &str = groups.get(p).unwrap();
                            if cur != g && !visited.insert(g) {
                                let msg = format!("order of paths contains fragmented groups: path {} belongs to group that is interspersed by one or more other groups", p);
                                log::error!("{}", &msg);
                                return Err(Error::new(
                                    ErrorKind::InvalidData,
                                    msg,
                                ));
                            }
                            cur = g;
                        }
                    }
                    maybe_order
                } else {
                    None
                };

                //let n_groups = HashSet::<&String>::from_iter(groups.values()).len();
                //if n_groups > 65534 {
                //    return Err(Error::new(
                //        ErrorKind::Unsupported,
                //        format!(
                //            "data has {} path groups, but command is not supported for more than 65534",
                //            n_groups
                //        ),
                //    ));
                //}

                Ok(AbacusAuxilliary {
                    groups: groups,
                    include_coords: include_coords,
                    exclude_coords: exclude_coords,
                    order: order,
                })
            }
            _ => Err(Error::new(
                ErrorKind::InvalidData,
                "cannot produce AbacusData from other Param items",
            )),
        }
    }

    fn complement_with_group_assignments(
        coords: Option<Vec<PathSegment>>,
        groups: &HashMap<PathSegment, String>,
    ) -> Result<Option<Vec<PathSegment>>, Error> {
        //
        // We allow coords to be defined via groups; the following code
        // 1. complements coords with path segments from group assignments
        // 2. checks that group-based coordinates don't have start/stop information
        //
        let mut group2paths: HashMap<String, Vec<PathSegment>> = HashMap::default();
        for (p, g) in groups.iter() {
            group2paths
                .entry(g.clone())
                .or_insert(Vec::new())
                .push(p.clone())
        }
        let path_to_group: HashMap<PathSegment, String> = groups
            .iter()
            .map(|(ps, g)| (ps.clear_coords(), g.clone()))
            .collect();

        match coords {
            None => Ok(None),
            Some(v) => {
                v.into_iter()
                    .map(|p| {
                        // check if path segment defined in coords associated with a specific path,
                        // it is not considered a group 
                        if path_to_group.contains_key(&p.clear_coords()) {
                            Ok(vec![p])
                        } else if group2paths.contains_key(&p.id()) {
                            if p.coords().is_some() {
                                let msg = format!("invalid coordinate \"{}\": group identifiers are not allowed to have start/stop information!", &p);
                                log::error!("{}", &msg);
                                Err(Error::new( ErrorKind::InvalidData, msg))
                            } else {
                                let paths = group2paths.get(&p.id()).unwrap().clone();
                                log::debug!("complementing coordinate list with {} paths associted with group {}", paths.len(), p.id());
                                Ok(paths)
                            }
                        } else {
                            let msg = format!("unknown path/group {}", &p);
                            log::error!("{}", &msg);
                            // let's not be so harsh as to throw an error, ok?
                            // Err(Error::new(ErrorKind::InvalidData, msg))
                            Ok(Vec::new())
                        }
                    })
                    .collect::<Result<Vec<Vec<PathSegment>>, Error>>().map(|x| Some(x[..]
                    .concat()))
            }
        }
    }

    fn load_coord_list(file_name: &str) -> Result<Option<Vec<PathSegment>>, Error> {
        Ok(if file_name.is_empty() {
            None
        } else {
            log::info!("loading coordinates from {}", file_name);
            let mut data = BufReader::new(fs::File::open(file_name)?);
            let coords = parse_bed(&mut data);
            log::debug!("loaded {} coordinates", coords.len());
            Some(coords)
        })
    }

    fn load_groups(
        file_name: &str,
        groupby_haplotype: bool,
        groupby_sample: bool,
        graph_aux: &GraphAuxilliary,
    ) -> Result<HashMap<PathSegment, String>, Error> {
        if groupby_haplotype {
            Ok(graph_aux
                .path_segments
                .iter()
                .map(|x| {
                    (
                        x.clear_coords(),
                        format!(
                            "{}#{}",
                            &x.sample,
                            &x.haplotype.as_ref().unwrap_or(&String::new())
                        ),
                    )
                })
                .collect())
        } else if groupby_sample {
            Ok(graph_aux
                .path_segments
                .iter()
                .map(|x| (x.clear_coords(), x.sample.clone()))
                .collect())
        } else if !file_name.is_empty() {
            log::info!("loading groups from {}", file_name);
            let mut data = BufReader::new(fs::File::open(file_name)?);
            let group_assignments = parse_groups(&mut data)?;
            let mut path_to_group = HashMap::default();
            for (i, (path, group)) in group_assignments.into_iter().enumerate() {
                let path_nocoords = path.clear_coords();
                match path_to_group.get(&path_nocoords) {
                    Some(g) => {
                        if g != &group {
                            let msg = format!(
                                "error in line {}: path {} cannot be assigned to more than one group, but is assigned to at least two groups: {}, {}",
                                i, &path_nocoords, &g, &group
                            );
                            log::error!("{}", &msg);
                            return Err(Error::new(ErrorKind::InvalidData, msg));
                        }
                    }
                    None => {
                        path_to_group.insert(path_nocoords, group);
                    }
                }
            }
            log::debug!("loaded {} group assignments", path_to_group.len());

            // augment the group assignments with yet unassigned path segments
            graph_aux.path_segments.iter().for_each(|x| {
                let path = x.clear_coords();
                if !path_to_group.contains_key(&path) {
                    path_to_group.insert(path, x.id());
                }
            });
            Ok(path_to_group)
        } else {
            log::info!("no explicit grouping instruction given, group paths by their IDs (sample ID+haplotype ID+seq ID)");
            Ok(graph_aux
                .path_segments
                .iter()
                .map(|x| (x.clear_coords(), x.id()))
                .collect())
        }
    }

    fn get_path_order<'a>(
        &'a self,
        path_segments: &Vec<PathSegment>,
    ) -> Result<Vec<(ItemIdSize, &'a str)>, Error> {
        // orders elements of path_segments by the order in abacus_aux.include; the returned vector
        // maps indices of path_segments to the group identifier

        let mut group_to_paths: HashMap<&'a str, Vec<(ItemIdSize, &'a str)>> = HashMap::default();

        for (i, p) in path_segments.into_iter().enumerate() {
            let group: &'a str = self.groups.get(&p.clear_coords()).unwrap();
            group_to_paths
                .entry(group)
                .or_insert(Vec::new())
                .push((i as ItemIdSize, group));
        }

        let order: Vec<&PathSegment> = if let Some(order) = &self.order {
            order.iter().collect()
        } else if let Some(include) = &self.include_coords {
            include.iter().collect()
        } else {
            let exclude: HashSet<&PathSegment> = match &self.exclude_coords {
                Some(e) => e.iter().collect(),
                None => HashSet::new(),
            };
            path_segments
                .into_iter()
                .filter_map(|x| if !exclude.contains(x) { Some(x) } else { None })
                .collect::<Vec<&PathSegment>>()
        };
        Ok(order
            .into_iter()
            .map(|p| {
                group_to_paths
                    .remove(&self.groups.get(&p.clear_coords()).unwrap()[..])
                    .unwrap_or(Vec::new())
            })
            .collect::<Vec<Vec<(ItemIdSize, &'a str)>>>()
            .concat())
    }

    #[allow(dead_code)]
    pub fn count_groups(&self) -> usize {
        HashSet::<&String>::from_iter(self.groups.values()).len()
    }
}

#[derive(Debug, Clone)]
pub struct AbacusByTotal<'a> {
    pub count: CountType,
    pub countable: Vec<CountSize>,
    pub uncovered_bps: HashMap<ItemIdSize, usize>,
    pub groups: Vec<String>,
    pub graph_aux: &'a GraphAuxilliary,
}

impl<'a> AbacusByTotal<'a> {
    pub fn from_gfa<R: std::io::Read>(
        data: &mut BufReader<R>,
        abacus_aux: &AbacusAuxilliary,
        graph_aux: &'a GraphAuxilliary,
        count: CountType,
    ) -> Result<Self, Error> {
        log::info!("parsing path + walk sequences");
        let (item_table, exclude_table, subset_covered_bps, _paths_len) =
            parse_gfa_itemcount(data, abacus_aux, graph_aux, &count);
        log::info!("counting abacus entries..");
        
        // first element in countable is "zero" element. It is ignored in counting
        let mut countable: Vec<CountSize> = vec![0; graph_aux.number_of_items(&count) + 1];
        // countable with ID "0" is special and should not be considered in coverage histogram
        countable[0] = CountSize::MAX;
        let mut last: Vec<ItemIdSize> = vec![ItemIdSize::MAX; graph_aux.number_of_items(&count) + 1];

        let mut groups = Vec::new();
        for (path_id, group_id) in abacus_aux.get_path_order(&graph_aux.path_segments)? {
            if groups.is_empty() || groups.last().unwrap() != group_id {
                groups.push(group_id.to_string());
            }

            //log::debug!("computing coverage of {} {}..", path_id, group_id);
            AbacusByTotal::coverage(
                &mut countable,
                &mut last,
                &item_table,
                &exclude_table,
                path_id,
                groups.len() as ItemIdSize - 1,
            );
        }

        log::info!(
            "abacus has {} path groups and {} countables",
            groups.len(),
            countable.len()-1,
        );

        Ok(Self {
            count: count,
            countable: countable,
            uncovered_bps: quantify_uncovered_bps(&exclude_table, &subset_covered_bps, &graph_aux),
            groups: groups,
            graph_aux: graph_aux,
        })
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
                    if last[sid] != group_id
                        && (exclude_table.is_none() || !exclude_table.as_ref().unwrap().items[sid])
                    {
                        (*countable_ptr.0)[sid] += 1;
                        (*last_ptr.0)[sid] = group_id;
                    }
                }
            }
        });
    }

    pub fn abaci_from_gfa(gfa_file: &str, count: CountType, graph_aux: &'a GraphAuxilliary, 
        abacus_aux: &AbacusAuxilliary) -> Result<Vec<Self>, Error> {
        let mut abaci = Vec::new();
        if let CountType::All = count {
            for count_type in CountType::iter() {
                if let CountType::All = count_type { }
                else {
                    let mut data = bufreader_from_compressed_gfa(gfa_file);
                    let abacus = AbacusByTotal::from_gfa(
                        &mut data,
                        &abacus_aux,
                        &graph_aux,
                        count_type,
                    )?;
                    abaci.push(abacus);
                }
            }
        } else {
            let mut data = bufreader_from_compressed_gfa(gfa_file);
            let abacus = AbacusByTotal::from_gfa(
                &mut data,
                &abacus_aux,
                &graph_aux,
                count,
            )?;
            abaci.push(abacus);
        }
        Ok(abaci)
    }

    pub fn construct_hist(&self) -> Vec<usize> {
        log::info!("constructing histogram..");
        // hist must be of size = num_groups + 1; having an index that starts 
        // from 1, instead of 0, makes easier the calculation in hist2pangrowth.
        let mut hist: Vec<usize> = vec![0; self.groups.len() + 1];

        for (i, cov) in self.countable.iter().enumerate() {
            if *cov as usize >= hist.len() {
                if i != 0 {
                    log::warn!("coverage {} of item {} exceeds the number of groups {}, it'll be ignored in the count", cov, i, self.groups.len());
                }
            } else {
                hist[*cov as usize] += 1;
            }
        }
        hist
    }

    pub fn construct_hist_bps(&self) -> Vec<usize> {
        log::info!("constructing bp histogram..");
        // hist must be of size = num_groups + 1; having an index that starts 
        // from 1, instead of 0, makes easier the calculation in hist2pangrowth.
        let mut hist: Vec<usize> = vec![0; self.groups.len() + 1];
        for (id, cov) in self.countable.iter().enumerate() {
            if *cov as usize >= hist.len() {
                if id != 0 {
                    log::info!("coverage {} of item {} exceeds the number of groups {}, it'll be ignored in the count", cov, id, self.groups.len());
                }
            } else {
                hist[*cov as usize] += self.graph_aux.node_lens[id] as usize;
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
}

#[derive(Debug, Clone)]
pub struct AbacusByGroup<'a> {
    pub count: CountType,
    pub r: Vec<usize>,
    pub v: Option<Vec<CountSize>>,
    pub c: Vec<GroupSize>,
    pub uncovered_bps: HashMap<ItemIdSize, usize>,
    pub groups: Vec<String>,
    pub graph_aux: &'a GraphAuxilliary,
}

impl<'a> AbacusByGroup<'a> {
    pub fn from_gfa<R: std::io::Read>(
        data: &mut std::io::BufReader<R>,
        abacus_aux: &AbacusAuxilliary,
        graph_aux: &'a GraphAuxilliary,
        count: CountType,
        report_values: bool,
    ) -> Result<Self, Error> {
        log::info!("parsing path + walk sequences");
        let (item_table, exclude_table, subset_covered_bps, _paths_len) =
            parse_gfa_itemcount(data, abacus_aux, graph_aux, &count);

        let mut path_order: Vec<(ItemIdSize, GroupSize)> = Vec::new();
        let mut groups: Vec<String> = Vec::new();

        for (path_id, group_id) in abacus_aux.get_path_order(&graph_aux.path_segments)? {
            log::debug!(
                "processing path {} (group {})",
                &graph_aux.path_segments[path_id as usize],
                group_id
            );
            if groups.is_empty() || groups.last().unwrap() != group_id {
                groups.push(group_id.to_string());
            }
            //if groups.len() > 65534 {
            //    panic!("data has more than 65534 path groups, but command is not supported for more than 65534");
            //}
            path_order.push((path_id, (groups.len() - 1) as GroupSize));
        }

        let r = AbacusByGroup::compute_row_storage_space(
            &item_table,
            &exclude_table,
            &path_order,
            graph_aux.number_of_items(&count),
        );
        let (v, c) =
            AbacusByGroup::compute_column_values(&item_table, &path_order, &r, report_values);
        log::info!(
            "abacus has {} path groups and {} countables",
            groups.len(),
            r.len()
        );

        Ok(Self {
            count: count,
            r: r,
            v: v,
            c: c,
            uncovered_bps: quantify_uncovered_bps(&exclude_table, &subset_covered_bps, graph_aux),
            groups: groups,
            graph_aux: graph_aux,
        })
    }

    fn compute_row_storage_space(
        item_table: &ItemTable,
        exclude_table: &Option<ActiveTable>,
        path_order: &Vec<(ItemIdSize, GroupSize)>,
        n_items: usize,
    ) -> Vec<usize> {
        log::info!("computing space allocating storage for group-based coverage table:");
        let mut last: Vec<GroupSize> = vec![GroupSize::MAX; n_items + 1];
        let last_ptr = Wrap(&mut last);

        let mut r: Vec<usize> = vec![0; n_items + 2];
        let r_ptr = Wrap(&mut r);
        for (path_id, group_id) in path_order {
            (0..SIZE_T).into_par_iter().for_each(|i| {
                let start = item_table.id_prefsum[i][*path_id as usize] as usize;
                let end = item_table.id_prefsum[i][*path_id as usize + 1] as usize;
                for j in start..end {
                    let sid = item_table.items[i][j] as usize;
                    if &last[sid] != group_id
                        && (exclude_table.is_none() || !exclude_table.as_ref().unwrap().items[sid])
                    {
                        unsafe {
                            (*r_ptr.0)[sid] += 1;
                            (*last_ptr.0)[sid] = *group_id;
                        }
                    }
                }
            });
        }
        log::info!(" ++ assigning storage locations");
        let mut c = 0;
        // can this be simplified?
        for i in 0..r.len() {
            let tmp = r[i];
            r[i] = c;
            c += tmp;
        }
        log::info!(
            " ++ group-aware table has {} non-zero elements",
            r.last().unwrap()
        );
        r
    }

    fn compute_column_values(
        item_table: &ItemTable,
        path_order: &Vec<(ItemIdSize, GroupSize)>,
        r: &Vec<usize>,
        report_values: bool,
    ) -> (Option<Vec<CountSize>>, Vec<GroupSize>) {
        let n = *r.last().unwrap() as usize;
        log::info!("allocating storage for group-based coverage table..");
        let mut v = if report_values {
            vec![0; n]
        } else {
            // we produce a dummy
            vec![0; 1]
        };
        let mut c: Vec<GroupSize> = vec![GroupSize::MAX; n];
        log::info!("done");

        log::info!("computing group-based coverage..");
        let v_ptr = Wrap(&mut v);
        let c_ptr = Wrap(&mut c);

        // group id is monotone increasing from 0 to #groups
        for (path_id, group_id) in path_order {
            let path_id_u = *path_id as usize;
            (0..SIZE_T).into_par_iter().for_each(|i| {
                let start = item_table.id_prefsum[i][path_id_u] as usize;
                let end = item_table.id_prefsum[i][path_id_u + 1] as usize;
                for j in start..end {
                    let sid = item_table.items[i][j] as usize;
                    let cv_start = r[sid];
                    let mut cv_end = r[sid + 1];
                    if cv_end != cv_start {
                        // look up storage location for node cur_sid: we use the last position
                        // of interval cv_start..cv_end, which is associated to coverage counts
                        // of the current node (sid), in the "c" array as pointer to the
                        // current column (group) / value (coverage) position. If the current group
                        // id does not match the one associated with the current position, we move
                        // on to the next. If cv_start + p == cv_end - 1, this means that we are
                        // currently writing the last element in that interval, and we need to make
                        // sure that we are no longer using it as pointer.
                        if cv_end - 1 > c.len() {
                            log::error!(
                                "oops, cv_end-1 is larger than the length of c for sid={}",
                                sid
                            );
                            cv_end = c.len() - 1;
                        }

                        let mut p = c[cv_end - 1] as usize;
                        unsafe {
                            // we  look at an untouched interval, so let's get the pointer game
                            // started...
                            if c[cv_end - 1] == GroupSize::MAX {
                                (*c_ptr.0)[cv_start] = *group_id;
                                // if it's just a single value in this interval, the pointer game
                                // ends before it started
                                if cv_start < cv_end - 1 {
                                    (*c_ptr.0)[cv_end - 1] = 0;
                                }
                                if report_values {
                                    (*v_ptr.0)[cv_start] += 1;
                                }
                            } else if cv_start + p < cv_end - 1 {
                                // if group id of current slot does not match current group id
                                // (remember group id's are strictly monotically increasing), then
                                // move on to the next slot
                                if c[cv_start + p] < *group_id {
                                    // move on to the next slot
                                    (*c_ptr.0)[cv_end - 1] += 1;
                                    // update local pointer
                                    p += 1;
                                    (*c_ptr.0)[cv_start + p] = *group_id
                                }
                                if report_values {
                                    (*v_ptr.0)[cv_start + p] += 1;
                                }
                            } else if report_values {
                                // make sure it points to the last element and not beyond
                                (*v_ptr.0)[cv_end - 1] += 1;
                            }
                        }
                    }
                }
            });
        }
        log::info!("done");
        (if report_values { Some(v) } else { None }, c)
    }

    // why &self and not self? we could destroy abacus at this point.
    pub fn calc_growth(&self, t_coverage: &Threshold, t_quorum: &Threshold) -> Vec<f64> {
        let mut res = vec![0.0; self.groups.len()];

        let c = usize::max(1, t_coverage.to_absolute(self.groups.len()));
        let q = f64::max(0.0, t_quorum.to_relative(self.groups.len()));

        let mut it = self.r.iter().tuple_windows().enumerate();
        // ignore first entry
        it.next();
        for (i, (&start, &end)) in it {
            if end - start >= c {
                let mut k = start;
                for j in self.c[start] as usize..self.groups.len() {
                    if k < end - 1 && self.c[k + 1] as usize <= j {
                        k += 1
                    }
                    if k - start + 1 >= ((self.c[k] as f64 + 1.0) * q).ceil() as usize {
                        // we never need to look into the actual value in self.v, because we
                        // know it must be non-zero, which is sufficient
                        match self.count {
                            CountType::Node | CountType::Edge => res[j] += 1.0,
                            CountType::Bp => {
                                let uncovered =
                                    self.uncovered_bps.get(&(i as ItemIdSize)).unwrap_or(&0);
                                let covered = self.graph_aux.node_lens[i] as usize;
                                if uncovered > &covered {
                                    log::error!("oops, #uncovered bps ({}) is larger than #coverd bps ({}) for node with sid {})", &uncovered, &covered, i);
                                } else {
                                    res[j] += (covered - uncovered) as f64
                                }
                            }
                            CountType::All => unreachable!("inadmissible count type"),
                        }
                    }
                }
            }
        }
        res
    }

    #[allow(dead_code)]
    pub fn write_rcv<W: Write>(&self, out: &mut BufWriter<W>) -> Result<(), Error> {
        write!(out, "{}", self.r[0])?;
        for x in self.r[1..].iter() {
            write!(out, "\t{}", x)?;
        }
        writeln!(out, "")?;
        write!(out, "{}", self.c[0])?;
        for x in self.c[1..].iter() {
            write!(out, "\t{}", x)?;
        }
        writeln!(out, "")?;
        if let Some(v) = &self.v {
            write!(out, "{}", v[0])?;
            for x in v[1..].iter() {
                write!(out, "\t{}", x)?;
            }
            writeln!(out, "")?;
        };
        Ok(())
    }

    pub fn to_tsv<W: Write>(
        &self,
        total: bool,
        out: &mut BufWriter<W>,
    ) -> Result<(), Error> {
        // create mapping from numerical node ids to original node identifiers
        log::info!("reporting coverage table");
        let dummy = Vec::new();
        let mut id2node: Vec<&Vec<u8>> = vec![&dummy; self.graph_aux.node_count + 1];
        for (node, id) in self.graph_aux.node2id.iter() {
            id2node[id.0 as usize] = node;
        }

        match self.count {
            CountType::Node | CountType::Bp => {
                write!(out, "node")?;
                if total {
                    write!(out, "\ttotal")?;
                } else {
                    for group in self.groups.iter() {
                        write!(out, "\t{}", group)?;
                    }
                }
                writeln!(out, "")?;

                let mut it = self.r.iter().tuple_windows().enumerate();
                // ignore first entry
                it.next();
                for (i, (&start, &end)) in it {
                    let bp = if self.count == CountType::Bp {
                        self.graph_aux.node_lens[i] as usize
                            - *self.uncovered_bps.get(&(i as ItemIdSize)).unwrap_or(&0)
                    } else {
                        1
                    };
                    write!(out, "{}", std::str::from_utf8(id2node[i]).unwrap())?;
                    if total {
                        // we never need to look into the actual value in self.v, because we
                        // know it must be non-zero, which is sufficient
                        writeln!(out, "\t{}", end - start)?;
                    } else {
                        let mut k = start;
                        for j in 0 as GroupSize..self.groups.len() as GroupSize {
                            if k == end || j < self.c[k] {
                                write!(out, "\t0")?;
                            } else if j == self.c[k] {
                                match &self.v {
                                    None => write!(out, "\t{}", bp),
                                    Some(v) => write!(out, "\t{}", v[k] as usize * bp),
                                }?;
                                k += 1;
                            }
                        }
                        writeln!(out, "")?;
                    }
                }
            }
            CountType::Edge => {
                if let Some(edge2id) = &self.graph_aux.edge2id {
                    let dummy_edge = Edge(
                        ItemId(0),
                        Orientation::default(),
                        ItemId(0),
                        Orientation::default(),
                    );
                    let mut id2edge: Vec<&Edge> =
                        vec![&dummy_edge; self.graph_aux.edge_count + 1];
                    for (edge, id) in edge2id.iter() {
                        id2edge[id.0 as usize] = edge;
                    }

                    write!(out, "edge")?;
                    if total {
                        write!(out, "\ttotal")?;
                    } else {
                        for group in self.groups.iter() {
                            write!(out, "\t{}", group)?;
                        }
                    }
                    writeln!(out, "")?;

                    let mut it = self.r.iter().tuple_windows().enumerate();
                    // ignore first entry
                    it.next();
                    for (i, (&start, &end)) in it {
                        let edge = id2edge[i];
                        let start = start as usize;
                        let end = end as usize;
                        write!(
                            out,
                            "{}{}{}{}",
                            edge.1,
                            std::str::from_utf8(id2node[edge.0 .0 as usize]).unwrap(),
                            edge.3,
                            std::str::from_utf8(id2node[edge.2 .0 as usize]).unwrap(),
                        )?;
                        if total {
                            // we never need to look into the actual value in self.v, because we
                            // know it must be non-zero, which is sufficient
                            writeln!(out, "\t{}", end - start)?;
                        } else {
                            let mut k = start;
                            for j in 0 as GroupSize..self.groups.len() as GroupSize {
                                if k == end || j < self.c[k] {
                                    write!(out, "\t0")?;
                                } else if j == self.c[k] {
                                    match &self.v {
                                        None => write!(out, "\t1"),
                                        Some(v) => write!(out, "\t{}", v[j as usize]),
                                    }?;
                                    k += 1;
                                }
                            }
                            writeln!(out, "")?;
                        }
                    }
                }
            }
            CountType::All => unreachable!("inadmissible count type"),
        };

        Ok(())
    }
}

//pub enum Abacus<'a> {
//    Total(AbacusByTotal<'a>),
//    Group(AbacusByGroup<'a>),
//    Nil,
//}

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
            if exclude_table.is_none() || !exclude_table.as_ref().unwrap().items[sid.0 as usize] {
                let l = graph_aux.node_len(sid) as usize;
                let covered = subset_map.total_coverage(
                    sid,
                    &exclude_table
                        .as_ref()
                        .map(|ex| ex.get_active_intervals(sid, l)),
                );
                if covered > l {
                    log::error!("oops, total coverage {} is larger than node length {} for node {}, intervals: {:?}", covered, l, sid.0, subset_map.get(sid).unwrap());
                } else {
                    // report uncovered bps
                    res.insert(sid.0, l - covered);
                }
            }
        }
    }
    res
}
