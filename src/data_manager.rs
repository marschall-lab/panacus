use std::{
    collections::{HashMap, HashSet}, fmt, fs, io::{BufRead, BufReader, Error, ErrorKind, Read}, iter::FromIterator, str::{self, FromStr}, usize
};

use strum::IntoEnumIterator;

use crate::{
    abacus::{quantify_uncovered_bps, AbacusAuxilliary, AbacusByGroup, AbacusByTotal}, analysis::InputRequirement, graph::{Edge, GraphAuxilliary, ItemId, PathSegment}, io::{bufreader_from_compressed_gfa, parse_bed_to_path_segments, parse_groups, parse_path_identifier, parse_path_seq_to_item_vec, parse_path_seq_update_tables, parse_walk_identifier, parse_walk_seq_to_item_vec, parse_walk_seq_update_tables, update_tables, update_tables_edgecount}, util::{intersects, is_contained, ActiveTable, CountType, IntervalContainer, ItemIdSize, ItemTable, SIZE_T}
};

#[derive(Debug)]
pub struct DataManager<'a> {
    // GraphAuxilliary
    graph_aux: GraphAuxilliary,

    // AbabcusAuxilliary
    groups: Option<HashMap<PathSegment, String>>,
    include_coords: Option<Vec<PathSegment>>,
    exclude_coords: Option<Vec<PathSegment>>,
    order: Option<Vec<PathSegment>>,

    total_abaci: Option<HashMap<CountType, AbacusByTotal>>,
    group_abaci: Option<HashMap<CountType, AbacusByGroup<'a>>>,

    path_lens: HashMap<PathSegment, (u32, u32)>,
    gfa_file: String,
    input_requirements: HashSet<InputRequirement>,
    count_type: CountType,
}

impl<'a> DataManager<'a> {
    pub fn from_gfa(gfa_file: &str, input_requirements: HashSet<InputRequirement>) -> Self {
        let graph_aux = GraphAuxilliary::from_gfa(gfa_file, CountType::Node);
        DataManager {
            graph_aux,
            groups: None,
            include_coords: None,
            exclude_coords: None,
            order: None,
            total_abaci: None,
            group_abaci: None,
            path_lens: HashMap::new(),
            gfa_file: gfa_file.to_owned(),
            input_requirements,
            count_type: CountType::Node,
        }
    }

    pub fn with_group(mut self, file_name: &str) -> Result<Self, Error>  {
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
        self.graph_aux.path_segments.iter().for_each(|x| {
            let path = x.clear_coords();
            path_to_group.entry(path).or_insert_with(|| x.id());
        });
        self.groups = Some(path_to_group);
        Ok(self)
    }

    pub fn with_haplo_group(mut self) -> Self {
        self.groups = Some(
            self.graph_aux.path_segments
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
                .collect(),
        );
        self
    }

    pub fn with_sample_group(mut self) -> Self {
        self.groups = Some(
            self.graph_aux.path_segments
                .iter()
                .map(|x| (x.clear_coords(), x.sample.clone()))
                .collect(),
        );
        self
    }

    pub fn with_default_group(mut self) -> Self {
        log::info!("no explicit grouping instruction given, group paths by their IDs (sample ID+haplotype ID+seq ID)");
        self.groups = Some(self.graph_aux
            .path_segments
            .iter()
            .map(|x| (x.clear_coords(), x.id()))
            .collect());
        self
    }

    pub fn include_coords(mut self, file_name: &str) -> Result<Self, Error> {
        if self.groups.is_none() {
            let msg = format!(
                "Cannot include coords {} before setting groups",
                file_name
            );
            log::error!("{}", &msg);
            return Err(Error::new(ErrorKind::Unsupported, msg));
        }
        self.include_coords = AbacusAuxilliary::complement_with_group_assignments(
            AbacusAuxilliary::load_coord_list(file_name)?,
            self.groups.as_ref().unwrap())?;
        Ok(self)
    }

    pub fn exclude_coords(mut self, file_name: &str) -> Result<Self, Error> {
        if self.groups.is_none() {
            let msg = format!(
                "Cannot exclude coords {} before setting groups",
                file_name
            );
            log::error!("{}", &msg);
            return Err(Error::new(ErrorKind::Unsupported, msg));
        }
        self.exclude_coords = AbacusAuxilliary::complement_with_group_assignments(
            AbacusAuxilliary::load_coord_list(file_name)?,
            self.groups.as_ref().unwrap())?;
        Ok(self)
    }

    pub fn with_order(mut self, file_name: &str) -> Result<Self, Error> {
        let maybe_order = AbacusAuxilliary::complement_with_group_assignments(
            AbacusAuxilliary::load_coord_list(file_name)?,
            self.groups.as_ref().unwrap(),
        )?;
        if let Some(o) = &maybe_order {
            // if order is given, check that it comprises all included coords
            let all_included_paths: Vec<PathSegment> = match &self.include_coords {
                None => {
                    let exclude: HashSet<&PathSegment> = match &self.exclude_coords {
                        Some(e) => e.iter().collect(),
                        None => HashSet::new(),
                    };
                    self.graph_aux.path_segments
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
                Some(include) => include.iter().map(|x| x.clear_coords()).collect(),
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
            let mut cur: &str = self.groups.as_ref().unwrap().get(&o[0]).unwrap();
            for p in o.iter() {
                let g: &str = self.groups.as_ref().unwrap().get(p).unwrap();
                if cur != g && !visited.insert(g) {
                    let msg = format!("order of paths contains fragmented groups: path {} belongs to group that is interspersed by one or more other groups", p);
                    log::error!("{}", &msg);
                    return Err(Error::new(ErrorKind::InvalidData, msg));
                }
                cur = g;
            }
            self.order = maybe_order;
            Ok(self)
        } else {
            let msg = format!("File {} contains no order data", file_name);
            log::error!("{}", &msg);
            Err(Error::new(ErrorKind::InvalidData, msg))
        }
    }

    pub fn finish(mut self) -> Self {
        let mut abaci = HashMap::new();
        if let CountType::All = self.count_type {
            for count_type in CountType::iter() {
                if let CountType::All = count_type {
                } else {
                    let mut data = bufreader_from_compressed_gfa(&self.gfa_file);
                    let abacus =
                         Self::abacus_from_gfa(&mut data, abacus_aux, graph_aux, count_type);
                    abaci.insert(count_type, abacus);
                }
            }
        } else {
            let mut data = bufreader_from_compressed_gfa(&self.gfa_file);
            // let abacus = Self::abacus_from_gfa(&mut data, abacus_aux, graph_aux, count);
            // abaci.insert(self.count_type, abacus);
        }
        self.total_abaci = Some(abaci);
        self
    }

    fn abacus_from_gfa<R: std::io::Read>(
        data: &mut BufReader<R>,
        count: CountType,
    ) -> Self {
        let (item_table, exclude_table, subset_covered_bps, _paths_len) =
            self.parse_gfa_paths_walks(data, &count);
        self.item_table_to_abacus(
            abacus_aux,
            graph_aux,
            count,
            item_table,
            exclude_table,
            subset_covered_bps,
        )
    }

    fn item_table_to_abacus(
        &self
        count: CountType,
        item_table: ItemTable,
        exclude_table: Option<ActiveTable>,
        subset_covered_bps: Option<IntervalContainer>,
    ) -> AbacusByTotal {
        log::info!("counting abacus entries..");
        // first element in countable is "zero" element. It is ignored in counting
        let mut countable: Vec<CountSize> = vec![0; graph_aux.number_of_items(&count) + 1];
        // countable with ID "0" is special and should not be considered in coverage histogram
        countable[0] = CountSize::MAX;
        let mut last: Vec<ItemIdSize> =
            vec![ItemIdSize::MAX; graph_aux.number_of_items(&count) + 1];

        let mut groups = Vec::new();
        for (path_id, group_id) in abacus_aux.get_path_order(&graph_aux.path_segments) {
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

        log::info!(
            "abacus has {} path groups and {} countables",
            groups.len(),
            countable.len() - 1
        );

        AbacusByTotal {
            count,
            countable,
            uncovered_bps: Some(quantify_uncovered_bps(
                &exclude_table,
                &subset_covered_bps,
                &self.graph_aux,
            )),
            groups,
        }
    }

    fn parse_gfa_paths_walks<R: Read>(
        &mut self,
        data: &mut BufReader<R>,
        count: &CountType,
    ) -> (
    ItemTable,
    Option<ActiveTable>,
    Option<IntervalContainer>,
    HashMap<PathSegment, (u32, u32)>,
    ) {
        log::info!("parsing path + walk sequences");
        let mut item_table = ItemTable::new(self.graph_aux.path_segments.len());
        let (mut subset_covered_bps, mut exclude_table, include_map, exclude_map) =
            self.load_optional_subsetting(count);

        let mut num_path = 0;
        let complete: Vec<(usize, usize)> = vec![(0, usize::MAX)];
        let mut paths_len: HashMap<PathSegment, (u32, u32)> = HashMap::new();

        let mut buf = vec![];
        while data.read_until(b'\n', &mut buf).unwrap_or(0) > 0 {
            if buf[0] == b'P' || buf[0] == b'W' {
                let (path_seg, buf_path_seg) = match buf[0] {
                    b'P' => parse_path_identifier(&buf),
                    b'W' => parse_walk_identifier(&buf),
                    _ => unreachable!(),
                };

                log::debug!("processing path {}", &path_seg);

                let include_coords = if self.include_coords.is_none() {
                    &complete[..]
                } else {
                    match include_map.get(&path_seg.id()) {
                        None => &[],
                        Some(coords) => {
                            log::debug!(
                                "found include coords {:?} for path segment {}",
                                &coords[..],
                                &path_seg.id()
                            );
                            &coords[..]
                        }
                    }
                };
                let exclude_coords = if self.exclude_coords.is_none() {
                    &[]
                } else {
                    match exclude_map.get(&path_seg.id()) {
                        None => &[],
                        Some(coords) => {
                            log::debug!(
                                "found exclude coords {:?} for path segment {}",
                                &coords[..],
                                &path_seg.id()
                            );
                            &coords[..]
                        }
                    }
                };

                let (start, end) = path_seg.coords().unwrap_or((0, usize::MAX));

                // do not process the path sequence if path is neither part of subset nor exclude
                if self.include_coords.is_some()
                    && !intersects(include_coords, &(start, end))
                        && !intersects(exclude_coords, &(start, end))
                {
                    log::debug!("path {} does not intersect with subset coordinates {:?} nor with exclude coordinates {:?} and therefore is skipped from processing", 
                        &path_seg, &include_coords, &exclude_coords);

                    // update prefix sum
                    for i in 0..SIZE_T {
                        item_table.id_prefsum[i][num_path + 1] += item_table.id_prefsum[i][num_path];
                    }

                    num_path += 1;
                    buf.clear();
                    continue;
                }

                if count != &CountType::Edge
                    && (self.include_coords.is_none()
                        || is_contained(include_coords, &(start, end)))
                        && (self.exclude_coords.is_none()
                            || is_contained(exclude_coords, &(start, end)))
                {
                    log::debug!("path {} is fully contained within subset coordinates {:?} and is eligible for full parallel processing", path_seg, include_coords);
                    let ex = if exclude_coords.is_empty() {
                        None
                    } else {
                        exclude_table.as_mut()
                    };

                    let (num_added_nodes, bp_len) = match buf[0] {
                        b'P' => parse_path_seq_update_tables(
                            buf_path_seg,
                            &self.graph_aux,
                            &mut item_table,
                            ex,
                            num_path,
                        ),
                        b'W' => parse_walk_seq_update_tables(
                            buf_path_seg,
                            &self.graph_aux,
                            &mut item_table,
                            ex,
                            num_path,
                        ),
                        _ => unreachable!(),
                    };
                    paths_len.insert(path_seg, (num_added_nodes, bp_len));
                } else {
                    let sids = match buf[0] {
                        b'P' => parse_path_seq_to_item_vec(buf_path_seg, &self.graph_aux),
                        b'W' => parse_walk_seq_to_item_vec(buf_path_seg, &self.graph_aux),
                        _ => unreachable!(),
                    };

                    paths_len.insert(path_seg, (sids.len() as u32, 0));

                    match count {
                        CountType::Node | CountType::Bp => update_tables(
                            &mut item_table,
                            &mut subset_covered_bps.as_mut(),
                            &mut exclude_table.as_mut(),
                            num_path,
                            &self.graph_aux,
                            sids,
                            include_coords,
                            exclude_coords,
                            start,
                        ),
                        CountType::Edge => update_tables_edgecount(
                            &mut item_table,
                            &mut exclude_table.as_mut(),
                            num_path,
                            &self.graph_aux,
                            sids,
                            include_coords,
                            exclude_coords,
                            start,
                        ),
                        CountType::All => unreachable!("inadmissable count type"),
                    };
                }
                num_path += 1;
            }
            buf.clear();
        }
        (item_table, exclude_table, subset_covered_bps, paths_len)
    }

    fn load_optional_subsetting(
        &self,
        count: &CountType,
    ) -> (
        Option<IntervalContainer>,
        Option<ActiveTable>,
        HashMap<String, Vec<(usize, usize)>>,
        HashMap<String, Vec<(usize, usize)>>,
    ) {
        // *only relevant for bps count in combination with subset option*
        // this table stores the number of bps of nodes that are *partially* uncovered by subset
        // coodinates
        let subset_covered_bps: Option<IntervalContainer> =
            if count == &CountType::Bp && self.include_coords.is_some() {
                Some(IntervalContainer::new())
            } else {
                None
            };

        // this table stores information about excluded nodes *if* the exclude setting is used
        let exclude_table = self.exclude_coords.as_ref().map(|_| {
            ActiveTable::new(
                self.graph_aux.number_of_items(count) + 1,
                count == &CountType::Bp,
            )
        });

        // build "include" lookup table
        let include_map = match &self.include_coords {
            None => HashMap::default(),
            Some(coords) => AbacusAuxilliary::build_subpath_map(coords),
        };

        // build "exclude" lookup table
        let exclude_map = match &self.exclude_coords {
            None => HashMap::default(),
            Some(coords) => AbacusAuxilliary::build_subpath_map(coords),
        };

        (subset_covered_bps, exclude_table, include_map, exclude_map)
    }
}
