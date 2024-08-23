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
use crate::cli::Params;
use crate::graph::*;
use crate::io::*;
use crate::util::*;

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

    pub fn parse_path_segment(data: &[u8]) -> Self {
        let mut iter = data.iter();
        let start = iter.position(|&x| x == b'\t').unwrap() + 1;
        let offset = iter.position(|&x| x == b'\t').unwrap();
        let path_name = str::from_utf8(&data[start..start + offset]).unwrap();
        PathSegment::from_str(path_name)
    }

    pub fn parse_walk_segment(data: &[u8]) -> Self {
        let mut six_col: Vec<&str> = Vec::with_capacity(6);

        let mut it = data.iter();
        let mut i = 0;
        for _ in 0..6 {
            let j = it.position(|x| x == &b'\t').unwrap();
            six_col.push(&str::from_utf8(&data[i..i + j]).unwrap());
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
        match params {
            Params::Histgrowth {
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
            => {
                let groups = PathAuxilliary::load_groups(
                    groupby,
                    *groupby_haplotype,
                    *groupby_sample,
                    graph_aux,
                )?;
                let include_coords = PathAuxilliary::complement_with_group_assignments(
                    PathAuxilliary::load_coord_list(positive_list)?,
                    &groups,
                )?;
                let exclude_coords = PathAuxilliary::complement_with_group_assignments(
                    PathAuxilliary::load_coord_list(negative_list)?,
                    &groups,
                )?;

                let order = if let Params::OrderedHistgrowth { order, .. } = params {
                    let maybe_order = PathAuxilliary::complement_with_group_assignments(
                        PathAuxilliary::load_coord_list(order)?,
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
                        let mut cur: &str = groups.get(&o[0]).unwrap();
                        for p in o.iter() {
                            let g: &str = groups.get(p).unwrap();
                            if cur != g && !visited.insert(g) {
                                let msg = format!("order of paths contains fragmented groups: path {} belongs to group that is interspersed by one or more other groups", p);
                                log::error!("{}", &msg);
                                return Err(Error::new(ErrorKind::InvalidData, msg));
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

                Ok(PathAuxilliary {
                    groups: groups,
                    include_coords: include_coords,
                    exclude_coords: exclude_coords,
                    order: order,
                })
            }
            _ => Err(Error::new(
                ErrorKind::InvalidData,
                "cannot produce PathAuxilliary from other Param items",
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
            let group: &'a str = self.groups.get(&p.clear_coords()).unwrap();
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
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_test_data() -> (GraphAuxilliary, Params, String) {
        let test_gfa_file = "test/cdbg.gfa";
        let graph_aux = GraphAuxilliary::from_gfa(test_gfa_file, CountType::Node);
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
        let mut test_path_segments = vec![];
        test_path_segments.push(PathSegment::from_str("a#0"));
        test_path_segments.push(PathSegment::from_str("b#0"));
        test_path_segments.push(PathSegment::from_str("c#0"));
        test_path_segments.push(PathSegment::from_str("c#1"));
        test_path_segments.push(PathSegment::from_str("d#0"));
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
        assert_eq!(path_aux.groups.len(), 5); // number of paths == groups
    }
}
