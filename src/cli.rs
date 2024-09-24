/* standard crate */
use std::fs;
use std::io::{BufReader, BufWriter, Write};
use std::io::{Error, ErrorKind};
use std::path::Path;
use std::str::FromStr;

/* external crate */
use clap::{crate_version, Parser, Subcommand};
use rayon::prelude::*;
use strum::VariantNames;

/* private use */
use crate::abacus::*;
use crate::graph::*;
use crate::hist::*;
use crate::html::*;
use crate::io::*;
use crate::util::*;

pub enum RequireThreshold {
    Absolute,
    Relative,
    #[allow(dead_code)]
    Either,
}

#[macro_export]
macro_rules! clap_enum_variants {
    // Credit: Johan Andersson (https://github.com/repi)
    // Code from https://github.com/clap-rs/clap/discussions/4264
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
    version = crate_version!(),
    author = "Luca Parmigiani <lparmig@cebitec.uni-bielefeld.de>, Daniel Doerr <daniel.doerr@hhu.de>",
    about = "Calculate count statistics for pangenomic data"
)]

struct Command {
    #[clap(subcommand)]
    cmd: Params,
}

#[derive(Subcommand, Debug)]
pub enum Params {
    #[clap(alias = "hg", about = "Run hist and growth. Return the growth curve")]
    Histgrowth {
        #[clap(
            index = 1,
            help = "graph in GFA1 format, accepts also compressed (.gz) file",
            required = true
        )]
        gfa_file: String,
        #[clap(short, long, help = "Graph quantity to be counted", default_value = "node", ignore_case = true, value_parser = clap_enum_variants!(CountType),)]
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
            short = 'l',
            long,
            help = "Ignore all countables with a coverage lower than the specified threshold. The coverage of a countable corresponds to the number of path/walk that contain it. Repeated appearances of a countable in the same path/walk are counted as one. You can pass a comma-separated list of coverage thresholds, each one will produce a separated growth curve (e.g., --coverage 2,3). Use --quorum to set a threshold in conjunction with each coverage (e.g., --quorum 0.5,0.9)",
            default_value = "1"
        )]
        coverage: String,
        #[clap(
            short,
            long,
            help = "Unlike the --coverage parameter, which specifies a minimum constant number of paths for all growth point m (1 <= m <= num_paths), --quorum adjust the threshold based on m. At each m, a countable is counted in the average growth if the countable is contained in at least floor(m*quorum) paths. Example: A quorum of 0.9 requires a countable to be in 90% of paths for each subset size m. At m=10, it must appear in at least 9 paths. At m=100, it must appear in at least 90 paths. A quorum of 1 (100%) requires presence in all paths of the subset, corresponding to the core. Default: 0, a countable counts if it is present in any path at each growth point. Specify multiple quorum values with a comma-separated list (e.g., --quorum 0.5,0.9). Use --coverage to set static path thresholds in conjunction with variable quorum percentages (e.g., --coverage 5,10).",
            default_value = "0"
        )]
        quorum: String,
        #[clap(short = 'a', long, help = "Also include histogram in output")]
        hist: bool,
        #[clap(short, long, help = "Choose output format: table (tab-separated-values) or html report", default_value = "table", ignore_case = true, value_parser = clap_enum_variants!(OutputFormat),)]
        output_format: OutputFormat,
        #[clap(
            short,
            long,
            help = "Run in parallel on N threads (0 for number of CPU cores)",
            default_value = "0"
        )]
        threads: usize,
    },

    #[clap(alias = "h", about = "Calculate coverage histogram")]
    Hist {
        #[clap(
            index = 1,
            help = "graph in GFA1 format, accepts also compressed (.gz) file",
            required = true
        )]
        gfa_file: String,
        #[clap(short, long, help = "Graph quantity to be counted", default_value = "node", ignore_case = true, value_parser = clap_enum_variants!(CountType),)]
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
        #[clap(short, long, help = "Choose output format: table (tab-separated-values) or html report", default_value = "table", ignore_case = true, value_parser = clap_enum_variants!(OutputFormat),)]
        output_format: OutputFormat,
        #[clap(
            short,
            long,
            help = "Run in parallel on N threads (0 for number of CPU cores)",
            default_value = "0"
        )]
        threads: usize,
    },

    #[clap(alias = "g", about = "Calculate growth curve from coverage histogram")]
    Growth {
        #[clap(
            index = 1,
            help = "Coverage histogram as tab-separated value (tsv) file",
            required = true
        )]
        hist_file: String,
        #[clap(
            short = 'l',
            long,
            help = "Ignore all countables with a coverage lower than the specified threshold. The coverage of a countable corresponds to the number of path/walk that contain it. Repeated appearances of a countable in the same path/walk are counted as one. You can pass a comma-separated list of coverage thresholds, each one will produce a separated growth curve (e.g., --coverage 2,3). Use --quorum to set a threshold in conjunction with each coverage (e.g., --quorum 0.5,0.9)",
            default_value = "1"
        )]
        coverage: String,
        #[clap(
            short,
            long,
            help = "Unlike the --coverage parameter, which specifies a minimum constant number of paths for all growth point m (1 <= m <= num_paths), --quorum adjust the threshold based on m. At each m, a countable is counted in the average growth if the countable is contained in at least floor(m*quorum) paths. Example: A quorum of 0.9 requires a countable to be in 90% of paths for each subset size m. At m=10, it must appear in at least 9 paths. At m=100, it must appear in at least 90 paths. A quorum of 1 (100%) requires presence in all paths of the subset, corresponding to the core. Default: 0, a countable counts if it is present in any path at each growth point. Specify multiple quorum values with a comma-separated list (e.g., --quorum 0.5,0.9). Use --coverage to set static path thresholds in conjunction with variable quorum percentages (e.g., --coverage 5,10).",
            default_value = "0"
        )]
        quorum: String,
        #[clap(short = 'a', long, help = "Also include histogram in output")]
        hist: bool,
        #[clap(short, long, help = "Choose output format: table (tab-separated-values) or html report", default_value = "table", ignore_case = true, value_parser = clap_enum_variants!(OutputFormat),)]
        output_format: OutputFormat,
        #[clap(
            short,
            long,
            help = "Run in parallel on N threads (0 for number of CPU cores)",
            default_value = "0"
        )]
        threads: usize,
    },

    #[clap(alias = "I", about = "Return general graph and paths info")]
    Info {
        #[clap(
            index = 1,
            help = "graph in GFA1 format, accepts also compressed (.gz) file",
            required = true
        )]
        gfa_file: String,
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
        #[clap(short, long, help = "Choose output format: table (tab-separated-values) or html report", default_value = "table", ignore_case = true, value_parser = clap_enum_variants!(OutputFormat),)]
        output_format: OutputFormat,
        #[clap(
            short,
            long,
            help = "Run in parallel on N threads (0 for number of CPU cores)",
            default_value = "0"
        )]
        threads: usize,
    },

    #[clap(alias = "s", about = "Subsets the paths")]
    Subset {
        #[clap(
            short = 'q',
            long,
            help = "Report nodes only if present at least in flt_quorum_min groups",
            default_value = "0"
        )]
        flt_quorum_min: u32,
        #[clap(
            short = 'Q',
            long,
            help = "Report nodes only if present at most in flt_quorum_max groups",
            default_value = "4294967295"
        )]
        flt_quorum_max: u32,
        #[clap(
            short = 'l',
            long,
            help = "Report nodes only if their length is at least flt_length_min base pairs",
            default_value = "0"
        )]
        flt_length_min: u32,
        #[clap(
            short = 'L',
            long,
            help = "Report nodes only if their length is at most flt_length_max base pairs",
            default_value = "4294967295"
        )]
        flt_length_max: u32,
        #[clap(
            index = 1,
            help = "graph in GFA1 format, accepts also compressed (.gz) file",
            required = true
        )]
        gfa_file: String,
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
            help = "Run in parallel on N threads (0 for number of CPU cores)",
            default_value = "0"
        )]
        threads: usize,
    },

    #[clap(
        alias = "o",
        about = "Calculate growth curve based on group file order (if order is unspecified, use path order in GFA)"
    )]
    OrderedHistgrowth {
        #[clap(
            index = 1,
            help = "graph in GFA1 format, accepts also compressed (.gz) file",
            required = true
        )]
        gfa_file: String,
        #[clap(short, long, help = "Graph quantity to be counted", default_value = "node", ignore_case = true, value_parser = clap_enum_variants_no_all!(CountType),)]
        count: CountType,
        #[clap(
            name = "order",
            short = 'O',
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
        #[clap(short, long, help = "Choose output format: table (tab-separated-values) or html report", default_value = "table", ignore_case = true, value_parser = clap_enum_variants!(OutputFormat),)]
        output_format: OutputFormat,
        #[clap(
            short,
            long,
            help = "Run in parallel on N threads (0 for number of CPU cores)",
            default_value = "0"
        )]
        threads: usize,
    },

    #[clap(about = "Compute coverage table for count type")]
    Table {
        #[clap(
            index = 1,
            help = "graph in GFA1 format, accepts also compressed (.gz) file",
            required = true
        )]
        gfa_file: String,
        #[clap(short, long, help = "Graph quantity to be counted", default_value = "node", ignore_case = true, value_parser = clap_enum_variants_no_all!(CountType),)]
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
            help = "Run in parallel on N threads (0 for number of CPU cores)",
            default_value = "0"
        )]
        threads: usize,
    },
    //#[clap(
    //    alias = "C",
    //    about = "Calculate the histogram and growth of a Compacted de Bruijn Graph"
    //)]
    //Cdbg {
    //    #[clap(
    //        index = 1,
    //        help = "graph in GFA1 format, accepts also compressed (.gz) file representing a compacted de Bruijn graph",
    //        required = true
    //    )]
    //    gfa_file: String,
    //    #[clap(short, long, help = "Value of k for cdBG", default_value = "")]
    //    k: usize,
    //    #[clap(
    //        name = "subset",
    //        short,
    //        long,
    //        help = "Produce counts by subsetting the graph to a given list of paths (1-column list) or path coordinates (3- or 12-column BED file)",
    //        default_value = ""
    //    )]
    //    positive_list: String,
    //    #[clap(
    //        name = "exclude",
    //        short,
    //        long,
    //        help = "Exclude bp/node/edge in growth count that intersect with paths (1-column list) or path coordinates (3- or 12-column BED-file) provided by the given file",
    //        default_value = ""
    //    )]
    //    negative_list: String,
    //    #[clap(
    //        short,
    //        long,
    //        help = "Merge counts from paths by path-group mapping from given tab-separated two-column file",
    //        default_value = ""
    //    )]
    //    #[clap(
    //        short,
    //        long,
    //        help = "Merge counts from paths by path-group mapping from given tab-separated two-column file",
    //        default_value = ""
    //    )]
    //    groupby: String,
    //    #[clap(
    //        short = 'H',
    //        long,
    //        help = "Merge counts from paths belonging to same haplotype"
    //    )]
    //    groupby_haplotype: bool,
    //    #[clap(
    //        short = 'S',
    //        long,
    //        help = "Merge counts from paths belonging to same sample"
    //    )]
    //    groupby_sample: bool,
    //    #[clap(
    //        short,
    //        long,
    //        help = "Run in parallel on N threads (0 for number of CPU cores)",
    //        default_value = "0"
    //    )]
    //    threads: usize,
    //},
}

pub fn read_params() -> Params {
    Command::parse().cmd
}

pub fn parse_threshold_cli(
    threshold_str: &str,
    require: RequireThreshold,
) -> Result<Vec<Threshold>, Error> {
    let mut thresholds = Vec::new();

    for (i, el) in threshold_str.split(',').enumerate() {
        let rel_val = match f64::from_str(el.trim()) {
            Ok(t) => {
                if (0.0..=1.0).contains(&t) {
                    Ok(t)
                } else {
                    Err(Error::new(
                        ErrorKind::InvalidData,
                        format!(
                            "relative threshold \"{}\" ({}. element in list) must be within [0,1].",
                            &threshold_str,
                            i + 1
                        ),
                    ))
                }
            }
            Err(_) => Err(Error::new(
                ErrorKind::InvalidData,
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
                    Error::new(
                            ErrorKind::InvalidData,
                            format!("threshold \"{}\" ({}. element in list) is required to be integer, but isn't.",
                    &threshold_str,
                    i + 1)))?),
            RequireThreshold::Relative => Threshold::Relative(rel_val?),
            RequireThreshold::Either =>
        if let Ok(t) = usize::from_str(el.trim()) {
            Threshold::Absolute(t)
        } else {
            Threshold::Relative(rel_val?)
            }
            }
            );
    }
    Ok(thresholds)
}

pub fn set_number_of_threads(params: &Params) {
    if let Params::Histgrowth { threads, .. }
    | Params::Hist { threads, .. }
    | Params::Info { threads, .. }
    | Params::Subset { threads, .. }
    | Params::OrderedHistgrowth { threads, .. }
    | Params::Table { threads, .. }
    //| Params::Cdbg { threads, .. } 
    = params
    {
        if *threads > 0 {
            log::info!("running panacus on {} threads", &threads);
            rayon::ThreadPoolBuilder::new()
                .num_threads(*threads)
                .build_global()
                .unwrap();
        } else {
            log::info!("running panacus using all available CPUs");
            rayon::ThreadPoolBuilder::new().build_global().unwrap();
        }
    }
}

// make sure either group, groupby-sample, or groupby-haplotype is set
pub fn validate_single_groupby_option(
    groupby: &str,
    groupby_haplotype: bool,
    groupby_sample: bool,
) -> Result<(), Error> {
    let mut c = 0;
    c += (!groupby.is_empty()) as u8;
    c += (groupby_haplotype) as u8;
    c += (groupby_sample) as u8;
    if c > 1 {
        let msg = "At most one option of groupby, groupby-haplotype, and groupby-sample can be set at once, but at least two are given.";
        log::error!("{}", &msg);
        return Err(Error::new(ErrorKind::InvalidInput, msg));
    }
    Ok(())
}

pub fn run<W: Write>(params: Params, out: &mut BufWriter<W>) -> Result<(), Error> {
    set_number_of_threads(&params);

    if let Params::Histgrowth {
        ref groupby,
        groupby_haplotype,
        groupby_sample,
        ..
    }
    | Params::Hist {
        ref groupby,
        groupby_haplotype,
        groupby_sample,
        ..
    }
    | Params::Info {
        ref groupby,
        groupby_haplotype,
        groupby_sample,
        ..
    }
    | Params::Subset {
        ref groupby,
        groupby_haplotype,
        groupby_sample,
        ..
    }
    | Params::OrderedHistgrowth {
        ref groupby,
        groupby_haplotype,
        groupby_sample,
        ..
    }
    | Params::Table {
        ref groupby,
        groupby_haplotype,
        groupby_sample,
        ..
    }
    //| Params::Cdbg {
    //    ref groupby,
    //    groupby_haplotype,
    //    groupby_sample,
    //    ..
    //} 
    = params
    {
        validate_single_groupby_option(groupby, groupby_haplotype, groupby_sample)?;
    }

    match params {
        Params::Histgrowth {
            ref gfa_file,
            count,
            output_format,
            ..
        } => {
            //Hist
            let graph_aux = match output_format {
                OutputFormat::Html => GraphAuxilliary::from_gfa(gfa_file, CountType::All),
                _ => GraphAuxilliary::from_gfa(gfa_file, count),
            };
            let abacus_aux = AbacusAuxilliary::from_params(&params, &graph_aux)?;
            let abaci = AbacusByTotal::abaci_from_gfa(gfa_file, count, &graph_aux, &abacus_aux)?;
            let mut hists = Vec::new();
            for abacus in abaci {
                hists.push(Hist::from_abacus(&abacus, Some(&graph_aux)));
            }
            //Growth
            let hist_aux = HistAuxilliary::from_params(&params)?;
            let filename = Path::new(&gfa_file).file_name().unwrap().to_str().unwrap();
            let growths: Vec<(CountType, Vec<Vec<f64>>)> = hists
                .par_iter()
                .map(|h| (h.count, h.calc_all_growths(&hist_aux)))
                .collect();
            log::info!("reporting histgrowth table");
            match output_format {
                OutputFormat::Table => write_histgrowth_table(&hists, &growths, &hist_aux, out)?,
                OutputFormat::Html => {
                    let mut data = bufreader_from_compressed_gfa(gfa_file);
                    let (_, _, _, paths_len) =
                        parse_gfa_paths_walks(&mut data, &abacus_aux, &graph_aux, &CountType::Node);

                    let info = graph_aux.info(&paths_len, &abacus_aux.groups, true);
                    write_histgrowth_html(
                        &Some(hists),
                        &growths,
                        &hist_aux,
                        filename,
                        None,
                        Some(info),
                        out,
                    )?
                }
            };
        }
        Params::Hist {
            ref gfa_file,
            count,
            output_format,
            ..
        } => {
            let graph_aux = match output_format {
                OutputFormat::Html => GraphAuxilliary::from_gfa(gfa_file, CountType::All),
                _ => GraphAuxilliary::from_gfa(gfa_file, count),
            };
            let abacus_aux = AbacusAuxilliary::from_params(&params, &graph_aux)?;
            let abaci = AbacusByTotal::abaci_from_gfa(gfa_file, count, &graph_aux, &abacus_aux)?;
            let mut hists = Vec::new();
            for abacus in abaci {
                hists.push(Hist::from_abacus(&abacus, Some(&graph_aux)));
            }

            let filename = Path::new(&gfa_file).file_name().unwrap().to_str().unwrap();
            match output_format {
                OutputFormat::Table => write_hist_table(&hists, out)?,
                OutputFormat::Html => {
                    let mut data = bufreader_from_compressed_gfa(gfa_file);
                    let (_, _, _, paths_len) =
                        parse_gfa_paths_walks(&mut data, &abacus_aux, &graph_aux, &CountType::Node);

                    let info = graph_aux.info(&paths_len, &abacus_aux.groups, true);
                    write_hist_html(&hists, &filename, Some(info), out)?
                }
            };
        }
        Params::Growth {
            ref hist_file,
            output_format,
            ..
        } => {
            let hist_aux = HistAuxilliary::from_params(&params)?;
            log::info!("loading coverage histogram from {}", hist_file);
            let mut data = BufReader::new(fs::File::open(hist_file)?);
            let (coverages, comments) = parse_hists(&mut data)?;
            for c in comments {
                out.write_all(&c[..])?;
                out.write_all(b"\n")?;
            }
            let hists: Vec<Hist> = coverages
                .into_iter()
                .map(|(count, coverage)| Hist { count, coverage })
                .collect();

            let filename = Path::new(&hist_file).file_name().unwrap().to_str().unwrap();
            let growths: Vec<(CountType, Vec<Vec<f64>>)> = hists
                .par_iter()
                .map(|h| (h.count, h.calc_all_growths(&hist_aux)))
                .collect();
            log::info!("reporting histgrowth table");
            match output_format {
                OutputFormat::Table => write_histgrowth_table(&hists, &growths, &hist_aux, out)?,
                OutputFormat::Html => write_histgrowth_html(
                    &Some(hists),
                    &growths,
                    &hist_aux,
                    filename,
                    None,
                    None,
                    out,
                )?,
            };
        }
        Params::Info {
            ref gfa_file,
            output_format,
            ..
        } => {
            let graph_aux = GraphAuxilliary::from_gfa(gfa_file, CountType::All);

            let abacus_aux = AbacusAuxilliary::from_params(&params, &graph_aux)?;
            let mut data = bufreader_from_compressed_gfa(gfa_file);
            let (_, _, _, paths_len) =
                parse_gfa_paths_walks(&mut data, &abacus_aux, &graph_aux, &CountType::Node);

            match output_format {
                OutputFormat::Table => {
                    let has_groups = match params {
                        Params::Info {
                            ref groupby,
                            groupby_haplotype,
                            groupby_sample,
                            ..
                        } => groupby != "" || groupby_haplotype || groupby_sample,
                        _ => false,
                    };
                    let info = graph_aux.info(&paths_len, &abacus_aux.groups, has_groups);
                    write_info(info, out)?
                }
                OutputFormat::Html => {
                    let info = graph_aux.info(&paths_len, &abacus_aux.groups, true);
                    let filename = Path::new(&gfa_file).file_name().unwrap().to_str().unwrap();
                    write_info_html(&filename, info, out)?
                }
            };
        }
        Params::Subset {
            ref gfa_file,
            flt_quorum_min,
            flt_quorum_max,
            flt_length_min,
            flt_length_max,
            ..
        } => {
            let graph_aux = GraphAuxilliary::from_gfa(gfa_file, CountType::Node);
            let abacus_aux = AbacusAuxilliary::from_params(&params, &graph_aux)?;
            let mut data = bufreader_from_compressed_gfa(gfa_file);
            let abacus =
                AbacusByTotal::from_gfa(&mut data, &abacus_aux, &graph_aux, CountType::Node);
            data = bufreader_from_compressed_gfa(gfa_file);

            subset_path_gfa(
                &mut data,
                &abacus,
                &graph_aux,
                flt_quorum_min,
                flt_quorum_max,
                flt_length_min,
                flt_length_max,
            );
            //println!("{}", abacus.countable.len()-1);
        }
        Params::OrderedHistgrowth {
            ref gfa_file,
            count,
            output_format,
            ..
        } => {
            let graph_aux = match output_format {
                OutputFormat::Html => GraphAuxilliary::from_gfa(gfa_file, CountType::All),
                _ => GraphAuxilliary::from_gfa(gfa_file, count),
            };
            let abacus_aux = AbacusAuxilliary::from_params(&params, &graph_aux)?;
            let mut data = bufreader_from_compressed_gfa(gfa_file);
            let abacus = AbacusByGroup::from_gfa(&mut data, &abacus_aux, &graph_aux, count, true)?;
            let hist_aux = HistAuxilliary::from_params(&params)?;
            match output_format {
                OutputFormat::Table => {
                    write_ordered_histgrowth_table(&abacus, &hist_aux, out)?;
                }
                OutputFormat::Html => {
                    let mut data = bufreader_from_compressed_gfa(gfa_file);
                    let (_, _, _, paths_len) =
                        parse_gfa_paths_walks(&mut data, &abacus_aux, &graph_aux, &CountType::Node);

                    let info = graph_aux.info(&paths_len, &abacus_aux.groups, true);
                    write_ordered_histgrowth_html(
                        &abacus,
                        &hist_aux,
                        gfa_file,
                        count,
                        Some(info),
                        out,
                    )?;
                }
            }
        }
        Params::Table {
            ref gfa_file,
            count,
            total,
            ..
        } => {
            let graph_aux = GraphAuxilliary::from_gfa(gfa_file, count);
            let abacus_aux = AbacusAuxilliary::from_params(&params, &graph_aux)?;
            let mut data = BufReader::new(fs::File::open(gfa_file)?);
            let abacus = AbacusByGroup::from_gfa(&mut data, &abacus_aux, &graph_aux, count, total)?;

            abacus.to_tsv(total, out)?;
        } //Params::Cdbg {
          //    ref gfa_file, k, ..
          //} => {
          //    let graph_aux = GraphAuxilliary::from_cdbg_gfa(gfa_file, k);
          //    let abacus_aux = AbacusAuxilliary::from_params(&params, &graph_aux)?;

          //    let mut hists = Vec::new();
          //    let abaci_node =
          //        AbacusByTotal::abaci_from_gfa(gfa_file, CountType::Node, &graph_aux, &abacus_aux)?;
          //    let abaci_bp =
          //        AbacusByTotal::abaci_from_gfa(gfa_file, CountType::Bp, &graph_aux, &abacus_aux)?;
          //    hists.push(Hist::from_abacus(&abaci_node[0], None));
          //    hists.push(Hist::from_abacus(&abaci_bp[0], Some(&graph_aux)));

          //    // k-mers and unimer
          //    let n = hists[0].coverage.len();
          //    let mut kmer: Vec<usize> = vec![0; n];
          //    let mut unimer: Vec<usize> = vec![0; n];

          //    for i in 0..n {
          //        kmer[i] = hists[1].coverage[i] - (k - 1) * hists[0].coverage[i];
          //        unimer[i] = hists[1].coverage[i] - k * hists[0].coverage[i];
          //    }

          //    let mut data = BufReader::new(fs::File::open(&gfa_file)?);
          //    let abaci_infix_eq =
          //        AbacusByTotal::from_cdbg_gfa(&mut data, &abacus_aux, &graph_aux, k, &unimer);

          //    println!("# infix_eq");
          //    for v in abaci_infix_eq.countable.iter() {
          //        println!("{}", v);
          //    }

          //    println!("# kmer");
          //    for i in 1..kmer.len() {
          //        println!("{}", kmer[i]);
          //    }
          //    write_hist_table(&hists, out)?;
          //}
    }

    Ok(())
}
