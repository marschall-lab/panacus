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

pub enum RequireThreshold {
    Absolute,
    Relative,
    Either,
}

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

#[macro_export]
macro_rules! clap_enum_variants_no_all {
    ($e: ty) => {{
        use clap::builder::TypedValueParser;
        clap::builder::PossibleValuesParser::new(<$e>::VARIANTS.iter().filter(|&x| x != &"all"))
            .map(|s| s.parse::<$e>().unwrap())
    }};
}

#[derive(Parser, Debug)]
#[clap(
    version = "0.2.1",
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
        default_value = "node",
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
            help = "Exclude bp/node/edge in growth count that intersect with paths (1-column list) or path coordinates (3- or 12-column BED-file) provided by the given file",
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
            short = 'H',
            long,
            help = "Merge counts from paths belonging to same haplotype"
        )]
        groupby_haplotype: bool,

        #[clap(
            short = 'S',
            long,
            help = "Merge counts from paths belonging to same sample"
        )]
        groupby_sample: bool,

        #[clap(
            short,
            long,
            help = "List of quorum fractions of the form <level1>,<level2>,... Number of values must be one or match that of coverage setting",
            default_value = "0"
        )]
        quorum: String,

        #[clap(
            short = 'l',
            long,
            help = "List of absolute coverage thresholds of the form <level1>,<level2>,... Number of values must be one or match that of quorum setting",
            default_value = "1"
        )]
        coverage: String,

        #[clap(short = 'a', long, help = "Include also histogram in output")]
        hist: bool,

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
        default_value = "node",
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
            help = "Exclude bp/node/edge in growth count that intersect with paths (1-column list) or path coordinates (3- or 12-column BED-file) provided by the given file; all intersecting bp/node/edge will be exluded also in other paths not part of the given list",
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
            short = 'H',
            long,
            help = "Merge counts from paths belonging to same haplotype"
        )]
        groupby_haplotype: bool,

        #[clap(
            short = 'S',
            long,
            help = "Merge counts from paths belonging to same sample"
        )]
        groupby_sample: bool,

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
            help = "List of quorum fractions of the form <level1>,<level2>,... Number of values must be one or match that of coverage setting",
            default_value = "0"
        )]
        quorum: String,

        #[clap(
            short = 'l',
            long,
            help = "List of absolute coverage thresholds of the form <level1>,<level2>,... Number of values must be one or match that of quorum setting",
            default_value = "1"
        )]
        coverage: String,

        #[clap(short = 'H', long, help = "Include histogram in output")]
        hist: bool,

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
    OrderedHistgrowth {
        #[clap(index = 1, help = "graph in GFA1 format", required = true)]
        gfa_file: String,

        #[clap(short, long,
        help = "Graph quantity to be counted",
        default_value = "node",
        ignore_case = true,
        value_parser = clap_enum_variants_no_all!(CountType),
    )]
        count: CountType,

        #[clap(
            name = "order",
            short,
            long,
            help = "The ordered histogram will be produced according to order of paths/groups in the supplied file (1-column list). If this option is not used, the order is determined by the rank of paths/groups in the subset list, and if that option is not used, the order is determined by the rank of paths/groups in the GFA file.",
            default_value = ""
        )]
        order: String,

        #[clap(
            name = "subset",
            short,
            long,
            help = "Produce counts by subsetting the graph to a given list of paths (1-column list) or path coordinates (3- or 12-column BED file). If the \"order\" option is not used, the subset list will also indicate the order of paths/groups in the histogram.",
            default_value = ""
        )]
        positive_list: String,

        #[clap(
            name = "exclude",
            short,
            long,
            help = "Exclude bp/node/edge in growth count that intersect with paths (1-column list) or path coordinates (3- or 12-column BED-file) provided by the given file",
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
            short = 'H',
            long,
            help = "Merge counts from paths belonging to same haplotype"
        )]
        groupby_haplotype: bool,

        #[clap(
            short = 'S',
            long,
            help = "Merge counts from paths belonging to same sample"
        )]
        groupby_sample: bool,

        #[clap(
            short,
            long,
            help = "List of quorum fractions of the form <level1>,<level2>,... Number of values must be one or match that of coverage setting",
            default_value = "0"
        )]
        quorum: String,

        #[clap(
            short = 'l',
            long,
            help = "List of absolute coverage thresholds of the form <level1>,<level2>,... Number of values must be one or match that of quorum setting",
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

    #[clap(about = "Compute coverage table for count type")]
    Table {
        #[clap(index = 1, help = "graph in GFA1 format", required = true)]
        gfa_file: String,

        #[clap(short, long,
            help = "Graph quantity to be counted",
            default_value = "node",
            ignore_case = true,
            value_parser = clap_enum_variants_no_all!(CountType),
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
            help = "Exclude bp/node/edge in growth count that intersect with paths (1-column list) or path coordinates (3- or 12-column BED-file) provided by the given file",
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
            short = 'H',
            long,
            help = "Merge counts from paths belonging to same haplotype"
        )]
        groupby_haplotype: bool,

        #[clap(
            short = 'S',
            long,
            help = "Merge counts from paths belonging to same sample"
        )]
        groupby_sample: bool,

        #[clap(
            short,
            long,
            help = "Run in parallel on N threads",
            default_value = "1"
        )]
        threads: usize,
    },
}

pub fn parse_threshold_cli(
    threshold_str: &str,
    require: RequireThreshold,
) -> Result<Vec<Threshold>, std::io::Error> {
    let mut thresholds = Vec::new();

    for (i, el) in threshold_str.split(',').enumerate() {
        let rel_val = match f64::from_str(el.trim()) {
            Ok(t) => {
                if 0.0 <= t && t <= 1.0 {
                    Ok(t)
                } else {
                    Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        format!(
                            "relative threshold \"{}\" ({}. element in list) must be within [0,1].",
                            &threshold_str,
                            i + 1
                        ),
                    ))
                }
            }
            Err(_) => Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!(
                    "threshold \"{}\" ({}. element in list) is required to be float, but isn't.",
                    &threshold_str,
                    i + 1
                ),
            )),
        };

        thresholds.push(
            match require {
                RequireThreshold::Absolute => Threshold::Absolute(usize::from_str(el.trim()).map_err(|_|
                    std::io::Error::new(
                            std::io::ErrorKind::InvalidData,
                            format!("threshold \"{}\" ({}. element in list) is required to be integer, but isn't.",
                    &threshold_str,
                    i + 1)))?),
            RequireThreshold::Relative => Threshold::Relative(rel_val?),
            RequireThreshold::Either =>
        if let Some(t) = usize::from_str(el.trim()).ok() {
            Threshold::Absolute(t)
        } else {
            Threshold::Relative(rel_val?)
            }
            }
            );
    }
    Ok(thresholds)
}

pub fn write_ordered_histgrowth_table<W: Write>(
    abacus_group: &AbacusByGroup,
    hist_aux: &HistAuxilliary,
    out: &mut BufWriter<W>,
) -> Result<(), std::io::Error> {
    let mut output_columns: Vec<Vec<f64>> = hist_aux
        .coverage
        .par_iter()
        .zip(&hist_aux.quorum)
        .map(|(c, q)| {
            log::info!(
                "calculating ordered growth for coverage >= {} and quorum >= {}",
                &c,
                &q
            );
            abacus_group.calc_growth(&c, &q)
        })
        .collect();

    // insert empty row for 0 element
    for c in &mut output_columns {
        c.insert(0, std::f64::NAN);
    }
    let m = hist_aux.coverage.len();
    let mut header_cols = vec![vec![
        "panacus".to_string(),
        "count".to_string(),
        "coverage".to_string(),
        "quorum".to_string(),
    ]];
    header_cols.extend(
        std::iter::repeat("ordered-growth")
            .take(m)
            .zip(std::iter::repeat(abacus_group.count).take(m))
            .zip(hist_aux.coverage.iter())
            .zip(&hist_aux.quorum)
            .map(|(((p, t), c), q)| {
                vec![p.to_string(), t.to_string(), c.to_string(), q.to_string()]
            })
            .collect::<Vec<Vec<String>>>(),
    );
    write_table(&header_cols, &output_columns, out)
}

pub fn write_table<W: Write>(
    headers: &Vec<Vec<String>>,
    columns: &Vec<Vec<f64>>,
    out: &mut BufWriter<W>,
) -> Result<(), std::io::Error> {
    let n = headers.first().unwrap_or(&Vec::new()).len();

    for i in 0..n {
        for j in 0..headers.len() {
            if j > 0 {
                write!(out, "\t")?;
            }
            write!(out, "{:0}", headers[j][i])?;
        }
        writeln!(out, "")?;
    }
    let n = columns.first().unwrap_or(&Vec::new()).len();
    for i in 0..n {
        write!(out, "{}", i)?;
        for j in 0..columns.len() {
            write!(out, "\t{:0}", columns[j][i].floor())?;
        }
        writeln!(out, "")?;
    }

    Ok(())
}

pub fn read_params() -> Params {
    Command::parse().cmd
}

pub fn run<W: Write>(params: Params, out: &mut BufWriter<W>) -> Result<(), std::io::Error> {
    // set the number of threads used in parallel computation
    if let Params::Histgrowth { threads, .. }
    | Params::Hist { threads, .. }
    | Params::OrderedHistgrowth { threads, .. }
    | Params::Table { threads, .. } = params
    {
        if threads > 0 {
            log::info!("running panacus on {} threads", &threads);
            rayon::ThreadPoolBuilder::new()
                .num_threads(threads)
                .build_global()
                .unwrap();
        } else {
            log::info!("running panacus using all available CPUs");
            rayon::ThreadPoolBuilder::new().build_global().unwrap();
        }
    }

    // make sure either group, groupby-sample, or groupby-haplotype is set
    if let Params::Histgrowth {
        groupby,
        groupby_haplotype,
        groupby_sample,
        ..
    }
    | Params::Hist {
        groupby,
        groupby_haplotype,
        groupby_sample,
        ..
    }
    | Params::OrderedHistgrowth {
        groupby,
        groupby_haplotype,
        groupby_sample,
        ..
    }
    | Params::Table {
        groupby,
        groupby_haplotype,
        groupby_sample,
        ..
    } = &params
    {
        let mut c = 0;
        if !groupby.is_empty() {
            c += 1;
        }
        if *groupby_haplotype {
            c += 1;
        }
        if *groupby_sample {
            c += 1
        }
        if c > 1 {
            let msg = "At most one option of groupby, groupby-haplotype, and groupby-sample can be set at once, but at least two are given.";
            log::error!("{}", &msg);
            return Err(std::io::Error::new(std::io::ErrorKind::InvalidInput, msg));
        }
    }

    //
    // 1st step: loading data from group / subset / exclude files and indexing graph
    //
    //
    // graph_aux and abacus_aux do not make use of count type information, so they don't need to be
    // adjusted
    let (graph_aux, abacus_aux) = match &params {
        Params::Histgrowth {
            gfa_file, count, ..
        }
        | Params::Hist {
            gfa_file, count, ..
        }
        | Params::OrderedHistgrowth {
            gfa_file, count, ..
        }
        | Params::Table {
            gfa_file, count, ..
        } => {
            log::info!("constructing indexes for node/edge IDs, node lengths, and P/W lines..");
            let mut data = std::io::BufReader::new(fs::File::open(&gfa_file)?);
            let graph_aux = GraphAuxilliary::from_gfa(
                &mut data,
                (count == &CountType::Edge) | (count == &CountType::All),
            )?;
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

    let mut abaci: Vec<Abacus> = match &params {
        Params::Histgrowth {
            gfa_file, count, ..
        }
        | Params::Hist {
            gfa_file, count, ..
        } => {
            // creating the abacus from the gfa

            let n_groups = abacus_aux.as_ref().unwrap().count_groups();
            if n_groups > 65534 {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::Unsupported,
                    format!(
                        "data has {} path groups, but command is not supported for more than 65534",
                        n_groups
                    ),
                ));
            }

            let mut abaci = Vec::new();
            if matches!(count, CountType::All | CountType::Node | CountType::Bp) {
                // unless we specifically count only nodes, let's ignore bps stuff...
                let mycount = match count {
                    CountType::Node => CountType::Node,
                    _ => CountType::Bp,
                };

                let mut data = std::io::BufReader::new(fs::File::open(&gfa_file)?);
                log::info!("loading graph from {}", &gfa_file);
                let abacus = AbacusByTotal::from_gfa(
                    &mut data,
                    abacus_aux.as_ref().unwrap(),
                    graph_aux.as_ref().unwrap(),
                    mycount,
                )?;
                log::info!(
                    "abacus has {} path groups and {} countables",
                    abacus.groups.len(),
                    abacus.countable.len()
                );
                abaci.push(Abacus::Total(abacus));
            }
            if matches!(count, CountType::All | CountType::Edge) {
                let mut data = std::io::BufReader::new(fs::File::open(&gfa_file)?);
                log::info!("loading graph from {}", &gfa_file);
                let abacus = AbacusByTotal::from_gfa(
                    &mut data,
                    abacus_aux.as_ref().unwrap(),
                    graph_aux.as_ref().unwrap(),
                    CountType::Edge,
                )?;
                log::info!(
                    "abacus has {} path groups and {} countables",
                    abacus.groups.len(),
                    abacus.countable.len()
                );
                abaci.push(Abacus::Total(abacus));
            }
            abaci
        }
        Params::Table {
            gfa_file, count, ..
        }
        | Params::OrderedHistgrowth {
            gfa_file, count, ..
        } => {
            log::info!("loading graph from {}", &gfa_file);
            let mut data = std::io::BufReader::new(fs::File::open(&gfa_file)?);
            let abacus = AbacusByGroup::from_gfa(
                &mut data,
                abacus_aux.as_ref().unwrap(),
                graph_aux.as_ref().unwrap(),
                *count,
                if let Params::Table { total, .. } = params {
                    !total
                } else {
                    false
                },
            )?;
            log::info!(
                "abacus has {} path groups and {} countables",
                abacus.groups.len(),
                abacus.r.len()
            );
            vec![Abacus::Group(abacus)]
        }
        _ => vec![Abacus::Nil],
    };

    //
    // 3rd step: build histograam
    //

    let hists: Option<Vec<Hist>> = match &params {
        Params::Histgrowth { count, .. } | Params::Hist { count, .. } => {
            let mut hists = Vec::new();

            if matches!(count, CountType::All) {
                // by construction, node/bp abacus comes first, then edge abacus
                if let Some(Abacus::Total(ref mut abacus_total)) = abaci.first_mut() {
                    // constructing histogram
                    log::info!("constructing bp histogram..");
                    hists.push(Hist::from_abacus(&abacus_total));
                    log::info!("constructing node histogram..");
                    abacus_total.count = CountType::Node;
                    hists.push(Hist::from_abacus(&abacus_total));
                    // revert back
                    abacus_total.count = CountType::Bp;
                }
            }
            if let Some(Abacus::Total(abacus_total)) = &abaci.last() {
                // constructing histogram
                log::info!("constructing histogram..");
                hists.push(Hist::from_abacus(abacus_total));
            }
            Some(hists)
        }
        Params::Growth { hist_file, .. } => {
            log::info!("loading coverage histogram from {}", hist_file);
            let mut data = std::io::BufReader::new(fs::File::open(&hist_file)?);
            Some(vec![Hist::from_tsv(&mut data)?])
        }
        Params::OrderedHistgrowth { .. } | Params::Table { .. } => {
            // do nothing
            None
        }
    };

    //
    // 4th step: calculation & output of growth curve / output of histogram
    //
    //
    writeln!(
        out,
        "# {}",
        std::env::args().collect::<Vec<String>>().join(" ")
    )?;

    //    if let Abacus::Group(abacus_group) = &abacus {
    //        abacus_group.write_rcv(out)?;
    //        out.flush()?;
    //        std::process::exit(0x0100);
    //    }

    match params {
        Params::OrderedHistgrowth { .. } => {
            let hist_aux = HistAuxilliary::from_params(&params)?;
            match &abaci.last() {
                Some(Abacus::Group(abacus_group)) => {
                    write_ordered_histgrowth_table(abacus_group, &hist_aux, out)?;
                }
                _ => unreachable!(),
            }
        }
        Params::Histgrowth { hist, .. } | Params::Growth { hist, .. } => {
            let hist_aux = HistAuxilliary::from_params(&params)?;
            if let Some(hs) = hists {
                let mut header_cols = vec![vec![
                    "panacus".to_string(),
                    "count".to_string(),
                    "coverage".to_string(),
                    "quorum".to_string(),
                ]];
                let mut output_columns = Vec::new();

                if hist {
                    for h in hs.iter() {
                        output_columns.push(h.coverage.iter().map(|x| *x as f64).collect());
                        header_cols.push(vec![
                            "hist".to_string(),
                            h.count.to_string(),
                            String::new(),
                            String::new(),
                        ])
                    }
                }

                for h in hs.iter() {
                    let mut columns: Vec<Vec<f64>> = hist_aux
                        .coverage
                        .par_iter()
                        .zip(&hist_aux.quorum)
                        .map(|(c, q)| {
                            log::info!(
                                "calculating growth for coverage >= {} and quorum >= {}",
                                &c,
                                &q
                            );
                            h.calc_growth(&c, &q)
                        })
                        .collect();
                    // insert empty row for 0 element
                    for c in &mut columns {
                        c.insert(0, std::f64::NAN);
                    }
                    output_columns.extend(columns);

                    let m = hist_aux.coverage.len();
                    header_cols.extend(
                        std::iter::repeat("growth")
                            .take(m)
                            .zip(std::iter::repeat(h.count).take(m))
                            .zip(hist_aux.coverage.iter())
                            .zip(&hist_aux.quorum)
                            .map(|(((p, t), c), q)| {
                                vec![p.to_string(), t.to_string(), c.to_string(), q.to_string()]
                            }),
                    );
                }
                write_table(&header_cols, &output_columns, out)?;
            }
        }
        Params::Hist { .. } => {
            if let Some(hs) = hists {
                let mut header_cols = vec![vec!["panacus".to_string(), "count".to_string()]];
                let mut output_columns = Vec::new();
                for h in hs.iter() {
                    output_columns.push(h.coverage.iter().map(|x| *x as f64).collect());
                    header_cols.push(vec!["hist".to_string(), h.count.to_string()])
                }
                write_table(&header_cols, &output_columns, out)?;
            }
        }
        Params::Table { total, .. } => {
            if let Some(Abacus::Group(abacus_group)) = abaci.last() {
                log::info!("reporting coverage table");
                abacus_group.to_tsv(total, out)?;
            }
        }
    };

    Ok(())
}
