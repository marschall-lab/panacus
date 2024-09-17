/* standard use */
use std::fs;
use std::io::{BufRead, BufReader, Read};
use std::io::{Error, ErrorKind};
use std::iter::FromIterator;
use std::str::{self, FromStr};
use std::fmt;
use regex::Regex;
use once_cell::sync::Lazy;
//use std::sync::{Arc, Mutex};

/* external crate*/
use std::collections::{HashMap, HashSet};

/* private use */
use crate::util::*;
use crate::cli::Params;
use crate::graph::*;
use crate::path_parser::*;

static PATHID_PANSN: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^([^#]+)(#[^#]+)?(#[^#]+)?$").unwrap());
static PATHID_COORDS: Lazy<Regex> = Lazy::new(|| Regex::new(r"^(.+):([0-9]+)-([0-9]+)$").unwrap());

#[derive(Debug, Clone, PartialEq, PartialOrd, Hash, Eq, Ord)]
pub struct PathSegment {
    pub sample: String,
    pub haplotype: Option<String>,
    pub seqid: Option<String>,
    pub start: Option<usize>,
    pub end: Option<usize>,
}

impl PathSegment {
    pub fn new(
        sample: String,
        haplotype: String,
        seqid: String,
        start: Option<usize>,
        end: Option<usize>,
    ) -> Self {
        Self {
            sample: sample,
            haplotype: Some(haplotype),
            seqid: Some(seqid),
            start: start,
            end: end,
        }
    }

    pub fn from_str(s: &str) -> Self {
        let mut res = PathSegment {
            sample: s.to_string(),
            haplotype: None,
            seqid: None,
            start: None,
            end: None,
        };

        if let Some(c) = PATHID_PANSN.captures(s) {
            let segments: Vec<&str> = c.iter().filter_map(|x| x.map(|y| y.as_str())).collect();
            // first capture group is the string itself
            //log::debug!(
            //    "path id {} can be decomposed into capture groups {:?}",
            //    s,
            //    segments
            //);
            match segments.len() {
                4 => {
                    res.sample = segments[1].to_string();
                    res.haplotype = Some(segments[2][1..].to_string());
                    match PATHID_COORDS.captures(&segments[3][1..]) {
                        None => {
                            res.seqid = Some(segments[3][1..].to_string());
                        }
                        Some(cc) => {
                            log::debug!("path has coodinates {:?}", cc);
                            res.seqid = Some(cc.get(1).unwrap().as_str().to_string());
                            res.start = usize::from_str(cc.get(2).unwrap().as_str()).ok();
                            res.end = usize::from_str(cc.get(3).unwrap().as_str()).ok();
                        }
                    }
                }
                3 => {
                    res.sample = segments[1].to_string();
                    match PATHID_COORDS.captures(&segments[2][1..]) {
                        None => {
                            res.haplotype = Some(segments[2][1..].to_string());
                        }
                        Some(cc) => {
                            log::debug!("path has coodinates {:?}", cc);
                            res.haplotype = Some(cc.get(1).unwrap().as_str().to_string());
                            res.start = usize::from_str(cc.get(2).unwrap().as_str()).ok();
                            res.end = usize::from_str(cc.get(3).unwrap().as_str()).ok();
                        }
                    }
                }
                2 => {
                    if let Some(cc) = PATHID_COORDS.captures(segments[1]) {
                        log::debug!("path has coodinates {:?}", cc);
                        res.sample = cc.get(1).unwrap().as_str().to_string();
                        res.start = usize::from_str(cc.get(2).unwrap().as_str()).ok();
                        res.end = usize::from_str(cc.get(3).unwrap().as_str()).ok();
                    }
                }
                _ => (),
            }
        }
        res
    }

    pub fn from_str_start_end(s: &str, start: usize, end: usize) -> Self {
        let mut segment = Self::from_str(s);
        segment.start = Some(start);
        segment.end = Some(end);
        segment
    }

    pub fn id(&self) -> String {
        if self.haplotype.is_some() {
            format!(
                "{}#{}{}",
                self.sample,
                self.haplotype.as_ref().unwrap(),
                if self.seqid.is_some() {
                    "#".to_owned() + self.seqid.as_ref().unwrap().as_str()
                } else {
                    "".to_string()
                }
            )
        } else if self.seqid.is_some() {
            format!(
                "{}#*#{}",
                self.sample,
                self.seqid.as_ref().unwrap().as_str()
            )
        } else {
            self.sample.clone()
        }
    }

    pub fn clear_coords(&self) -> Self {
        Self {
            sample: self.sample.clone(),
            haplotype: self.haplotype.clone(),
            seqid: self.seqid.clone(),
            start: None,
            end: None,
        }
    }

    pub fn coords(&self) -> Option<(usize, usize)> {
        if self.start.is_some() && self.end.is_some() {
            Some((self.start.unwrap(), self.end.unwrap()))
        } else {
            None
        }
    }

    //#[allow(dead_code)]
    //pub fn covers(&self, other: &PathSegment) -> bool {
    //    self.sample == other.sample
    //        && (self.haplotype == other.haplotype
    //            || (self.haplotype.is_none()
    //                && self.seqid.is_none()
    //                && self.start.is_none()
    //                && self.end.is_none()))
    //        && (self.seqid == other.seqid
    //            || (self.seqid.is_none() && self.start.is_none() && self.end.is_none()))
    //        && (self.start == other.start
    //            || self.start.is_none()
    //            || (other.start.is_some() && self.start.unwrap() <= other.start.unwrap()))
    //        && (self.end == other.end
    //            || self.end.is_none()
    //            || (other.end.is_some() && self.end.unwrap() >= other.end.unwrap()))
    //}
}

impl fmt::Display for PathSegment {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        if let Some((start, end)) = self.coords() {
            write!(formatter, "{}:{}-{}", self.id(), start, end)?;
        } else {
            write!(formatter, "{}", self.id())?;
        }
        Ok(())
    }
}

pub struct PathAuxilliary {
    pub groups: HashMap<PathSegment, String>,
    pub include_coords: Option<Vec<PathSegment>>,
    pub exclude_coords: Option<Vec<PathSegment>>,
    pub order: Option<Vec<PathSegment>>,
}

impl PathAuxilliary {
    pub fn from_params(params: &Params, graph_aux: &GraphAuxilliary) -> Result<Self, Error> {
        if let Params::Histgrowth {
            positive_list,
            negative_list,
            groupby,
            groupby_sample,
            groupby_haplotype,
            ..
        }
        | Params::Hist {
            positive_list,
            negative_list,
            groupby,
            groupby_sample,
            groupby_haplotype,
            ..
        }
        | Params::Stats {
            positive_list,
            negative_list,
            groupby,
            groupby_sample,
            groupby_haplotype,
            ..
        }
        | Params::Subset {
            positive_list,
            negative_list,
            groupby,
            groupby_sample,
            groupby_haplotype,
            ..
        }
        | Params::OrderedHistgrowth {
            positive_list,
            negative_list,
            groupby,
            groupby_sample,
            groupby_haplotype,
            ..
        }
        | Params::Table {
            positive_list,
            negative_list,
            groupby,
            groupby_sample,
            groupby_haplotype,
            ..
        }
        //| Params::Cdbg {
        //    positive_list,
        //    negative_list,
        //    groupby,
        //    groupby_sample,
        //    groupby_haplotype,
        //    ..
        //} 
        = params {
            let groups = Self::load_groups(
                groupby,
                *groupby_haplotype,
                *groupby_sample,
                graph_aux,
            )?;
            let include_coords = Self::complement_with_group_assignments(
                Self::load_coord_list(positive_list),
                &groups,
            )?;
            let exclude_coords = Self::complement_with_group_assignments(
                Self::load_coord_list(negative_list),
                &groups,
            )?;
            let some_order = if let Params::OrderedHistgrowth { order, .. } = params {
                Self::complement_with_group_assignments(
                    Self::load_coord_list(order),
                    &groups,
                )?
            } else {
                None
            };

            if let Some(order) = &some_order {
                Self::check_order_comprises_all_included_coords(&order, &graph_aux, &include_coords, &exclude_coords);
                Self::check_order_groups_are_not_scrambled(&order, &groups)?;
            }

            Ok(PathAuxilliary {
                groups: groups,
                include_coords: include_coords,
                exclude_coords: exclude_coords,
                order: some_order,
            })
        } else {
            Err(Error::new(
                ErrorKind::InvalidData,
                "cannot produce PathAuxilliary from other Param items",
            ))
        }
    }

    fn complement_with_group_assignments(
        coords: Option<Vec<PathSegment>>,
        groups: &HashMap<PathSegment, String>,
    ) -> Result<Option<Vec<PathSegment>>, Error> {
        // We allow coords to be defined via groups
        // This code adds all the paths from a group in specified in coords
        let mut group_to_path: HashMap<String, Vec<PathSegment>> = HashMap::default();
        for (p, g) in groups.iter() {
            group_to_path
                .entry(g.clone())
                .or_insert(Vec::new())
                .push(p.clone())
        }
        let path_to_group: HashMap<PathSegment, String> = groups
            .iter()
            .map(|(ps, g)| (ps.clear_coords(), g.clone()))
            .collect();

        if let Some(v) = coords {
            let mut complemented_path_segments = Vec::new();
            for path_segment in v.iter() {
                if path_to_group.contains_key(&path_segment.clear_coords()) {
                    complemented_path_segments.push(path_segment.clone());
                } else if group_to_path.contains_key(&path_segment.id()) {
                    // checks that group-based coordinates don't have start/stop information
                    if path_segment.coords().is_some() {
                        let msg = format!("invalid coordinate \"{}\": group identifiers are not allowed to have start/stop information!", &path_segment);
                        log::error!("{}", &msg);
                        return Err(Error::new( ErrorKind::InvalidData, msg))
                    } else {
                        // complements coords with path segments from group assignments
                        let mut paths = group_to_path.get(&path_segment.id()).unwrap().clone();
                        log::debug!("complementing coordinate list with {} paths associted with group {}", paths.len(), path_segment.id());
                        complemented_path_segments.append(&mut paths);
                    }
                } else {
                    let msg = format!("unknown path/group {}", &path_segment);
                    log::error!("{}", &msg);
                    // let's not be so harsh as to throw an error, ok?
                    // Err(Error::new(ErrorKind::InvalidData, msg))
                }
            }
            return Ok(Some(complemented_path_segments))
        } 
        Ok(None)
    }

    fn load_coord_list(file_name: &str) -> Option<Vec<PathSegment>> {
        if !file_name.is_empty() {
            log::info!("loading coordinates from {}", file_name);
            let mut data = BufReader::new(fs::File::open(file_name).expect(&format!("Could not open file {}", file_name)));
            let use_block_info = true;
            let coords = parse_bed_to_path_segments(&mut data, use_block_info);
            log::debug!("loaded {} coordinates", coords.len());
            return Some(coords)
        }
        None
    }

    fn parse_groups<R: Read>(data: &mut BufReader<R>) -> Result<Vec<(PathSegment, String)>, Error> {
        let mut res: Vec<(PathSegment, String)> = Vec::new();

        let mut i = 1;
        let mut buf = vec![];
        while data.read_until(b'\n', &mut buf).unwrap_or(0) > 0 {
            //Remove new line at the end
            if let Some(&last_byte) = buf.last() {
                if last_byte == b'\n' || last_byte == b'\r' {
                    buf.pop();
                }
            }
            let line = String::from_utf8(buf.clone()).expect(&format!("error in line {}: some character is not UTF-8",i));
            let columns: Vec<&str> = line.split('\t').collect();

            if columns.len() != 2 {
                let msg = format!("error in line {}: table must have exactly two columns", i);
                log::error!("{}", &msg);
                return Err(Error::new(ErrorKind::InvalidData, msg));
            }

            let path_seg = PathSegment::from_str(columns[0]);
            res.push((path_seg, columns[1].to_string()));

            i += 1;
            buf.clear();
        }

        Ok(res)
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
            let group_assignments = Self::parse_groups(&mut data)?;
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

    pub fn get_path_order<'a>(
        &'a self,
        path_segments: &Vec<PathSegment>,
    ) -> Vec<(ItemId, &'a str)> {
        // orders elements of path_segments by the order in path_aux.include; the returned vector
        // maps indices of path_segments to the group identifier

        let mut group_to_paths: HashMap<&'a str, Vec<(ItemId, &'a str)>> = HashMap::default();

        for (i, p) in path_segments.into_iter().enumerate() {
            let group: &'a str = self.groups.get(&p.clear_coords()).expect(&format!("{} not found groups", &p.clear_coords()));
            group_to_paths
                .entry(group)
                .or_insert(Vec::new())
                .push((i as ItemId, group));
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
        order
            .into_iter()
            .map(|p| {
                group_to_paths
                    .remove(&self.groups.get(&p.clear_coords()).unwrap()[..])
                    .unwrap_or(Vec::new())
            })
            .collect::<Vec<Vec<(ItemId, &'a str)>>>()
            .concat()
    }

    #[allow(dead_code)]
    pub fn count_groups(&self) -> usize {
        HashSet::<&String>::from_iter(self.groups.values()).len()
    }

    pub fn build_subpath_map(
        path_segments: &Vec<PathSegment>,
    ) -> HashMap<String, Vec<(usize, usize)>> {
        // intervals are 0-based, and [start, end), see https://en.wikipedia.org/wiki/BED_(file_format)
        let mut res: HashMap<String, HashSet<(usize, usize)>> = HashMap::default();

        path_segments.into_iter().for_each(|x| {
            res.entry(x.id())
                .or_insert(HashSet::default())
                .insert(match x.coords() {
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

    pub fn load_optional_subsetting(
        &self,
        graph_aux: &GraphAuxilliary,
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
                graph_aux.number_of_items(count) + 1,
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

    fn check_order_comprises_all_included_coords(
        order: &Vec<PathSegment>, 
        graph_aux: &GraphAuxilliary, 
        include_coords: &Option<Vec<PathSegment>>, 
        exclude_coords: &Option<Vec<PathSegment>>,
    ) {
        let all_included_paths: Vec<PathSegment> = match include_coords {
            None => {
                let exclude: HashSet<&PathSegment> = match exclude_coords {
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
            Some(include) => include.iter().map(|x| x.clear_coords()).collect(),
        };
        let order_set: HashSet<&PathSegment> = HashSet::from_iter(order.iter());

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
    }
    fn check_order_groups_are_not_scrambled(
        order: &Vec<PathSegment>, 
        groups: &HashMap<PathSegment, String>
    ) -> Result<(), Error> {
        let mut visited: HashSet<&str> = HashSet::new();
        let mut cur: &str = groups.get(&order[0]).unwrap();
        for p in order.iter() {
            let g: &str = groups.get(p).unwrap();
            if cur != g && !visited.insert(g) {
                let msg = format!("order of paths contains fragmented groups: path {} belongs to group that is interspersed by one or more other groups", p);
                log::error!("{}", &msg);
                return Err(Error::new(ErrorKind::InvalidData, msg));
            }
            cur = g;
        }

        Ok(())
    }
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
    fn test_parse_groups_with_valid_input() {
        //let (graph_aux, _, _) = setup_test_data();
        let file_name = "test/test_groups.txt";
        let test_path_segments = vec![
            PathSegment::from_str("a#0"),
            PathSegment::from_str("b#0"),
            PathSegment::from_str("c#0"),
            PathSegment::from_str("c#1"),
            PathSegment::from_str("d#0")
        ];
        let test_groups = vec!["G1","G1","G2","G2","G2"];

        let mut data = BufReader::new(fs::File::open(file_name).unwrap());
        let result = PathAuxilliary::parse_groups(&mut data);
        assert!(result.is_ok(), "Expected successful group loading");
        let path_segments_group = result.unwrap();
        assert!(path_segments_group.len() > 0, "Expected non-empty group assignments");
        assert_eq!(path_segments_group.len(), 5); // number of paths == groups
        for (i, (path_seg, group)) in path_segments_group.into_iter().enumerate() {
            assert_eq!(path_seg, test_path_segments[i]);
            assert_eq!(group, test_groups[i]);
        }
    }

    #[test]
    fn test_path_auxilliary_from_params_success() {
        let (graph_aux, params, _) = setup_test_data();

        let path_aux = PathAuxilliary::from_params(&params, &graph_aux);
        assert!(path_aux.is_ok(), "Expected successful creation of PathAuxilliary");

        let path_aux = path_aux.unwrap();
        dbg!(&path_aux.groups.len());
        assert_eq!(path_aux.groups.len(), 6); // number of paths == groups
    }

    #[test]
    fn test_path_auxilliary_load_groups_by_sample() {
        let (graph_aux, _, _) = setup_test_data();

        let result = PathAuxilliary::load_groups("", false, true, &graph_aux);
        assert!(result.is_ok(), "Expected successful group loading by sample");
        let groups = result.unwrap();
        let mut group_count = HashSet::new();
        for (_, g) in groups {
            group_count.insert(g);
        }
        assert_eq!(group_count.len(), 4, "Expected one group per sample");
    }

    #[test]
    fn test_path_auxilliary_load_groups_by_haplotype() {
        let (graph_aux, _, _) = setup_test_data();

        let result = PathAuxilliary::load_groups("", true, false, &graph_aux);
        let groups = result.unwrap();
        let mut group_count = HashSet::new();
        for (_, g) in groups {
            group_count.insert(g);
        }
        assert_eq!(group_count.len(), 5, "Expected 5 groups based on haplotype");
    }

    #[test]
    fn test_complement_with_group_assignments_valid() {
        let groups = HashMap::from([
            (PathSegment::from_str("a#1#h1"), "G1".to_string()),
            (PathSegment::from_str("b#1#h1"), "G1".to_string()),
            (PathSegment::from_str("c#1#h1"), "G2".to_string()),
        ]);

        let coords = Some(vec![PathSegment::from_str("G1")]);
        let result = PathAuxilliary::complement_with_group_assignments(coords, &groups);
        assert!(result.is_ok(), "Expected successful complement with group assignments");

        let complemented = result.unwrap();
        assert!(complemented.is_some(), "Expected Some(complemented) coordinates");
        assert_eq!(complemented.unwrap().len(), 2, "Expected 2 path segments in the complemented list");
    }

    #[test]
    fn test_complement_with_group_assignments_invalid() {
        let groups = HashMap::from([
            (PathSegment::from_str("a#0"), "G1".to_string()),
            (PathSegment::from_str("b#0"), "G1".to_string()),
        ]);

        let coords = Some(vec![PathSegment::from_str("G1:1-5")]);
        let result = PathAuxilliary::complement_with_group_assignments(coords, &groups);
        assert!(result.is_err(), "Expected error due to invalid group identifier with start/stop information");
    }

    #[test]
    fn test_build_subpath_map_with_overlaps() {
        let path_segments = vec![
            PathSegment::new("sample".to_string(), "hap1".to_string(), "seq1".to_string(), Some(0), Some(100)),
            PathSegment::new("sample".to_string(), "hap1".to_string(), "seq1".to_string(), Some(50), Some(150)),
            PathSegment::new("sample".to_string(), "hap1".to_string(), "seq2".to_string(), Some(0), Some(100)),
        ];

        let subpath_map = PathAuxilliary::build_subpath_map(&path_segments);
        assert_eq!(subpath_map.len(), 2, "Expected 2 sequences in the subpath map");
        assert_eq!(subpath_map.get("sample#hap1#seq1").unwrap().len(), 1, "Expected 1 non-overlapping interval for seq1");
        assert_eq!(subpath_map.get("sample#hap1#seq2").unwrap().len(), 1, "Expected 1 interval for seq2");
    }

    #[test]
    fn test_get_path_order_with_exclusions() {
        let (graph_aux, _, _) = setup_test_data();

        let path_aux = PathAuxilliary {
            groups: PathAuxilliary::load_groups("", false, false, &graph_aux).unwrap(),
            include_coords: None,
            exclude_coords: Some(vec![PathSegment::from_str("a#1#h1"), 
                                      PathSegment::from_str("b#1#h1"),
                                      PathSegment::from_str("b#1#h1")]), //duplicates do not cause any error
            order: None,
        };
        let ordered_paths = path_aux.get_path_order(&graph_aux.path_segments);
        assert_eq!(ordered_paths.len(), 4, "Expected 4 paths in the final order");
    }

    #[test]
    fn test_path_auxilliary_count_groups() {
        let path_aux = PathAuxilliary {
            groups: HashMap::from([
                (PathSegment::from_str("a#1#h1"), "G1".to_string()),
                (PathSegment::from_str("b#1#h1"), "G1".to_string()),
                (PathSegment::from_str("c#1#h1"), "G2".to_string()),
            ]),
            include_coords: None,
            exclude_coords: None,
            order: None,
        };

        assert_eq!(path_aux.count_groups(), 2, "Expected 2 unique groups");
    }
}
