/* standard use */
use std::fs;
use std::io::{BufWriter, Write};
use std::iter::FromIterator;
//use std::sync::{Arc, Mutex};

/* external crate*/
use itertools::Itertools;
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
    pub order: Option<Vec<PathSegment>>,
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
            }
            | Params::OrderedHistgrowth {
                count,
                positive_list,
                negative_list,
                groupby,
                ..
            }
            | Params::Table {
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

                let order = if let Params::OrderedHistgrowth { order, .. } = params {
                    let maybe_order = AbacusAuxilliary::complement_with_group_assignments(
                        AbacusAuxilliary::load_coord_list(order)?,
                        &groups,
                    )?;
                    if let Some(o) = &maybe_order {
                        // if order is given, check that it comprises all included coords
                        let all_included_paths: Vec<&PathSegment> = match &include_coords {
                            None => groups.keys().collect(),
                            Some(include) => include.iter().collect(),
                        };
                        let order_set: HashSet<&PathSegment> = HashSet::from_iter(o.iter());

                        for p in all_included_paths.iter() {
                            if !order_set.contains(p) {
                                let msg = format!(
                                    "order list does not contain information about path {}",
                                    p
                                );
                                log::error!("{}", &msg);
                                return Err(std::io::Error::new(
                                    std::io::ErrorKind::InvalidData,
                                    msg,
                                ));
                            }
                        }
                    }
                    maybe_order
                } else {
                    None
                };

                AbacusAuxilliary {
                    count: count.clone(),
                    groups: groups,
                    include_coords: include_coords,
                    exclude_coords: exclude_coords,
                    order: order,
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
        for (p, g) in groups.iter() {
            group2ps
                .entry(g.clone())
                .or_insert(Vec::new())
                .push(p.clone())
        }
        match coords {
            None => Ok(None),
            Some(v) => {
                v.into_iter()
                    .map(|p| {
                        // check if path segment defined in coords associated with a specific path
                        // segment (i.e., is not a group) by querying the keys of the "groups"
                        // hashmap
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

    fn get_path_order<'a>(
        &'a self,
        path_segments: &Vec<PathSegment>,
    ) -> Result<Vec<(ItemIdSize, &'a str)>, std::io::Error> {
        // orders elements of path_segments by the order in abacus_aux.include; the returned vector
        // maps indices of path_segments to the group identifier

        let order = if self.order.is_some() {
            self.order.as_ref()
        } else {
            self.include_coords.as_ref()
        };

        match order {
            None => {
                let mut group_to_paths: HashMap<&'a str, Vec<(ItemIdSize, &'a str)>> =
                    HashMap::default();
                let mut groups: Vec<&'a str> = Vec::new();
                for (i, s) in path_segments.into_iter().enumerate() {
                    let group = self.groups.get(s).unwrap();
                    group_to_paths
                        .entry(group)
                        .or_insert({
                            groups.push(group);
                            Vec::new()
                        })
                        .push((i as ItemIdSize, group));
                }
                Ok(groups
                    .into_iter()
                    .map(|g| group_to_paths.remove(g).unwrap_or(Vec::new()))
                    .collect::<Vec<Vec<(ItemIdSize, &'a str)>>>()
                    .concat())
            }
            Some(o) => {
                // check that groups are not scrambled in include
                let mut visited: HashSet<&'a str> = HashSet::new();
                let mut cur: &'a str = &self.groups.get(&o[0]).unwrap();
                for p in o.iter() {
                    let g = self.groups.get(p).unwrap();
                    if cur != g && !visited.insert(g) {
                        let msg = format!("order of paths contains fragmented groups: path {} belongs to group that is interspersed by one or more other groups", p);
                        log::error!("{}", &msg);
                        return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, msg));
                    }
                    cur = g;
                }

                let mut path_to_id: HashMap<&PathSegment, ItemIdSize> = path_segments
                    .into_iter()
                    .enumerate()
                    .map(|(i, p)| (p, i as ItemIdSize))
                    .collect();
                Ok(o.iter()
                    .map(|p| {
                        (
                            path_to_id.remove(p).expect(&format!(
                                "path segment {} occurs more than once in path order list",
                                p
                            )),
                            &self.groups.get(p).unwrap()[..],
                        )
                    })
                    .collect())
            }
        }
    }

    pub fn count_groups(&self) -> usize {
        HashSet::<&String>::from_iter(self.groups.values()).len()
    }
}

#[derive(Debug, Clone)]
pub struct AbacusByTotal {
    pub count: CountType,
    pub countable: Vec<CountSize>,
    pub uncovered_bps: HashMap<ItemIdSize, usize>,
    pub groups: Vec<String>,
    pub graph_aux: GraphAuxilliary,
}

impl AbacusByTotal {
    pub fn from_gfa<R: std::io::Read>(
        data: &mut std::io::BufReader<R>,
        abacus_aux: AbacusAuxilliary,
        graph_aux: GraphAuxilliary,
    ) -> Result<Self, std::io::Error> {
        log::info!("parsing path + walk sequences");
        let (item_table, exclude_table, subset_covered_bps) =
            io::parse_gfa_itemcount(data, &abacus_aux, &graph_aux);
        log::info!("counting abacus entries..");
        // first element in countable is the "zero" element--which should be ignored in
        // counting
        let mut countable: Vec<CountSize> =
            vec![0; graph_aux.number_of_items(&abacus_aux.count) + 1];
        // countable with ID "0" is special and should not be considered in coverage histogram
        countable[0] = CountSize::MAX;
        let mut last: Vec<ItemIdSize> =
            vec![ItemIdSize::MAX; graph_aux.number_of_items(&abacus_aux.count) + 1];

        let mut groups = Vec::new();
        for (path_id, group_id) in abacus_aux.get_path_order(&graph_aux.path_segments)? {
            if groups.is_empty() || groups.last().unwrap() != group_id {
                groups.push(group_id.to_string());
            }
            AbacusByTotal::coverage(
                &mut countable,
                &mut last,
                &item_table,
                &exclude_table,
                path_id,
                groups.len() as ItemIdSize - 1,
            );
        }

        Ok(Self {
            count: abacus_aux.count,
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

    //Why &self and not self? we could destroy abacus at this point.
    pub fn construct_hist(&self) -> Vec<usize> {
        // hist must be of size = num_groups + 1; having an index that starts from 1, instead of 0,
        // makes easier the calculation in hist2pangrowth.
        let mut hist: Vec<usize> = vec![0; self.groups.len() + 1];

        for (i, cov) in self.countable.iter().enumerate() {
            if *cov as usize >= hist.len() {
                if i != 0 {
                    log::info!("coverage {} of item {} exceeds the number of groups {}, it'll be ignored in the count", cov, i, self.groups.len());
                }
            } else {
                hist[*cov as usize] += 1;
            }
        }
        hist
    }

    pub fn construct_hist_bps(&self) -> Vec<usize> {
        // hist must be of size = num_groups + 1; having an index that starts from 1, instead of 0,
        // makes easier the calculation in hist2pangrowth.
        let mut hist: Vec<usize> = vec![0; self.groups.len() + 1];
        for (id, cov) in self.countable.iter().enumerate() {
            if *cov as usize >= hist.len() {
                if id != 0 {
                    log::info!("coverage {} of item {} exceeds the number of groups {}, it'll be ignored in the count", cov, id, self.groups.len());
                }
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
}

#[derive(Debug, Clone)]
pub struct AbacusByGroup {
    pub count: CountType,
    pub r: Vec<usize>,
    pub v: Option<Vec<CountSize>>,
    pub c: Vec<GroupSize>,
    pub uncovered_bps: HashMap<ItemIdSize, usize>,
    pub groups: Vec<String>,
    pub graph_aux: GraphAuxilliary,
}

impl AbacusByGroup {
    pub fn from_gfa<R: std::io::Read>(
        data: &mut std::io::BufReader<R>,
        abacus_aux: AbacusAuxilliary,
        graph_aux: GraphAuxilliary,
        report_values: bool,
    ) -> Result<Self, std::io::Error> {
        log::info!("parsing path + walk sequences");
        let (item_table, exclude_table, subset_covered_bps) =
            io::parse_gfa_itemcount(data, &abacus_aux, &graph_aux);

        let mut path_order: Vec<(ItemIdSize, GroupSize)> = Vec::new();
        let mut groups: Vec<String> = Vec::new();
        for (path_id, group_id) in abacus_aux.get_path_order(&graph_aux.path_segments)? {
            if groups.is_empty() || groups.last().unwrap() != group_id {
                groups.push(group_id.to_string());
            }
            if groups.len() > 65534 {
                panic!("data has more than 65534 path groups, but command is not supported for more than 65534");
            }
            path_order.push((path_id, (groups.len() - 1) as GroupSize));
        }

        let r = AbacusByGroup::compute_row_storage_space(
            &item_table,
            &exclude_table,
            &path_order,
            graph_aux.number_of_items(&abacus_aux.count),
        );
        let (v, c) = AbacusByGroup::compute_column_values(
            &item_table,
            &exclude_table,
            &path_order,
            &r,
            report_values,
        );

        Ok(Self {
            count: abacus_aux.count,
            r: r,
            v: v,
            c: c,
            uncovered_bps: quantify_uncovered_bps(&exclude_table, &subset_covered_bps, &graph_aux),
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
        exclude_table: &Option<ActiveTable>,
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
                    if exclude_table.is_none() || !exclude_table.as_ref().unwrap().items[sid] {
                        let cv_start = r[sid];
                        let cv_end = r[sid + 1];
                        // look up storage location for node cur_sid: we use the last position
                        // of interval cv_start..cv_end, which is associated to coverage counts
                        // of the current node (sid), in the "c" array as pointer to the
                        // current column (group) / value (coverage) position. If the current group
                        // id does not match the one associated with the current position, we move
                        // on to the next. If cv_start + p == cv_end - 1, this means that we are
                        // currently writing the last element in that interval, and we need to make
                        // sure that we are no longer using it as pointer.
                        if cv_end - 1 > c.len() {
                            log::info!(
                                "oopse, cv_end-1 is larger than the length of c for sid={}",
                                sid
                            );
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

    //Why &self and not self? we could destroy abacus at this point.
    pub fn calc_growth(&self, t_coverage: &Threshold, t_quorum: &Threshold) -> Vec<f64> {
        let mut res = vec![0.0; self.groups.len()];

        let c = usize::max(1, t_coverage.to_absolute(self.groups.len()));
        let q = f64::max(0.0, t_quorum.to_relative(self.groups.len()));

        // start with 1, countable 0 is a forbidden element
        for (i, (&start, &end)) in self.r.iter().tuple_windows().enumerate() {
            if end - start >= c {
                let mut k = start;
                for j in self.c[start] as usize..self.groups.len() {
                    if k < end - 1 && self.c[k + 1] as usize <= j {
                        k += 1
                    }
                    if k - start + 1 >= (self.c[k] as f64 * q).ceil() as usize {
                        // we never need to look into the actual value in self.v, because we
                        // know it must be non-zero, which is sufficient
                        match self.count {
                            CountType::Node | CountType::Edge => res[j] += 1.0,
                            CountType::Bp => {
                                res[j] += (self.graph_aux.node_len_ary[i] as usize
                                    - self.uncovered_bps.get(&(i as ItemIdSize)).unwrap_or(&0))
                                    as f64
                            }
                        }
                    }
                }
            }
        }
        res
    }

    pub fn write_rcv<W: Write>(&self, out: &mut BufWriter<W>) -> Result<(), std::io::Error> {
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
    ) -> Result<(), std::io::Error> {
        // create mapping from numerical node ids to original node identifiers
        let dummy = Vec::new();
        let mut id2node: Vec<&Vec<u8>> = vec![&dummy; self.graph_aux.node2id.len() + 1];
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

                for (i, (&start, &end)) in self.r[1..].iter().tuple_windows().enumerate() {
                    let bp = if self.count == CountType::Bp {
                        self.graph_aux.node_len_ary[i + 1] as usize
                            - *self.uncovered_bps.get(&(i as ItemIdSize + 1)).unwrap_or(&0)
                    } else {
                        1
                    };
                    write!(out, "{}", std::str::from_utf8(id2node[i + 1]).unwrap())?;
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
                if let Some(ref edge2id) = self.graph_aux.edge2id {
                    let dummy_edge = Edge(
                        ItemId(0),
                        Orientation::default(),
                        ItemId(0),
                        Orientation::default(),
                    );
                    let mut id2edge: Vec<&Edge> = vec![&dummy_edge; edge2id.len() + 1];
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

                    for (i, (&start, &end)) in self.r[1..].iter().tuple_windows().enumerate() {
                        let edge = id2edge[i + 1];
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
        };

        Ok(())
    }
}

pub enum Abacus {
    Total(AbacusByTotal),
    Group(AbacusByGroup),
    Nil,
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
            if exclude_table.is_none() || !exclude_table.as_ref().unwrap().items[sid.0 as usize] {
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
