/* standard crate */
use std::fs;
use std::io::{BufWriter, Write};
use std::str::FromStr;

/* external crate */
use clap::{Parser, Subcommand};
use rayon::prelude::*;
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
        help = "Graph quantity to be counted",
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
            default_value = "1"
        )]
        intersection: String,

        #[clap(
            short = 'l',
            long,
            help = "List of (named) coverage thresholds of the form <level1>,<level2>,.. or <name1>=<level1>,<name2>=<level2> or a file that provides these levels in a tab-separated format; a level is absolute, i.e., corresponds to a number of paths/groups IFF it is integer, otherwise it is a float value representing a percentage of paths/groups.",
            default_value = "1"
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
        help = "Graph quantity to be counted",
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
            help = "List of intersection thresholds of the form <level1>,<level2>,.. or a file that provides these levels (one per line); a level is absolute, i.e., corresponds to a number of paths/groups IFF it is integer, otherwise it is a float value (must contain a \".\") representing a percentage of paths/groups. The list must have the same length as the intersection list, or contain only a single entry (which is then used in for all coverage settings).",
            default_value = "1"
        )]
        intersection: String,

        #[clap(
            short = 'l',
            long,
            help = "List of coverage thresholds of the form <level1>,<level2>,.. or a file that provides these levels in a tab-separated format; a level is absolute, i.e., corresponds to a number of paths/groups IFF it is integer, otherwise it is a float (must contain a \".\") value representing a percentage of paths/groups. The list must have the same length as the coverage list, or contain only a single entry (which is then used in for all intersection settings).",
            default_value = "1"
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

    #[clap(about = "Compute coverage table for count items")]
    Table {
        #[clap(index = 1, help = "graph in GFA1 format", required = true)]
        gfa_file: String,

        #[clap(short, long,
            help = "Graph quantity to be counted",
            default_value = "nodes",
            ignore_case = true,
            value_parser = clap_enum_variants!(CountType),
        )]
        count: CountType,

        #[clap(
            name = "total",
            short = 'a',
            long,
            help = "Summarize by totaling presence/absence over all groups"
        )]
        total: bool,

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
            help = "Run in parallel on N threads",
            default_value = "1"
        )]
        threads: usize,
    },
}

pub fn parse_threshold_cli(threshold_str: &str) -> Result<Vec<Threshold>, std::io::Error> {
    let mut thresholds = Vec::new();

    for (i, el) in threshold_str.split(',').enumerate() {
        if let Some(t) = usize::from_str(el.trim()).ok() {
            thresholds.push(Threshold::Absolute(t));
        } else if let Some(t) = f64::from_str(el.trim()).ok() {
            thresholds.push(Threshold::Relative(t));
        } else {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                &format!(
                    "threshold \"{}\" ({}. element in list) is neither an integer nor a float",
                    &threshold_str,
                    i + 1
                )[..],
            ));
        }
    }
    Ok(thresholds)
}

pub fn read_params() -> Params {
    Command::parse().cmd
}

pub fn run<W: Write>(params: Params, out: &mut BufWriter<W>) -> Result<(), std::io::Error> {
    // check if in case of coverage, count is not bp
    if let Params::Table { count, .. } = params {
        if count == CountType::Bps {
            log::error!("count type \"bps\" is not available for coverage command");
            return Ok(());
        }
    }

    // set the number of threads used in parallel computation
    if let Params::Histgrowth { threads, .. }
    | Params::Hist { threads, .. }
    | Params::Table { threads, .. } = params
    {
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
    let (graph_aux, abacus_aux) = match &params {
        Params::Histgrowth {
            gfa_file, count, ..
        }
        | Params::Hist {
            gfa_file, count, ..
        }
        | Params::Table {
            gfa_file, count, ..
        } => {
            log::info!("constructing indexes for node/edge IDs, node lengths, and P/W lines..");
            let mut data = std::io::BufReader::new(fs::File::open(&gfa_file)?);
            let graph_aux = GraphAuxilliary::from_gfa(&mut data, count == &CountType::Edges)?;
            log::info!(
                "..done; found {} paths/walks and {} nodes{}",
                graph_aux.path_segments.len(),
                graph_aux.node2id.len(),
                if let Some(edge2id) = &graph_aux.edge2id {
                    format!(" {} edges", edge2id.len())
                } else {
                    String::new()
                }
            );

            if graph_aux.path_segments.len() == 0 {
                log::error!("there's nothing to do--graph does not contain any annotated paths (P/W lines), exiting");
                return Ok(());
            }

            log::info!("loading data from group / subset / exclude files");
            let abacus_aux = AbacusAuxilliary::from_params(&params, &graph_aux)?;

            (Some(graph_aux), Some(abacus_aux))
        }
        _ => (None, None),
    };

    //
    // 2nd step: build abacus or calculate coverage table
    //

    let abacus = match &params {
        Params::Histgrowth { gfa_file, .. } | Params::Hist { gfa_file, .. } => {
            // creating the abacus from the gfa
            log::info!("loading graph from {}", &gfa_file);
            let mut data = std::io::BufReader::new(fs::File::open(&gfa_file)?);
            let abacus = Abacus::from_gfa(&mut data, abacus_aux.unwrap(), graph_aux.unwrap());
            log::info!(
                "abacus has {} path groups and {} countables",
                abacus.groups.len(),
                abacus.countable.len()
            );
            Some(abacus)
        }
        Params::Table {
            gfa_file,
            count,
            total,
            ..
        } => {
            let graph_aux = graph_aux.as_ref().unwrap();
            let abacus_aux = abacus_aux.as_ref().unwrap();

            log::info!("parsing path + walk sequences");
            let mut data = std::io::BufReader::new(fs::File::open(&gfa_file)?);
            let (table, groups) = Abacus::get_coverage_table(&mut data, abacus_aux, graph_aux);

            log::info!("reporting coverage table");
            // create mapping from numerical node ids to original node identifiers
            let dummy = Vec::new();
            let mut id2node: Vec<&Vec<u8>> = vec![&dummy; graph_aux.node2id.len() + 1];
            graph_aux
                .node2id
                .iter()
                .for_each(|(node, id)| id2node[id.0 as usize] = node);

            if count == &CountType::Nodes {
                write!(out, "node")?;
                if *total {
                    write!(out, "\ttotal")?;
                } else {
                    for group in &groups {
                        write!(out, "\t{}", group)?;
                    }
                }
                writeln!(out, "")?;

                for (i, node) in id2node[1..].iter().enumerate() {
                    write!(out, "{}", std::str::from_utf8(node).unwrap())?;
                    let mut c = 0;
                    if *total {
                        table[i + 1].iter().for_each(|x| {
                            if x > &0 {
                                c += 1
                            }
                        });
                        writeln!(out, "\t{}", c)?;
                    } else {
                        for j in 0..groups.len() {
                            write!(out, "\t{}", table[i + 1][j])?;
                        }
                        writeln!(out, "")?;
                    }
                }
            }
            if count == &CountType::Edges {
                if let Some(ref edge2id) = graph_aux.edge2id {
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
                    if *total {
                        write!(out, "\ttotal")?;
                    } else {
                        for group in &groups {
                            write!(out, "\t{}", group)?;
                        }
                    }
                    writeln!(out, "")?;

                    for (i, edge) in id2edge[1..].iter().enumerate() {
                        write!(
                            out,
                            "{}{}{}{}",
                            edge.1,
                            std::str::from_utf8(id2node[edge.0 .0 as usize]).unwrap(),
                            edge.3,
                            std::str::from_utf8(id2node[edge.2 .0 as usize]).unwrap(),
                        )?;
                        let mut c = 0;
                        if *total {
                            table[i + 1].iter().for_each(|x| {
                                if x > &0 {
                                    c += 1
                                }
                            });
                            writeln!(out, "\t{}", c)?;
                        } else {
                            for j in 0..groups.len() {
                                write!(out, "\t{}", table[i + 1][j])?;
                            }
                            writeln!(out, "")?;
                        }
                    }
                }
            }
            None
        }
        _ => None,
    };

    //
    // 3rd step: build histograam
    //

    let hist: Option<Hist> = match &params {
        Params::Histgrowth { .. } | Params::Hist { .. } => {
            // constructing histogram
            log::info!("constructing histogram..");
            Some(Hist::from_abacus(abacus.as_ref().unwrap()))
        }
        Params::Growth { hist_file, .. } => {
            log::info!("loading coverage histogram from {}", hist_file);
            let mut data = std::io::BufReader::new(fs::File::open(&hist_file)?);
            Some(Hist::from_tsv(&mut data)?)
        }
        Params::OrderedHistgrowth => {
            // XXX
            Some(Hist {
                coverage: Vec::new(),
            })
        }
        Params::Table { .. } => {
            // do nothing
            None
        }
    };

    //
    // 4th step: calculation & output of growth curve / output of histogram
    //
    match params {
        Params::Histgrowth { .. } | Params::Growth { .. } => {
            // <hist> must be some-thing in histgrowth and growth, so let's unwrap it!
            let hist = hist.unwrap();
            let hist_aux = HistAuxilliary::from_params(&params)?;

            let growths: Vec<Vec<usize>> = hist_aux
                .coverage
                .par_iter()
                .zip(&hist_aux.intersection)
                .map(|(c, i)| hist.calc_growth(&c, &i))
                .collect();

            writeln!(
                out,
                "coverage\t{}",
                hist_aux
                    .coverage
                    .iter()
                    .map(|x| x.to_string())
                    .collect::<Vec<String>>()
                    .join("\t")
            )?;
            writeln!(
                out,
                "intersection\t{}",
                hist_aux
                    .intersection
                    .iter()
                    .map(|x| x.to_string())
                    .collect::<Vec<String>>()
                    .join("\t")
            )?;
            for i in 0..hist.coverage.len() - 1 {
                write!(out, "{}", i + 1)?;
                for j in 0..hist_aux.intersection.len() {
                    write!(out, "\t{}", growths[j][i])?;
                }
                writeln!(out, "")?;
            }
        }
        Params::Hist { count, .. } => {
            hist.unwrap().to_tsv(&count, out)?;
        }
        Params::OrderedHistgrowth => {
            unreachable!();
        }
        Params::Table { .. } => {
            // do nothing
            ()
        }
    };

    Ok(())
}
