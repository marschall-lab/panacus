/* standard use */
use std::io::{BufReader, BufWriter, Write};
use std::io::{Error};
use std::str::{self};
//use std::sync::{Arc, Mutex};

/* external crate*/
use itertools::Itertools;
use rayon::prelude::*;
use std::collections::{HashMap};
use strum::IntoEnumIterator;

/* private use */
use crate::cli::Params;
use crate::graph::*;
use crate::path::*;
use crate::path_parser::*;
use crate::io::*;
use crate::util::*;

use crate::Bench;

#[derive(Debug, Clone)]
pub struct AbacusByTotal {
    pub count: CountType,
    pub countable: Vec<CountSize>,
    pub uncovered_bps: Option<HashMap<ItemId, usize>>,
    pub groups: Vec<String>,
}

impl AbacusByTotal {
    pub fn from_gfa<R: std::io::Read>(
        data: &mut BufReader<R>,
        path_aux: &PathAuxilliary,
        graph_aux: &GraphAuxilliary,
        count: CountType,
    ) -> Self {

        Bench::start("parse_gfa_paths_walks");
        let (items, path_to_items, item_table, exclude_table, subset_covered_bps, _paths_len) =
            parse_gfa_paths_walks(data, path_aux, graph_aux, &count);
        Bench::end("parse_gfa_paths_walks");

        Bench::start("item_table_to_abacus");
        let tmp = Self::items_to_abacus(
            path_aux,
            graph_aux,
            count,
            items,
            path_to_items,
            exclude_table,
            subset_covered_bps,
        );
        Bench::end("item_table_to_abacus");
        return tmp;
    }

    pub fn items_to_abacus(
        path_aux: &PathAuxilliary,
        graph_aux: &GraphAuxilliary,
        count: CountType,
        items: Vec<ItemId>,
        path_to_items: Vec<usize>,
        exclude_table: Option<ActiveTable>,
        subset_covered_bps: Option<IntervalContainer>,
    ) -> Self {
        log::info!("counting abacus entries..");
        // first element in countable is "zero" element. It is ignored in counting
        let mut countable: Vec<CountSize> = vec![0; graph_aux.number_of_items(&count) + 1];
        // countable with ID "0" is special and should not be considered in coverage histogram
        countable[0] = CountSize::MAX;
        let mut last: Vec<ItemId> =
            vec![ItemId::MAX; graph_aux.number_of_items(&count) + 1];

        let mut groups = Vec::new();
        for (path_id, group_id) in path_aux.get_path_order(&graph_aux.path_segments) {
            if groups.is_empty() || groups.last().unwrap() != group_id {
                groups.push(group_id.to_string());
            }
            AbacusByTotal::coverage_from_items(
                &mut countable,
                &mut last,
                &items,
                &path_to_items,
                &exclude_table,
                path_id,
                groups.len() as ItemId - 1,
            );
        }

        //for i in 0..countable.len() {
        //    let cnt = countable[i];
        //    if cnt == 0 {
        //        print!("{},", i);
        //    }
        //}
        //println!("");

        log::info!(
            "abacus has {} path groups and {} countables",
            groups.len(),
            countable.len() - 1
        );

        Self {
            count: count,
            countable: countable,
            uncovered_bps: Some(quantify_uncovered_bps(
                &exclude_table,
                &subset_covered_bps,
                &graph_aux,
            )),
            groups: groups,
        }
    }

    pub fn item_table_to_abacus(
        path_aux: &PathAuxilliary,
        graph_aux: &GraphAuxilliary,
        count: CountType,
        item_table: ItemTable,
        exclude_table: Option<ActiveTable>,
        subset_covered_bps: Option<IntervalContainer>,
    ) -> Self {
        log::info!("counting abacus entries..");
        // first element in countable is "zero" element. It is ignored in counting
        let mut countable: Vec<CountSize> = vec![0; graph_aux.number_of_items(&count) + 1];
        // countable with ID "0" is special and should not be considered in coverage histogram
        countable[0] = CountSize::MAX;
        let mut last: Vec<ItemId> =
            vec![ItemId::MAX; graph_aux.number_of_items(&count) + 1];

        let mut groups = Vec::new();
        for (path_id, group_id) in path_aux.get_path_order(&graph_aux.path_segments) {
            if groups.is_empty() || groups.last().unwrap() != group_id {
                groups.push(group_id.to_string());
            }
            AbacusByTotal::coverage(
                &mut countable,
                &mut last,
                &item_table,
                &exclude_table,
                path_id,
                groups.len() as ItemId - 1,
            );
        }

        log::info!(
            "abacus has {} path groups and {} countables",
            groups.len(),
            countable.len() - 1
        );

        Self {
            count: count,
            countable: countable,
            uncovered_bps: Some(quantify_uncovered_bps(
                &exclude_table,
                &subset_covered_bps,
                &graph_aux,
            )),
            groups: groups,
        }
    }

    pub fn from_cdbg_gfa<R: std::io::Read>(
        data: &mut BufReader<R>,
        path_aux: &PathAuxilliary,
        graph_aux: &GraphAuxilliary,
        k: usize,
        unimer: &Vec<usize>,
    ) -> Self {
        let item_table = parse_cdbg_gfa_paths_walks(data, graph_aux, k);
        Self::k_plus_one_mer_table_to_abacus(item_table, &path_aux, &graph_aux, k, unimer)
    }

    pub fn k_plus_one_mer_table_to_abacus(
        item_table: ItemTable,
        path_aux: &PathAuxilliary,
        graph_aux: &GraphAuxilliary,
        k: usize,
        unimer: &Vec<usize>,
    ) -> Self {
        log::info!("counting abacus entries..");

        let mut infix_eq_tables: [HashMap<u64, InfixEqStorage>; SIZE_T] =
            [(); SIZE_T].map(|_| HashMap::default());

        let mut groups = Vec::new();
        for (path_id, group_id) in path_aux.get_path_order(&graph_aux.path_segments) {
            if groups.is_empty() || groups.last().unwrap() != group_id {
                groups.push(group_id.to_string());
            }
            AbacusByTotal::create_infix_eq_table(
                &item_table,
                &graph_aux,
                &mut infix_eq_tables,
                path_id,
                groups.len() as u32 - 1,
                k,
            );
        }
        ////DEBUG
        //for i in 0..SIZE_T {
        //    for (key, v) in &infix_eq_tables[i] {
        //        println!("{}: {:?} {} {} {}", bits2kmer(*key, k-1), v.edges, v.last_edge, v.last_group, v.sigma);
        //    }
        //}

        let m = (groups.len() + 1) * groups.len() / 2;
        let mut countable: Vec<CountSize> = vec![0; m];

        for i in 0..SIZE_T {
            for (_k_minus_one_mer, infix_storage) in &infix_eq_tables[i] {
                for edge_count in infix_storage.edges.iter() {
                    if *edge_count != 0 {
                        //println!("{:?} {} {} {} ", infix_storage.edges, infix_storage.last_edge, infix_storage.last_group, infix_storage.sigma);
                        let idx = ((infix_storage.sigma) * (infix_storage.sigma - 1) / 2
                            + edge_count
                            - 1) as usize;
                        countable[idx] += 1;
                        //if infix_storage.sigma == 1 {
                        //    println!("{}",bits2kmer(*k_minus_one_mer, k-1));
                        //}
                    }
                }
            }
        }
        //DEBUG
        //println!("{:?}", countable);

        for i in 1..unimer.len() {
            countable[((i + 1) * i / 2) - 1] += unimer[i] as u32;
            //countable[((i+1)*i/2) - 1] = unimer[i] as u32;
        }

        Self {
            count: CountType::Node,
            countable: countable,
            uncovered_bps: None,
            groups: groups,
        }
    }

    fn create_infix_eq_table(
        item_table: &ItemTable,
        _graph_aux: &GraphAuxilliary,
        infix_eq_tables: &mut [HashMap<u64, InfixEqStorage>; SIZE_T],
        path_id: ItemId,
        group_id: u32,
        k: usize,
    ) {
        let infix_eq_tables_ptr = Wrap(infix_eq_tables);

        (0..SIZE_T).into_par_iter().for_each(|i| {
            let start = item_table.id_prefsum[i][path_id as usize] as usize;
            let end = item_table.id_prefsum[i][path_id as usize + 1] as usize;
            for j in start..end {
                let k_plus_one_mer = item_table.items[i][j];
                let infix = get_infix(k_plus_one_mer, k);
                let first_nt = (k_plus_one_mer >> (2 * k)) as u64;
                let last_nt = k_plus_one_mer & 0b11;
                //println!("{}", bits2kmer(infix, k)); // Be sure that the first is an A
                //println!("{}", bits2kmer(infix, k-1));
                let combined_nt = ((first_nt << 2) | last_nt) as u8;
                unsafe {
                    (*infix_eq_tables_ptr.0)[i]
                        .entry(infix)
                        .and_modify(|infix_storage| {
                            if infix_storage.last_group == group_id
                                && infix_storage.last_edge != combined_nt
                                && infix_storage.last_edge != 255
                            {
                                //if infix_storage.last_group == group_id && infix_storage.last_edge != 255 {
                                infix_storage.edges[infix_storage.last_edge as usize] -= 1;
                                infix_storage.last_edge = 255;
                            } else if infix_storage.last_group != group_id {
                                infix_storage.last_edge = combined_nt;
                                infix_storage.edges[infix_storage.last_edge as usize] += 1;
                                infix_storage.last_group = group_id;
                                infix_storage.sigma += 1;
                            }
                        })
                        .or_insert_with(|| {
                            let mut infix_storage = InfixEqStorage::new();
                            infix_storage.last_edge = combined_nt;
                            infix_storage.edges[infix_storage.last_edge as usize] += 1;
                            infix_storage.last_group = group_id;
                            infix_storage.sigma = 1;
                            infix_storage
                        });
                }
            }
        });
    }

    fn coverage_from_items(
        countable: &mut Vec<CountSize>,
        last: &mut Vec<ItemId>,
        items: &Vec<ItemId>,
        path_to_items: &Vec<usize>,
        exclude_table: &Option<ActiveTable>,
        path_id: ItemId,
        group_id: ItemId,
    ) {
        //print!("{}:",path_id);
        for i in path_to_items[path_id as usize]..path_to_items[path_id as usize +1] {
            let sid = items[i] as usize;
            //print!("{},", sid);
            if last[sid] != group_id && (exclude_table.is_none() || !exclude_table.as_ref().unwrap().items[sid]) {
                countable[sid] += 1;
                last[sid] = group_id;
            }
        }

    }

    fn coverage(
        countable: &mut Vec<CountSize>,
        last: &mut Vec<ItemId>,
        item_table: &ItemTable,
        exclude_table: &Option<ActiveTable>,
        path_id: ItemId,
        group_id: ItemId,
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

    pub fn abaci_from_gfa(
        gfa_file: &str,
        count: CountType,
        graph_aux: &GraphAuxilliary,
        path_aux: &PathAuxilliary,
    ) -> Result<Vec<Self>, Error> {
        let mut abaci = Vec::new();
        if let CountType::All = count {
            for count_type in CountType::iter() {
                if count_type != CountType::All {
                    let mut data = bufreader_from_compressed_gfa(gfa_file);
                    let abacus = AbacusByTotal::from_gfa(&mut data, &path_aux, &graph_aux, count_type);
                    abaci.push(abacus);
                }
            }
        } else {
            let mut data = bufreader_from_compressed_gfa(gfa_file);
            let abacus = AbacusByTotal::from_gfa(&mut data, &path_aux, &graph_aux, count);
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

    pub fn construct_hist_bps(&self, graph_aux: &GraphAuxilliary) -> Vec<usize> {
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
                hist[*cov as usize] += graph_aux.node_lens[id] as usize;
            }
        }

        // subtract uncovered bps
        let uncovered_bps = self.uncovered_bps.as_ref().unwrap();
        for (id, uncov) in uncovered_bps.iter() {
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
    pub uncovered_bps: HashMap<ItemId, usize>,
    pub groups: Vec<String>,
    pub graph_aux: &'a GraphAuxilliary,
}

impl<'a> AbacusByGroup<'a> {
    pub fn from_gfa<R: std::io::Read>(
        data: &mut std::io::BufReader<R>,
        path_aux: &PathAuxilliary,
        graph_aux: &'a GraphAuxilliary,
        count: CountType,
        report_values: bool,
    ) -> Result<Self, Error> {
        log::info!("parsing path + walk sequences");
        let (_, _, item_table, exclude_table, subset_covered_bps, _paths_len) =
            parse_gfa_paths_walks(data, path_aux, graph_aux, &count);

        let mut path_order: Vec<(ItemId, GroupSize)> = Vec::new();
        let mut groups: Vec<String> = Vec::new();

        for (path_id, group_id) in path_aux.get_path_order(&graph_aux.path_segments) {
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
        path_order: &Vec<(ItemId, GroupSize)>,
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
        path_order: &Vec<(ItemId, GroupSize)>,
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
                                    self.uncovered_bps.get(&(i as ItemId)).unwrap_or(&0);
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

    pub fn to_tsv<W: Write>(&self, total: bool, out: &mut BufWriter<W>) -> Result<(), Error> {
        // create mapping from numerical node ids to original node identifiers
        log::info!("reporting coverage table");
        let dummy = Vec::new();
        let mut id2node: Vec<&Vec<u8>> = vec![&dummy; self.graph_aux.node_count + 1];
        for (node, id) in self.graph_aux.node2id.iter() {
            id2node[*id as usize] = node;
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
                            - *self.uncovered_bps.get(&(i as ItemId)).unwrap_or(&0)
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
                        0,
                        Orientation::default(),
                        0,
                        Orientation::default(),
                    );
                    let mut id2edge: Vec<&Edge> = vec![&dummy_edge; self.graph_aux.edge_count + 1];
                    for (edge, id) in edge2id.iter() {
                        id2edge[*id as usize] = edge;
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
                            std::str::from_utf8(id2node[edge.0 as usize]).unwrap(),
                            edge.3,
                            std::str::from_utf8(id2node[edge.2 as usize]).unwrap(),
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
) -> HashMap<ItemId, usize> {
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
            let sid = *sid;
            // ignore COMPETELY excluded nodes
            if exclude_table.is_none() || !exclude_table.as_ref().unwrap().items[sid as usize] {
                let l = graph_aux.node_len(sid) as usize;
                let covered = subset_map.total_coverage(
                    sid,
                    &exclude_table
                        .as_ref()
                        .map(|ex| ex.get_active_intervals(sid, l)),
                );
                if covered > l {
                    log::error!("oops, total coverage {} is larger than node length {} for node {}, intervals: {:?}", covered, l, sid, subset_map.get(sid).unwrap());
                } else {
                    // report uncovered bps
                    res.insert(sid, l - covered);
                }
            }
        }
    }
    res
}


#[cfg(test)]
mod tests {
    use super::*;

    fn setup_test_data() -> (GraphAuxilliary, Params, String) {
        let test_gfa_file = "test/cdbg.gfa";
        let graph_aux = GraphAuxilliary::from_gfa(test_gfa_file, false);
        let mut params = Params::default_histgrowth();
        if let Params::Histgrowth {
            ref mut gfa_file,
            ..
        } = params {
            *gfa_file=test_gfa_file.to_string()
        }

        (graph_aux, params, test_gfa_file.to_string())
    }

    #[test]
    fn test_abacus_by_total_from_gfa() {
        let (graph_aux, params, test_gfa_file) = setup_test_data();
        let path_aux = PathAuxilliary::from_params(&params, &graph_aux).unwrap();
        let test_abacus_by_total = AbacusByTotal {
            count: CountType::Node,
            countable: vec![CountSize::MAX, 6,4,4,2,1],
            uncovered_bps: Some(HashMap::default()),
            groups:  vec!["a#1#h1".to_string(), "b#1#h1".to_string(), 
                          "c#1#h1".to_string(), "c#1#h2".to_string(), 
                          "c#2#h1".to_string(), "d#1#h1".to_string()
            ]
        };

        let mut data = bufreader_from_compressed_gfa(test_gfa_file.as_str());
        let abacus_by_total = AbacusByTotal::from_gfa(&mut data, &path_aux, &graph_aux, CountType::Node);
        assert_eq!(abacus_by_total.count, test_abacus_by_total.count, "Expected CountType to match Node");
        assert_eq!(abacus_by_total.countable, test_abacus_by_total.countable, "Expected same countable");
        assert_eq!(abacus_by_total.uncovered_bps, test_abacus_by_total.uncovered_bps, "Expected empty uncovered bps");
        assert_eq!(abacus_by_total.groups, test_abacus_by_total.groups, "Expected same groups");
    }
}
