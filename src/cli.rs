/* standard crate */
use std::fs;
use std::path::Path;
use std::str::FromStr;
/* external crate */
use clap::{Parser, Subcommand};
use regex::Regex;
/* private use */
use crate::graph::*;
use crate::io;

#[derive(Parser, Debug)]
#[clap(
    version = "0.2",
    author = "Luca Parmigiani <lparmig@cebitec.uni-bielefeld.de>, Daniel Doerr <daniel.doerr@hhu.de>",
    about = "Calculate count statistics for pangenomic data"
)]

struct Command {
    #[clap(subcommand)]
    cmd: SubCommand,
}

#[derive(Subcommand, Debug)]
enum SubCommand {
    #[clap(
        about = "run in default mode, i.e., run hist an growth successively and output only the results of the latter"
    )]
    Growth {
        #[clap(index = 1, help = "graph in GFA1 format", required = true)]
        gfa_file: String,

        #[clap(short, long,
        help = "count type: node or edge count",
        default_value = "nodes",
        possible_values = &["nodes", "edges", "bp"],
    )]
        count: String,

        #[clap(
            name = "subset",
            short,
            long,
            help = "produce counts by subsetting the graph to a given list of paths (1-column list) or path coordinates (3- or 12-column BED file)",
            default_value = ""
        )]
        positive_list: String,

        #[clap(
            name = "exclude",
            short,
            long,
            help = "exclude bps/nodes/edges in growth count that intersect with paths (1-column list) or path coordinates (3- or 12-column BED-file) provided by the given file",
            default_value = ""
        )]
        negative_list: String,

        #[clap(
            short,
            long,
            help = "merge counts from paths by path-group mapping from given tab-separated two-column file",
            default_value = ""
        )]
        groupby: String,

        #[clap(
            short,
            long,
            help = "list of (named) intersection thresholds of the form <level1>,<level2>,.. or <name1>=<level1>,<name2>=<level2> or a file that provides these levels in a tab-separated format; a level is absolute, i.e., corresponds to a number of paths/groups IFF it is integer, otherwise it is a float value representing a percentage of paths/groups.",
            default_value = "cumulative_count=1"
        )]
        intersection: String,

        #[clap(
            short = 'l',
            long,
            help = "list of (named) coverage thresholds of the form <level1>,<level2>,.. or <name1>=<level1>,<name2>=<level2> or a file that provides these levels in a tab-separated format; a level is absolute, i.e., corresponds to a number of paths/groups IFF it is integer, otherwise it is a float value representing a percentage of paths/groups.",
            default_value = "cumulative_count=1"
        )]
        coverage: String,

        #[clap(
            short,
            long,
            help = "run in parallel on N threads",
            default_value = "1"
        )]
        threads: usize,
    },

    #[clap(about = "calculate coverage histogram from GFA file")]
    Hist{
        #[clap(index = 1, help = "graph in GFA1 format", required = true)]
        gfa_file: String,

        #[clap(short, long,
        help = "count type: node or edge count",
        default_value = "nodes",
        possible_values = &["nodes", "edges", "bp"],
    )]
        count: String,

        #[clap(
            name = "subset",
            short,
            long,
            help = "produce counts by subsetting the graph to a given list of paths (1-column list) or path coordinates (3- or 12-column BED file)",
            default_value = ""
        )]
        positive_list: String,

        #[clap(
            name = "exclude",
            short,
            long,
            help = "exclude bps/nodes/edges in growth count that intersect with paths (1-column list) or path coordinates (3- or 12-column BED-file) provided by the given file",
            default_value = ""
        )]
        negative_list: String,

        #[clap(
            short,
            long,
            help = "merge counts from paths by path-group mapping from given tab-separated two-column file",
            default_value = ""
        )]
        groupby: String,

        #[clap(
            short,
            long,
            help = "run in parallel on N threads",
            default_value = "1"
        )]
        threads: usize,
    },

    #[clap(about = "construct growth table from coverage histogram")]
    GrowthOnly,

    #[clap(about = "compute growth table for order specified in grouping file (or, if non specified, the order of paths in the GFA file)")]
    OrderedGrowth,
}

pub enum Params {
    Growth {
        gfa_file: String,
        count: CountType,
        groups: Option<Vec<(PathSegment, String)>>,
        subset_coords: Option<Vec<PathSegment>>,
        exclude_coords: Option<Vec<PathSegment>>,
        intersection: Option<Vec<(String, Threshold)>>,
        coverage: Option<Vec<(String, Threshold)>>,
    },
    Hist {
        gfa_file: String,
        count: CountType,
        groups: Option<Vec<(PathSegment, String)>>,
        subset_coords: Option<Vec<PathSegment>>,
        exclude_coords: Option<Vec<PathSegment>>,
    },
}

pub enum CountType {
    Node,
    BasePair,
    Edge,
}

pub fn read_params() -> Result<Params, std::io::Error> {
    // initialize command line parser & parse command line arguments
    let args = Command::parse();

    match args.cmd {
        SubCommand::Growth {
            gfa_file,
            count,
            positive_list,
            negative_list,
            groupby,
            intersection,
            coverage,
            threads,
        } => {
            if threads > 0 {
                log::info!("running pangenome-growth on {} threads", &threads);
                rayon::ThreadPoolBuilder::new()
                    .num_threads(threads)
                    .build_global()
                    .unwrap();
            } else {
                log::info!("running pangenome-growth using all available CPUs");
            }

            let mut subset_coords = None;
            if !positive_list.is_empty() {
                log::info!("loading subset coordinates from {}", &positive_list);
                let mut data = std::io::BufReader::new(fs::File::open(&positive_list)?);
                subset_coords = Some(io::parse_bed(&mut data));
                log::debug!(
                    "loaded {} coordinates",
                    subset_coords.as_ref().unwrap().len()
                );
            }

            let mut exclude_coords = None;
            if !negative_list.is_empty() {
                log::info!("loading exclusion coordinates from {}", &negative_list);
                let mut data = std::io::BufReader::new(fs::File::open(&negative_list)?);
                exclude_coords = Some(io::parse_bed(&mut data));
                log::debug!(
                    "loaded {} coordinates",
                    exclude_coords.as_ref().unwrap().len()
                );
            }

            let mut groups = None;
            if !groupby.is_empty() {
                log::info!("loading groups from {}", &groupby);
                let mut data = std::io::BufReader::new(fs::File::open(&groupby)?);
                groups = Some(io::parse_groups(&mut data));
                log::debug!(
                    "loaded {} group assignments ",
                    groups.as_ref().unwrap().len()
                );
            }

            let mut intersection_thresholds = None;
            if !intersection.is_empty() {
                if Path::new(&intersection).exists() {
                    log::info!("loading intersection thresholds from {}", &intersection);
                    let mut data = std::io::BufReader::new(fs::File::open(&groupby)?);
                    intersection_thresholds = Some(io::parse_coverage_threshold_file(&mut data));
                } else {
                    intersection_thresholds = Some(parse_coverage_threshold_cli(&intersection[..]));
                }
                log::debug!(
                    "loaded {} intersection thresholds:\n{}",
                    intersection_thresholds.as_ref().unwrap().len(),
                    intersection_thresholds
                        .as_ref()
                        .unwrap()
                        .iter()
                        .map(|(n, t)| format!("\t{}: {}", n, t))
                        .collect::<Vec<String>>()
                        .join("\n")
                );
            }

            let mut coverage_thresholds = None;
            if !coverage.is_empty() {
                if Path::new(&coverage).exists() {
                    log::info!("loading coverage thresholds from {}", &coverage);
                    let mut data = std::io::BufReader::new(fs::File::open(&groupby)?);
                    coverage_thresholds = Some(io::parse_coverage_threshold_file(&mut data));
                } else {
                    coverage_thresholds = Some(parse_coverage_threshold_cli(&coverage[..]));
                }
                log::debug!(
                    "loaded {} coverage thresholds:\n{}",
                    coverage_thresholds.as_ref().unwrap().len(),
                    coverage_thresholds
                        .as_ref()
                        .unwrap()
                        .iter()
                        .map(|(n, t)| format!("\t{}: {}", n, t))
                        .collect::<Vec<String>>()
                        .join("\n")
                );
            }

            Ok(Params::Growth {
                gfa_file: gfa_file,
                count: match &count[..] {
                    "nodes" => CountType::Node,
                    "edges" => CountType::Edge,
                    _ => CountType::BasePair,
                },
                subset_coords: subset_coords,
                exclude_coords: exclude_coords,
                groups: groups,
                intersection: intersection_thresholds,
                coverage: coverage_thresholds,
            })

        }
        _ => Err(std::io::Error::new(std::io::ErrorKind::Other, "oops")),
    }
}

pub fn parse_coverage_threshold_cli(threshold_str: &str) -> Vec<(String, Threshold)> {
    let mut coverage_thresholds = Vec::new();

    let re = Regex::new(r"^\s?([!-<,>-~]+)\s?=\s?([!-<,>-~]+)\s*$").unwrap();
    for el in threshold_str.split(',') {
        if let Some(t) = usize::from_str(el.trim()).ok() {
            coverage_thresholds.push((el.trim().to_string(), Threshold::Absolute(t)));
        } else if let Some(t) = f64::from_str(el.trim()).ok() {
            coverage_thresholds.push((el.trim().to_string(), Threshold::Relative(t)));
        } else if let Some(caps) = re.captures(&el) {
            let name = caps.get(1).unwrap().as_str().trim().to_string();
            let threshold_str = caps.get(2).unwrap().as_str();
            let threshold = if let Some(t) = usize::from_str(threshold_str).ok() {
                Threshold::Absolute(t)
            } else {
                Threshold::Relative(f64::from_str(threshold_str).unwrap())
            };
            coverage_thresholds.push((name, threshold));
        } else {
            panic!(
                "coverage threshold \"{}\" string is not well-formed",
                &threshold_str
            );
        }
    }

    coverage_thresholds
}




