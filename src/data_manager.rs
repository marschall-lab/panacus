use std::{
    collections::{HashMap, HashSet}, fmt, fs, io::{BufRead, BufReader, Error, ErrorKind, Read}, iter::FromIterator, str::{self, FromStr}, usize
};

use strum::IntoEnumIterator;

use crate::{
    abacus::{AbacusByGroup, AbacusByTotal}, analysis::InputRequirement, graph::{Edge, ItemId, PathSegment}, io::{bufreader_from_compressed_gfa, parse_bed_to_path_segments, parse_groups, parse_path_identifier, parse_path_seq_to_item_vec, parse_path_seq_update_tables, parse_walk_identifier, parse_walk_seq_to_item_vec, parse_walk_seq_update_tables, update_tables, update_tables_edgecount}, util::{intersects, is_contained, ActiveTable, CountType, IntervalContainer, ItemIdSize, ItemTable, SIZE_T}
};

#[derive(Debug)]
pub struct DataManager<'a> {
    // GraphAuxilliary
    node2id: Option<HashMap<Vec<u8>, ItemId>>,
    node_lens: Option<Vec<u32>>,
    edge2id: Option<HashMap<Edge, ItemId>>,
    path_segments: Option<Vec<PathSegment>>,
    node_count: Option<usize>,
    edge_count: Option<usize>,
    degree: Option<Vec<u32>>,

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
        let (node2id, path_segments, node_lens, node_count) = if input_requirements
            .contains(&InputRequirement::Ga)
            || input_requirements.contains(&InputRequirement::GaEdge)
        {
            Self::parse_nodes_gfa(gfa_file)
        } else {
            (None, None, None, None)
        };

        let (edge2id, edge_count, degree) =
            if input_requirements.contains(&InputRequirement::GaEdge) {
                Self::parse_edge_gfa(gfa_file, &node2id.as_ref().unwrap())
            } else {
                (None, None, None)
            };
        DataManager {
            node2id,
            node_lens,
            edge2id,
            path_segments,
            node_count,
            edge_count,
            degree,
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
        self.path_segments.as_ref().unwrap().iter().for_each(|x| {
            let path = x.clear_coords();
            path_to_group.entry(path).or_insert_with(|| x.id());
        });
        self.groups = Some(path_to_group);
        Ok(self)
    }

    pub fn with_haplo_group(mut self) -> Self {
        self.groups = Some(
            self.path_segments
                .as_ref()
                .unwrap()
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
            self.path_segments
                .as_ref()
                .unwrap()
                .iter()
                .map(|x| (x.clear_coords(), x.sample.clone()))
                .collect(),
        );
        self
    }

    pub fn with_default_group(mut self) -> Self {
        log::info!("no explicit grouping instruction given, group paths by their IDs (sample ID+haplotype ID+seq ID)");
        self.groups = Some(self
            .path_segments.as_ref().unwrap()
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
        self.include_coords = Self::complement_with_group_assignments(
            Self::load_coord_list(file_name)?,
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
        self.exclude_coords = Self::complement_with_group_assignments(
            Self::load_coord_list(file_name)?,
            self.groups.as_ref().unwrap())?;
        Ok(self)
    }

    pub fn with_order(mut self, file_name: &str) -> Result<Self, Error> {
        let maybe_order = Self::complement_with_group_assignments(
            Self::load_coord_list(file_name)?,
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
                    self.path_segments.as_ref().unwrap()
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
                    // let abacus =
                    //     Self::abacus_from_gfa(&mut data, abacus_aux, graph_aux, count_type);
                    // abaci.insert(count_type, abacus);
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
        let mut item_table = ItemTable::new(self.path_segments.unwrap().len());
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
                            graph_aux,
                            &mut item_table,
                            ex,
                            num_path,
                        ),
                        b'W' => parse_walk_seq_update_tables(
                            buf_path_seg,
                            graph_aux,
                            &mut item_table,
                            ex,
                            num_path,
                        ),
                        _ => unreachable!(),
                    };
                    paths_len.insert(path_seg, (num_added_nodes, bp_len));
                } else {
                    let sids = match buf[0] {
                        b'P' => parse_path_seq_to_item_vec(buf_path_seg, graph_aux),
                        b'W' => parse_walk_seq_to_item_vec(buf_path_seg, graph_aux),
                        _ => unreachable!(),
                    };

                    paths_len.insert(path_seg, (sids.len() as u32, 0));

                    match count {
                        CountType::Node | CountType::Bp => update_tables(
                            &mut item_table,
                            &mut subset_covered_bps.as_mut(),
                            &mut exclude_table.as_mut(),
                            num_path,
                            graph_aux,
                            sids,
                            include_coords,
                            exclude_coords,
                            start,
                        ),
                        CountType::Edge => update_tables_edgecount(
                            &mut item_table,
                            &mut exclude_table.as_mut(),
                            num_path,
                            graph_aux,
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
                self.number_of_items(count).unwrap() + 1,
                count == &CountType::Bp,
            )
        });

        // build "include" lookup table
        let include_map = match &self.include_coords {
            None => HashMap::default(),
            Some(coords) => Self::build_subpath_map(coords),
        };

        // build "exclude" lookup table
        let exclude_map = match &self.exclude_coords {
            None => HashMap::default(),
            Some(coords) => Self::build_subpath_map(coords),
        };

        (subset_covered_bps, exclude_table, include_map, exclude_map)
    }

    fn number_of_items(&self, c: &CountType) -> Option<usize> {
        match c {
            &CountType::Node | &CountType::Bp => self.node_count,
            &CountType::Edge => self.edge_count,
            &CountType::All => unreachable!("inadmissible count type"),
        }
    }

    fn build_subpath_map(
        path_segments: &[PathSegment],
    ) -> HashMap<String, Vec<(usize, usize)>> {
        // intervals are 0-based, and [start, end), see https://en.wikipedia.org/wiki/BED_(file_format)
        let mut res: HashMap<String, HashSet<(usize, usize)>> = HashMap::default();

        path_segments.iter().for_each(|x| {
            res.entry(x.id()).or_default().insert(match x.coords() {
                None => (0, usize::MAX),
                Some((i, j)) => (i, j),
            });
        });

        HashMap::from_iter(res.into_iter().map(|(pid, coords)| {
            let mut v: Vec<(usize, usize)> = coords.into_iter().collect();
            v.sort();
            let mut i = 1;
            // remove overlaps
            while i < v.len() {
                if v[i - 1].1 >= v[i].0 {
                    let x = v.remove(i);
                    v[i - 1].1 = x.1;
                } else {
                    i += 1
                }
            }
            (pid, v)
        }))
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
            group2paths.entry(g.clone()).or_default().push(p.clone())
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
            let use_block_info = true;
            let coords = parse_bed_to_path_segments(&mut data, use_block_info);
            log::debug!("loaded {} coordinates", coords.len());
            Some(coords)
        })
    }

    fn parse_edge_gfa(
        gfa_file: &str,
        node2id: &HashMap<Vec<u8>, ItemId>,
    ) -> (
        Option<HashMap<Edge, ItemId>>,
        Option<usize>,
        Option<Vec<u32>>,
    ) {
        let mut edge2id = HashMap::default();
        let mut degree: Vec<u32> = vec![0; node2id.len() + 1];
        let mut edge_id: ItemIdSize = 1;

        let mut buf = vec![];
        let mut data = bufreader_from_compressed_gfa(gfa_file);
        while data.read_until(b'\n', &mut buf).unwrap_or(0) > 0 {
            if buf[0] == b'L' {
                let edge = Edge::from_link(&buf[..], node2id, true);
                if let std::collections::hash_map::Entry::Vacant(e) = edge2id.entry(edge) {
                    degree[edge.0 .0 as usize] += 1;
                    //if e.0.0 != e.2.0 {
                    degree[edge.2 .0 as usize] += 1;
                    //}
                    e.insert(ItemId(edge_id));
                    edge_id += 1;
                } else {
                    log::warn!("edge {} is duplicated in GFA", &edge);
                }
            }
            buf.clear();
        }
        let edge_count = edge2id.len();
        log::info!("found: {} edges", edge_count);

        (Some(edge2id), Some(edge_count), Some(degree))
    }

    fn parse_nodes_gfa(
        gfa_file: &str,
    ) -> (
        Option<HashMap<Vec<u8>, ItemId>>,
        Option<Vec<PathSegment>>,
        Option<Vec<u32>>,
        Option<usize>,
    ) {
        let mut node2id: HashMap<Vec<u8>, ItemId> = HashMap::default();
        let mut path_segments: Vec<PathSegment> = Vec::new();
        let mut node_lens: Vec<u32> = Vec::new();

        log::info!("constructing indexes for node/edge IDs, node lengths, and P/W lines..");
        node_lens.push(u32::MIN); // add empty element to node_lens to make it in sync with node_id
        let mut node_id = 1; // important: id must be > 0, otherwise counting procedure will produce errors

        let mut buf = vec![];
        let mut data = bufreader_from_compressed_gfa(gfa_file);
        while data.read_until(b'\n', &mut buf).unwrap_or(0) > 0 {
            if buf[0] == b'S' {
                let mut iter = buf[2..].iter();
                let offset = iter.position(|&x| x == b'\t').unwrap();
                if node2id
                    .insert(buf[2..offset + 2].to_vec(), ItemId(node_id))
                    .is_some()
                {
                    panic!(
                        "Segment with ID {} occurs multiple times in GFA",
                        str::from_utf8(&buf[2..offset + 2]).unwrap()
                    )
                }
                // let start_sequence = offset + 3;
                let offset = iter
                    .position(|&x| x == b'\t' || x == b'\n' || x == b'\r')
                    .unwrap();
                node_lens.push(offset as u32);
                node_id += 1;
            } else if buf[0] == b'P' {
                path_segments.push(Self::parse_path_segment(&buf));
            } else if buf[0] == b'W' {
                path_segments.push(Self::parse_walk_segment(&buf));
            }
            buf.clear();
        }

        log::info!(
            "found: {} paths/walks, {} nodes",
            path_segments.len(),
            node2id.len()
        );
        if path_segments.is_empty() {
            log::warn!("graph does not contain any annotated paths (P/W lines)");
        }

        let node_count = node2id.len();

        (
            Some(node2id),
            Some(path_segments),
            Some(node_lens),
            Some(node_count),
        )
    }

    pub fn parse_path_segment(data: &[u8]) -> PathSegment {
        let mut iter = data.iter();
        let start = iter.position(|&x| x == b'\t').unwrap() + 1;
        let offset = iter.position(|&x| x == b'\t').unwrap();
        let path_name = str::from_utf8(&data[start..start + offset]).unwrap();
        PathSegment::from_str(path_name)
    }

    pub fn parse_walk_segment(data: &[u8]) -> PathSegment {
        let mut six_col: Vec<&str> = Vec::with_capacity(6);

        let mut it = data.iter();
        let mut i = 0;
        for _ in 0..6 {
            let j = it.position(|x| x == &b'\t').unwrap();
            six_col.push(str::from_utf8(&data[i..i + j]).unwrap());
            i += j + 1;
        }

        let seq_start = match six_col[4] {
            "*" => None,
            a => Some(usize::from_str(a).unwrap()),
        };

        let seq_end = match six_col[5] {
            "*" => None,
            a => Some(usize::from_str(a).unwrap()),
        };

        PathSegment::new(
            six_col[1].to_string(),
            six_col[2].to_string(),
            six_col[3].to_string(),
            seq_start,
            seq_end,
        )
    }
}
