use std::fs;
use std::path::Path;
use std::str::FromStr;
/* external crate */
use clap::Parser;
use regex::Regex;
/* private use */
use crate::io;
use crate::graph::{*};

#[derive(clap::Parser, Debug)]
#[clap(
    version = "0.2",
    author = "Daniel Doerr <daniel.doerr@hhu.de>",
    about = "Calculate growth statistics for pangenome graphs"
)]
pub struct Command {
    #[clap(index = 1, help = "graph in GFA1 format", required = true)]
    pub gfa_file: String,

    #[clap(
        short = 'c',
        long = "count",
        help = "count type: node or edge count",
        default_value = "nodes",
        possible_values = &["nodes", "edges", "bp"],
    )]
    pub count_type: String,

    #[clap(
        short = 's',
        long = "subset",
        help = "produce counts by subsetting the graph to a given list of paths (1-column list) or path coordinates (3- or 12-column BED file)",
        default_value = ""
    )]
    pub positive_list: String,

    #[clap(
        short = 'e',
        long = "exclude",
        help = "exclude bps/nodes/edges in growth count that intersect with paths (1-column list) or path coordinates (3- or 12-column BED-file) provided by the given file",
        default_value = ""
    )]
    pub negative_list: String,

    #[clap(
        short = 'g',
        long = "groupby",
        help = "merge counts from paths by path-group mapping from given tab-separated two-column file",
        default_value = ""
    )]
    pub groups: String,

    #[clap(
        short = 'l',
        long = "coverage_level",
        help = "list of (named) coverage levels of the form <level1>,<level2>,.. or <name1>=<level1>,<name2>=<level2> or a file that provides these levels in a tab-separated format; a level is absolute, i.e., corresponds to a number of paths/groups IFF it is integer, otherwise it is a float value representing a percentage of paths/groups.",
        default_value = "cumulative_count=1"
    )]
    pub thresholds: String,

    #[clap(
        short = 'a',
        long = "apriori",
        help = "identify coverage threshold groups a priori rather than during the cumulative counting"
    )]
    pub apriori: bool,

    #[clap(
        short = 'o',
        long = "ordered",
        help = "rather than computing growth across all permutations of the input, produce counts in the order of the paths in the GFA file, or, if a grouping file is specified, in the order of the provided groups"
    )]
    pub ordered: bool,

    #[clap(
        short = 't',
        long = "threads",
        help = "run in parallel on N threads",
        default_value = "1"
    )]
    pub threads: usize,
}

pub struct Params {
    pub gfa_file: String,
    pub threads: usize,
    pub subset_coords: Option<Vec<PathSegment>>,
    pub exclude_coords: Option<Vec<PathSegment>>,
    pub groups: Option<Vec<(PathSegment,String)>>,
    pub coverage_thresholds: Option<Vec<(String, CoverageThreshold)>>
}

pub fn read_params() -> Result<Params, std::io::Error>{
    // initialize command line parser & parse command line arguments
    let args = Command::parse();

    if args.threads > 0 {
        log::info!("running pangenome-growth on {} threads", &args.threads);
        rayon::ThreadPoolBuilder::new()
            .num_threads(args.threads)
            .build_global()
            .unwrap();
    } else {
        log::info!("running pangenome-growth using all available CPUs");
    }

    let mut subset_coords = None;
    if !args.positive_list.is_empty() {
        log::info!("loading subset coordinates from {}", &args.positive_list);
        let mut data = std::io::BufReader::new(fs::File::open(&args.positive_list)?);
        subset_coords = Some(io::parse_bed(&mut data));
        log::debug!( "loaded {} coordinates", subset_coords.as_ref().unwrap().len());
    }

    let mut exclude_coords = None;
    if !args.negative_list.is_empty() {
        log::info!(
            "loading exclusion coordinates from {}",
            &args.negative_list
        );
        let mut data = std::io::BufReader::new(fs::File::open(&args.negative_list)?);
        exclude_coords = Some(io::parse_bed(&mut data));
        log::debug!("loaded {} coordinates", exclude_coords.as_ref().unwrap().len());
    }

    let mut groups = None;
    if !args.groups.is_empty() {
        log::info!("loading groups from {}", &args.groups);
        let mut data = std::io::BufReader::new(fs::File::open(&args.groups)?);
        groups = Some(io::parse_groups(&mut data));
        log::debug!( "loaded {} group assignments ", groups.as_ref().unwrap().len());
    }

    let mut coverage_thresholds = None;
    if !args.thresholds.is_empty() {
        if Path::new(&args.thresholds).exists() {
            log::info!("loading coverage thresholds from {}", &args.thresholds);
            let mut data = std::io::BufReader::new(fs::File::open(&args.groups)?);
            coverage_thresholds = Some(io::parse_coverage_threshold_file(&mut data));
        } else {
            coverage_thresholds = Some(parse_coverage_threshold_cli(&args));
        }
        log::debug!(
            "loaded {} coverage thresholds:\n{}",
            coverage_thresholds.as_ref().unwrap().len(),
            coverage_thresholds.as_ref().unwrap()
                .iter()
                .map(|(n, t)| format!("\t{}: {}", n, t))
                .collect::<Vec<String>>()
                .join("\n")
        );
    }

    Ok(Params {
        gfa_file: args.gfa_file,
        threads: args.threads,
        subset_coords: subset_coords,
        exclude_coords: exclude_coords,
        groups: groups,
        coverage_thresholds: coverage_thresholds
    })
}

pub fn parse_coverage_threshold_cli (args: &Command) -> Vec<(String, CoverageThreshold)> {
    let mut coverage_thresholds = Vec::new();

    let re = Regex::new(r"^\s?([!-<,>-~]+)\s?=\s?([!-<,>-~]+)\s*$").unwrap();
    for el in args.thresholds.split(',') {
        if let Some(t) = usize::from_str(el.trim()).ok() {
            coverage_thresholds
                .push((el.trim().to_string(), CoverageThreshold::Absolute(t)));
        } else if let Some(t) = f64::from_str(el.trim()).ok() {
            coverage_thresholds
                .push((el.trim().to_string(), CoverageThreshold::Relative(t)));
        } else if let Some(caps) = re.captures(&el) {
            let name = caps.get(1).unwrap().as_str().trim().to_string();
            let threshold_str = caps.get(2).unwrap().as_str();
            let threshold = if let Some(t) = usize::from_str(threshold_str).ok() {
                CoverageThreshold::Absolute(t)
            } else {
                CoverageThreshold::Relative(f64::from_str(threshold_str).unwrap())
            };
            coverage_thresholds.push((name, threshold));
        } else {
            panic!(
                "coverage threshold \"{}\" string is not well-formed",
                &args.thresholds
            );
        }
    }

    coverage_thresholds
}
