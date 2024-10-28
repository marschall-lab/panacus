use core::fmt;
use std::{
    collections::{HashMap, HashSet},
    io::{BufWriter, Error, Write},
};

use clap::{arg, ArgMatches, Command};

use crate::{
    analyses::{Analysis, AnalysisSection, InputRequirement},
    data_manager::{DataManager, Edge, ItemId, ViewParams},
    html_report::{AnalysisTab, ReportItem},
    util::{averageu32, median_already_sorted, n50_already_sorted},
};

pub struct Info {
    pub graph_info: GraphInfo,
    pub path_info: PathInfo,
    pub group_info: Option<GroupInfo>,
}

impl Analysis for Info {
    fn build(dm: &DataManager, _matches: &ArgMatches) -> Result<Box<Self>, Error> {
        Ok(Box::new(Info {
            graph_info: GraphInfo::from(dm),
            path_info: PathInfo::from(dm),
            group_info: Some(GroupInfo::from(dm)),
        }))
    }

    fn write_table<W: Write>(
        &mut self,
        _dm: &DataManager,
        out: &mut BufWriter<W>,
    ) -> Result<(), Error> {
        writeln!(out, "{}", self)
    }

    fn generate_report_section(&mut self, _dm: &DataManager) -> Vec<AnalysisSection> {
        let (graph_header, graph_values) = self.get_graph_table();
        let graph_values = Self::remove_duplication(graph_values);
        let (node_header, node_values) = self.get_node_table();
        let node_values = Self::remove_duplication(node_values);
        let (path_header, path_values) = self.get_path_table();
        let path_values = Self::remove_duplication(path_values);

        let mut buf = BufWriter::new(Vec::new());
        self.write_table(_dm, &mut buf)
            .expect("Can write to string");
        let bytes = buf.into_inner().unwrap();
        let table = String::from_utf8(bytes).unwrap();
        let table = format!("`{}`", &table);
        vec![AnalysisSection {
            name: "pangenome info".to_string(),
            id: "info".to_string(),
            is_first: true,
            table: Some(table),
            tabs: vec![
                AnalysisTab {
                    id: "info-1".to_string(),
                    is_first: true,
                    name: "graph".to_string(),
                    items: vec![ReportItem::Table {
                        id: "info-1-table".to_string(),
                        header: graph_header,
                        values: graph_values,
                    }],
                },
                AnalysisTab {
                    id: "info-2".to_string(),
                    is_first: false,
                    name: "node".to_string(),
                    items: vec![ReportItem::Table {
                        id: "info-2-table".to_string(),
                        header: node_header,
                        values: node_values,
                    }],
                },
                AnalysisTab {
                    id: "info-3".to_string(),
                    is_first: false,
                    name: "path".to_string(),
                    items: vec![ReportItem::Table {
                        id: "info-3-table".to_string(),
                        header: path_header,
                        values: path_values,
                    }],
                },
                AnalysisTab {
                    id: "info-4".to_string(),
                    is_first: false,
                    name: "group".to_string(),
                    items: vec![self.get_group_bar("node"), self.get_group_bar("bp")],
                },
            ],
        }]
    }

    fn get_subcommand() -> Command {
        Command::new("info")
            .about("Return general graph and paths info")
            .args(&[
                arg!(gfa_file: <GFA_FILE> "graph in GFA1 format, accepts also compressed (.gz) file"),
                arg!(-s --subset <FILE> "Produce counts by subsetting the graph to a given list of paths (1-column list) or path coordinates (3- or 12-column BED file)"),
                arg!(-e --exclude <FILE> "Exclude bp/node/edge in growth count that intersect with paths (1-column list) or path coordinates (3- or 12-column BED-file) provided by the given file; all intersecting bp/node/edge will be exluded also in other paths not part of the given list"),
                arg!(-g --groupby <FILE> "Merge counts from paths by path-group mapping from given tab-separated two-column file"),
                arg!(-H --"groupby-haplotype" "Merge counts from paths belonging to same haplotype"),
                arg!(-S --"groupby-sample" "Merge counts from paths belonging to same sample"),
            ])
    }

    fn get_input_requirements(
        matches: &ArgMatches,
    ) -> Option<(HashSet<InputRequirement>, ViewParams, String)> {
        let matches = matches.subcommand_matches("info")?;
        let req = HashSet::from([
            InputRequirement::Node,
            InputRequirement::Edge,
            InputRequirement::Bp,
            InputRequirement::PathLens,
        ]);
        // TODO: validate_single_groupby_option(groupby, groupby_haplotype, groupby_sample)
        let view = ViewParams {
            groupby: matches
                .get_one::<String>("groupby")
                .cloned()
                .unwrap_or_default(),
            groupby_haplotype: matches.get_flag("groupby-haplotype"),
            groupby_sample: matches.get_flag("groupby-sample"),
            positive_list: matches
                .get_one::<String>("subset")
                .cloned()
                .unwrap_or_default(),
            negative_list: matches
                .get_one::<String>("exclude")
                .cloned()
                .unwrap_or_default(),
            order: None,
        };
        let file_name = matches.get_one::<String>("gfa_file")?.to_owned();
        log::debug!("input params: {:?}, {:?}, {:?}", req, view, file_name);
        Some((req, view, file_name))
    }
}

impl Info {
    fn get_graph_table(&self) -> (Vec<String>, Vec<Vec<String>>) {
        let header = Self::get_header();
        let values = vec![
            Self::get_row(
                "graph",
                "total",
                "node",
                self.graph_info.node_count.to_string(),
            ),
            Self::get_row(
                "graph",
                "total",
                "bp",
                self.graph_info.basepairs.to_string(),
            ),
            Self::get_row(
                "graph",
                "total",
                "edge",
                self.graph_info.edge_count.to_string(),
            ),
            Self::get_row(
                "graph",
                "total",
                "path",
                self.path_info.no_paths.to_string(),
            ),
            Self::get_row(
                "graph",
                "total",
                "group",
                self.graph_info.group_count.to_string(),
            ),
            Self::get_row(
                "graph",
                "total",
                "0-degree node",
                self.graph_info.number_0_degree.to_string(),
            ),
            Self::get_row(
                "graph",
                "total",
                "component",
                self.graph_info.connected_components.to_string(),
            ),
            Self::get_row(
                "graph",
                "largest",
                "component",
                self.graph_info.largest_component.to_string(),
            ),
            Self::get_row(
                "graph",
                "smallest",
                "component",
                self.graph_info.smallest_component.to_string(),
            ),
            Self::get_row(
                "graph",
                "median",
                "component",
                self.graph_info.median_component.to_string(),
            ),
        ];
        (header, values)
    }

    fn get_node_table(&self) -> (Vec<String>, Vec<Vec<String>>) {
        let header = Self::get_header();
        let values = vec![
            Self::get_row(
                "node",
                "average",
                "bp",
                self.graph_info.average_node.to_string(),
            ),
            Self::get_row(
                "node",
                "average",
                "degree",
                self.graph_info.average_degree.to_string(),
            ),
            Self::get_row(
                "node",
                "longest",
                "bp",
                self.graph_info.largest_node.to_string(),
            ),
            Self::get_row(
                "node",
                "shortest",
                "bp",
                self.graph_info.shortest_node.to_string(),
            ),
            Self::get_row(
                "node",
                "median",
                "bp",
                self.graph_info.median_node.to_string(),
            ),
            Self::get_row(
                "node",
                "N50 node",
                "bp",
                self.graph_info.n50_node.to_string(),
            ),
            Self::get_row(
                "node",
                "max",
                "degree",
                self.graph_info.max_degree.to_string(),
            ),
            Self::get_row(
                "node",
                "min",
                "degree",
                self.graph_info.min_degree.to_string(),
            ),
        ];
        (header, values)
    }

    fn get_group_bar(&self, countable: &str) -> ReportItem {
        let groups = &self.group_info.as_ref().unwrap().groups;
        let (labels, values): (Vec<_>, Vec<_>) = if countable == "node" {
            groups.iter().map(|(k, v)| (k.to_string(), v.0)).unzip()
        } else {
            groups.iter().map(|(k, v)| (k.to_string(), v.1)).unzip()
        };
        if labels.len() <= 100 {
            ReportItem::Bar {
                id: format!("info-group-{}", countable),
                name: countable.to_string(),
                x_label: "groups".to_string(),
                y_label: format!("#{}s", countable),
                log_toggle: true,
                labels,
                values: values.into_iter().map(|v| v as f64).collect(),
            }
        } else {
            let (labels, values) = Self::bin_values(values);
            ReportItem::Bar {
                id: format!("info-group-{}", countable),
                name: countable.to_string(),
                x_label: "groups".to_string(),
                y_label: format!("#{}s", countable),
                log_toggle: true,
                labels,
                values: values.into_iter().map(|v| v as f64).collect(),
            }
        }
    }

    fn bin_values(list: Vec<u32>) -> (Vec<String>, Vec<usize>) {
        if list.is_empty() {
            return (Vec::new(), Vec::new());
        }
        let n_bins = 50;
        let max = *list.iter().max().unwrap();
        let min = *list.iter().min().unwrap();
        let bin_size = ((max - min) as f32 / n_bins as f32).round();
        let bins: Vec<_> = (min..max)
            .step_by(bin_size as usize)
            .zip((min + (bin_size as u32)..max + 1).step_by(bin_size as usize))
            .collect();
        let values = bins
            .iter()
            .map(|(s, e)| list.iter().filter(|a| **a >= *s && **a < *e).count())
            .collect::<Vec<_>>();
        let bin_names = bins
            .iter()
            .map(|(s, e)| format!("{}-{}", s, e))
            .collect::<Vec<_>>();
        (bin_names, values)
    }

    fn get_path_table(&self) -> (Vec<String>, Vec<Vec<String>>) {
        let header = Self::get_header();
        let values = vec![
            Self::get_row(
                "path",
                "average",
                "bp",
                self.path_info.bp_len.average.to_string(),
            ),
            Self::get_row(
                "path",
                "average",
                "node",
                self.path_info.node_len.average.to_string(),
            ),
            Self::get_row(
                "path",
                "longest",
                "bp",
                self.path_info.bp_len.longest.to_string(),
            ),
            Self::get_row(
                "path",
                "longest",
                "node",
                self.path_info.node_len.longest.to_string(),
            ),
            Self::get_row(
                "path",
                "shortest",
                "bp",
                self.path_info.bp_len.shortest.to_string(),
            ),
            Self::get_row(
                "path",
                "shortest",
                "node",
                self.path_info.node_len.shortest.to_string(),
            ),
        ];
        (header, values)
    }

    fn get_row(first: &str, second: &str, third: &str, value: String) -> Vec<String> {
        vec![
            first.to_string(),
            second.to_string(),
            third.to_string(),
            value,
        ]
    }

    fn get_header() -> Vec<String> {
        vec![
            "feature".to_string(),
            "category".to_string(),
            "countable".to_string(),
            "value".to_string(),
        ]
    }

    fn remove_duplication(values: Vec<Vec<String>>) -> Vec<Vec<String>> {
        let mut new = values.clone();
        let mut prev_row = &values[0];
        for (j, row) in values.iter().enumerate().skip(1) {
            for (i, col) in row.iter().enumerate() {
                if *col == prev_row[i] {
                    new[j][i] = String::new();
                } else {
                    break;
                }
            }
            prev_row = &values[j];
        }
        new
    }
}

impl fmt::Display for Info {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "feature\tcategory\tcountable\tvalue")?;
        writeln!(f, "graph\ttotal\tnode\t{}", self.graph_info.node_count)?;
        writeln!(f, "graph\ttotal\tbp\t{}", self.graph_info.basepairs)?;
        writeln!(f, "graph\ttotal\tedge\t{}", self.graph_info.edge_count)?;
        writeln!(f, "graph\ttotal\tpath\t{}", self.path_info.no_paths)?;
        writeln!(f, "graph\ttotal\tgroup\t{}", self.graph_info.group_count)?;
        writeln!(
            f,
            "graph\ttotal\t0-degree node\t{}",
            self.graph_info.number_0_degree
        )?;
        writeln!(
            f,
            "graph\ttotal\tcomponent\t{}",
            self.graph_info.connected_components
        )?;
        writeln!(
            f,
            "graph\tlargest\tcomponent\t{}",
            self.graph_info.largest_component
        )?;
        writeln!(
            f,
            "graph\tsmallest\tcomponent\t{}",
            self.graph_info.smallest_component
        )?;
        writeln!(
            f,
            "graph\tmedian\tcomponent\t{}",
            self.graph_info.median_component
        )?;
        writeln!(f, "node\taverage\tbp\t{}", self.graph_info.average_node)?;
        writeln!(
            f,
            "node\taverage\tdegree\t{}",
            self.graph_info.average_degree
        )?;
        writeln!(f, "node\tlongest\tbp\t{}", self.graph_info.largest_node)?;
        writeln!(f, "node\tshortest\tbp\t{}", self.graph_info.shortest_node)?;
        writeln!(f, "node\tmedian\tbp\t{}", self.graph_info.median_node)?;
        writeln!(f, "node\tN50 node\tbp\t{}", self.graph_info.n50_node)?;
        writeln!(f, "node\tmax\tdegree\t{}", self.graph_info.max_degree)?;
        writeln!(f, "node\tmin\tdegree\t{}", self.graph_info.min_degree)?;
        writeln!(f, "path\taverage\tbp\t{}", self.path_info.bp_len.average)?;
        writeln!(
            f,
            "path\taverage\tnode\t{}",
            self.path_info.node_len.average
        )?;
        writeln!(f, "path\tlongest\tbp\t{}", self.path_info.bp_len.longest)?;
        writeln!(
            f,
            "path\tlongest\tnode\t{}",
            self.path_info.node_len.longest
        )?;
        writeln!(f, "path\tshortest\tbp\t{}", self.path_info.bp_len.shortest)?;
        write!(
            f,
            "path\tshortest\tnode\t{}",
            self.path_info.node_len.shortest
        )?;
        if let Some(group_info) = &self.group_info {
            let mut sorted: Vec<_> = group_info.groups.clone().into_iter().collect();
            sorted.sort_by(|(k0, _v0), (k1, _v1)| k0.cmp(k1));
            for (k, v) in sorted {
                write!(f, "\ngroup\t{}\tbp\t{}\n", k, v.1)?;
                write!(f, "group\t{}\tnode\t{}", k, v.0)?;
            }
        }
        Ok(())
    }
}

pub struct GraphInfo {
    pub node_count: usize,
    pub edge_count: usize,
    pub average_degree: f32,
    pub max_degree: u32,
    pub min_degree: u32,
    pub number_0_degree: usize,
    pub connected_components: u32,
    pub largest_component: u32,
    pub smallest_component: u32,
    pub median_component: f64,
    pub largest_node: u32,
    pub shortest_node: u32,
    pub average_node: f32,
    pub median_node: f64,
    pub n50_node: u32,
    pub basepairs: u32,
    pub group_count: usize,
}

impl GraphInfo {
    fn from(dm: &DataManager) -> Self {
        let degree = dm.get_degree();
        let mut node_lens_sorted = dm.get_node_lens()[1..].to_vec();
        node_lens_sorted.sort_by(|a, b| b.cmp(a)); // decreasing, for N50
        let mut components = connected_components(dm.get_edges(), dm.get_nodes());
        components.sort();

        Self {
            node_count: dm.get_node_count(),
            edge_count: dm.get_edge_count(),
            average_degree: averageu32(&degree[1..]),
            max_degree: *degree[1..].iter().max().unwrap(),
            min_degree: *degree[1..].iter().min().unwrap(),
            number_0_degree: degree[1..].iter().filter(|&x| *x == 0).count(),
            connected_components: components.len() as u32,
            largest_component: *components.iter().max().unwrap_or(&0),
            smallest_component: *components.iter().min().unwrap_or(&0),
            median_component: median_already_sorted(&components),
            largest_node: *node_lens_sorted.iter().max().unwrap(),
            shortest_node: *node_lens_sorted.iter().min().unwrap(),
            average_node: averageu32(&node_lens_sorted),
            median_node: median_already_sorted(&node_lens_sorted),
            n50_node: n50_already_sorted(&node_lens_sorted).unwrap(),
            basepairs: dm.get_node_lens().iter().sum(),
            group_count: dm.get_group_count(),
        }
    }
}

pub struct PathInfo {
    pub no_paths: usize,
    pub node_len: LenInfo,
    pub bp_len: LenInfo,
}

impl PathInfo {
    fn from(dm: &DataManager) -> Self {
        let paths_len = dm.get_path_lens();
        let paths_bp_len: Vec<_> = paths_len.values().map(|x| x.1).collect();
        let paths_len: Vec<_> = paths_len.values().map(|x| x.0).collect();
        Self {
            no_paths: paths_len.len(),
            node_len: LenInfo {
                longest: *paths_len.iter().max().unwrap(),
                shortest: *paths_len.iter().min().unwrap(),
                average: averageu32(&paths_len),
            },
            bp_len: LenInfo {
                longest: *paths_bp_len.iter().max().unwrap(),
                shortest: *paths_bp_len.iter().min().unwrap(),
                average: averageu32(&paths_bp_len),
            },
        }
    }
}

pub struct LenInfo {
    pub longest: u32,
    pub shortest: u32,
    pub average: f32,
}

pub struct GroupInfo {
    pub groups: HashMap<String, (u32, u32)>,
}

impl GroupInfo {
    fn from(dm: &DataManager) -> Self {
        let groups = dm.get_groups();
        let mut group_map: HashMap<String, (u32, u32)> = HashMap::new();
        for (k, v) in dm.get_path_lens() {
            if !groups.contains_key(k) {
                continue;
            }
            let group = groups[k].clone();
            let tmp = group_map.entry(group).or_insert((0, 0));
            tmp.0 += v.0;
            tmp.1 += v.1;
        }

        GroupInfo { groups: group_map }
    }
}

fn connected_components(
    edge2id: &HashMap<Edge, ItemId>,
    node2id: &HashMap<Vec<u8>, ItemId>,
) -> Vec<u32> {
    let mut component_lengths = Vec::new();
    let mut visited: HashSet<ItemId> = HashSet::new();
    let edges: HashMap<ItemId, Vec<ItemId>> = edge2id
        .keys()
        .map(|x| (x.0, x.2))
        .chain(edge2id.keys().map(|x| (x.2, x.0)))
        .fold(HashMap::new(), |mut acc, (k, v)| {
            acc.entry(k).and_modify(|x| x.push(v)).or_insert(vec![v]);
            acc
        });
    let nodes: Vec<ItemId> = node2id.values().copied().collect();
    for node in &nodes {
        if !visited.contains(node) {
            component_lengths.push(dfs(&edges, *node, &mut visited));
        }
    }
    component_lengths
}

fn dfs(edges: &HashMap<ItemId, Vec<ItemId>>, node: ItemId, visited: &mut HashSet<ItemId>) -> u32 {
    let mut s = Vec::new();
    let mut length = 0;
    s.push(node);
    while let Some(v) = s.pop() {
        if visited.contains(&v) {
            continue;
        }
        visited.insert(v);
        length += 1;
        if !edges.contains_key(&v) {
            continue;
        }
        for neigh in &edges[&v] {
            if !visited.contains(neigh) {
                s.push(*neigh);
            }
        }
    }
    length
}
