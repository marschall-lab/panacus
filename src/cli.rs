/* standard crate */
use std::fs;
use std::io::{BufWriter, Write};
use std::str::FromStr;

/* external crate */
use clap::{Parser, Subcommand};
use regex::Regex;
use strum::VariantNames;

/* private use */
use crate::abacus::*;
use crate::graph::*;
use crate::hist::*;
use crate::util::*;

//
// Credit: Johan Andersson (https://github.com/repi)
// Code from https://github.com/clap-rs/clap/discussions/4264
//
#[macro_export]
macro_rules! clap_enum_variants {
    ($e: ty) => {{
        use clap::builder::TypedValueParser;
        clap::builder::PossibleValuesParser::new(<$e>::VARIANTS).map(|s| s.parse::<$e>().unwrap())
    }};
}




#[derive(Parser, Debug)]
#[clap(
    version = "0.2",
    author = "Luca Parmigiani <lparmig@cebitec.uni-bielefeld.de>, Daniel Doerr <daniel.doerr@hhu.de>",
    about = "Calculate count statistics for pangenomic data"
)]

struct Command {
    #[clap(subcommand)]
    cmd: Params,
}

#[derive(Subcommand, Debug)]
pub enum Params {
    #[clap(
        alias = "hg",
        about = "Run in default mode, i.e., run hist and growth successively and output the results of the latter"
    )]
    Histgrowth {
        #[clap(index = 1, help = "graph in GFA1 format", required = true)]
        gfa_file: String,

        #[clap(short, long,
        help = "Count type: node, basepair (bp) or edge count",
        default_value = "nodes",
        ignore_case = true,
        value_parser = clap_enum_variants!(CountType),
    )]
        count: CountType,

        #[clap(
            name = "subset",
            short,
            long,
            help = "Produce counts by subsetting the graph to a given list of paths (1-column list) or path coordinates (3- or 12-column BED file)",
            default_value = ""
        )]
        positive_list: String,

        #[clap(
            name = "exclude",
            short,
            long,
            help = "Exclude bps/nodes/edges in growth count that intersect with paths (1-column list) or path coordinates (3- or 12-column BED-file) provided by the given file",
            default_value = ""
        )]
        negative_list: String,

        #[clap(
            short,
            long,
            help = "Merge counts from paths by path-group mapping from given tab-separated two-column file",
            default_value = ""
        )]
        groupby: String,

        #[clap(
            short,
            long,
            help = "List of (named) intersection thresholds of the form <level1>,<level2>,.. or <name1>=<level1>,<name2>=<level2> or a file that provides these levels in a tab-separated format; a level is absolute, i.e., corresponds to a number of paths/groups IFF it is integer, otherwise it is a float value representing a percentage of paths/groups.",
            default_value = "cumulative_count=1"
        )]
        intersection: String,

        #[clap(
            short = 'l',
            long,
            help = "List of (named) coverage thresholds of the form <level1>,<level2>,.. or <name1>=<level1>,<name2>=<level2> or a file that provides these levels in a tab-separated format; a level is absolute, i.e., corresponds to a number of paths/groups IFF it is integer, otherwise it is a float value representing a percentage of paths/groups.",
            default_value = "cumulative_count=1"
        )]
        coverage: String,

        #[clap(
            short,
            long,
            help = "Run in parallel on N threads",
            default_value = "1"
        )]
        threads: usize,
    },

    #[clap(alias = "h", about = "Calculate coverage histogram from GFA file")]
    Hist {
        #[clap(index = 1, help = "graph in GFA1 format", required = true)]
        gfa_file: String,

        #[clap(short, long,
        help = "Count type: node, basepair (bp), or edge count",
        default_value = "nodes",
        ignore_case = true,
        value_parser = clap_enum_variants!(CountType),
    )]
        count: CountType,

        #[clap(
            name = "subset",
            short,
            long,
            help = "Produce counts by subsetting the graph to a given list of paths (1-column list) or path coordinates (3- or 12-column BED file)",
            default_value = ""
        )]
        positive_list: String,

        #[clap(
            name = "exclude",
            short,
            long,
            help = "Exclude bps/nodes/edges in growth count that intersect with paths (1-column list) or path coordinates (3- or 12-column BED-file) provided by the given file; all intersecting bps/nodes/edges will be exluded also in other paths not part of the given list",
            default_value = ""
        )]
        negative_list: String,

        #[clap(
            short,
            long,
            help = "Merge counts from paths by path-group mapping from given tab-separated two-column file",
            default_value = ""
        )]
        groupby: String,

        #[clap(
            short,
            long,
            help = "Run in parallel on N threads",
            default_value = "1"
        )]
        threads: usize,
    },

    #[clap(alias = "g", about = "Construct growth table from coverage histogram")]
    Growth {
        #[clap(
            index = 1,
            help = "Coverage histogram as tab-separated value (tsv) file",
            required = true
        )]
        hist_file: String,

        #[clap(
            short,
            long,
            help = "List of (named) intersection thresholds of the form <level1>,<level2>,.. or <name1>=<level1>,<name2>=<level2> or a file that provides these levels in a tab-separated format; a level is absolute, i.e., corresponds to a number of paths/groups IFF it is integer, otherwise it is a float value representing a percentage of paths/groups.",
            default_value = "cumulative_count=1"
        )]
        intersection: String,

        #[clap(
            short = 'l',
            long,
            help = "List of (named) coverage thresholds of the form <level1>,<level2>,.. or <name1>=<level1>,<name2>=<level2> or a file that provides these levels in a tab-separated format; a level is absolute, i.e., corresponds to a number of paths/groups IFF it is integer, otherwise it is a float value representing a percentage of paths/groups.",
            default_value = "cumulative_count=1"
        )]
        coverage: String,

        #[clap(
            short,
            long,
            help = "Run in parallel on N threads",
            default_value = "1"
        )]
        threads: usize,
    },

    #[clap(
        alias = "o",
        about = "Compute growth table for order specified in grouping file (or, if non specified, the order of paths in the GFA file)"
    )]
    OrderedHistgrowth,
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

pub fn read_params() -> Params {
    Command::parse().cmd
}

pub fn run<W: Write>(params: Params, out: &mut BufWriter<W>) -> Result<(), std::io::Error> {
    // set the number of threads used in parallel computation
    if let Params::Histgrowth { threads, .. } | Params::Hist { threads, .. } = params {
        if threads > 0 {
            log::info!("running pangenome-growth on {} threads", &threads);
            rayon::ThreadPoolBuilder::new()
                .num_threads(threads)
                .build_global()
                .unwrap();
        } else {
            log::info!("running pangenome-growth using all available CPUs");
        }
    }

    //
    // 1st step: loading data from group / subset / exclude files and indexing graph
    //
    //
    let (graph_marginals, abacus_data) = match &params {
        Params::Histgrowth {
            gfa_file, count, ..
        }
        | Params::Hist {
            gfa_file, count, ..
        } => {
            log::info!("constructing indexes for node/edge IDs, node lengths, and P/W lines..");
            let mut data = std::io::BufReader::new(fs::File::open(&gfa_file)?);
            let graph_marginals =
                GraphData::from_gfa(&mut data, count == &CountType::Edges);
            log::info!(
                "..done; found {} paths/walks and {} nodes{}",
                graph_marginals.path_segments.len(),
                graph_marginals.node2id.len(),
                if let Some(edge2id) = &graph_marginals.edge2id {
                    format!(" edges {}", edge2id.len())
                } else {
                    String::new()
                }
            );

            if graph_marginals.path_segments.len() == 0 {
                log::error!("there's nothing to do--graph does not contain any annotated paths (P/W lines), exiting");
                return Ok(());
            }

            log::info!("loading data from group / subset / exclude files");
            let abacus_data = AbacusData::from_params(&params, &graph_marginals)?;

            (Some(graph_marginals), Some(abacus_data))
        }
        _ => (None, None),
    };

    //
    // 2nd step: build abacus
    //

    let abacus = match &params {
        Params::Histgrowth { gfa_file, .. } | Params::Hist { gfa_file, .. } => {
            // creating the abacus from the gfa
            log::info!("loading graph from {}", &gfa_file);
            let mut data = std::io::BufReader::new(fs::File::open(&gfa_file)?);
            let abacus =
                Abacus::from_gfa(&mut data, abacus_data.unwrap(), graph_marginals.unwrap());
            log::info!(
                "abacus has {} path groups and {} countables",
                abacus.groups.len(),
                abacus.countable.len()
            );
            Some(abacus)
        }
        _ => None,
    };

    //
    // 3rd step: build histograam
    //

    let hist: Hist = match &params {
        Params::Histgrowth { .. } | Params::Hist { .. } => {
            // constructing histogram
            log::info!("constructing histogram..");
            Hist::from_abacus(&abacus.unwrap())
        }
        Params::Growth { hist_file, .. } => {
            log::info!("loading coverage histogram from {}", hist_file);
            let mut data = std::io::BufReader::new(fs::File::open(&hist_file)?);
            Hist::from_tsv(&mut data)
        }
        Params::OrderedHistgrowth => {
            // XXX
            Hist {
                coverage: Vec::new(),
            }
        }
    };

    //
    // 4th step: calculation & output of growth curve / output of histogram
    //
    match params {
        Params::Histgrowth { .. } | Params::Growth { .. } => {
            // XXX
            let hist_data = HistData::from_params(&params);

            let growth = hist.calc_growth();

            for (i, pang_m) in growth.into_iter().enumerate() {
                writeln!(out, "{}\t{}", i + 1, pang_m)?;
            }
        }
        Params::Hist { count, .. } => {
            hist.to_tsv(&count, out)?;
        }
        Params::OrderedHistgrowth => {
            unreachable!();
        }
    };

    Ok(())
}
