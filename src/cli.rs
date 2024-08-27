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
use crate::path::*;
use crate::path_parser::*;
use crate::hist::*;
use crate::html::*;
use crate::io::*;
use crate::util::*;

pub enum RequireThreshold {
    Absolute,
    Relative,
    //#[allow(dead_code)]
    //Either,
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
        #[clap(short = 'l', long, help = "Ignore all countables with a coverage lower than the specified threshold. The coverage of a countable corresponds to the number of path/walk that contain it. Repeated appearances of a countable in the same path/walk are counted as one. You can pass a comma-separated list of coverage thresholds, each one will produce a separated growth curve (e.g., --coverage 2,3). Use --quorum to set a threshold in conjunction with each coverage (e.g., --quorum 0.5,0.9)", default_value = "1")] 
        coverage: String, 
        #[clap(short, long, help = "Unlike the --coverage parameter, which specifies a minimum constant number of paths for all growth point m (1 <= m <= num_paths), --quorum adjust the threshold based on m. At each m, a countable is counted in the average growth if the countable is contained in at least floor(m*quorum) paths. Example: A quorum of 0.9 requires a countable to be in 90% of paths for each subset size m. At m=10, it must appear in at least 9 paths. At m=100, it must appear in at least 90 paths. A quorum of 1 (100%) requires presence in all paths of the subset, corresponding to the core. Default: 0, a countable counts if it is present in any path at each growth point. Specify multiple quorum values with a comma-separated list (e.g., --quorum 0.5,0.9). Use --coverage to set static path thresholds in conjunction with variable quorum percentages (e.g., --coverage 5,10).", default_value = "0")] 
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
        #[clap(short = 'l', long, help = "Ignore all countables with a coverage lower than the specified threshold. The coverage of a countable corresponds to the number of path/walk that contain it. Repeated appearances of a countable in the same path/walk are counted as one. You can pass a comma-separated list of coverage thresholds, each one will produce a separated growth curve (e.g., --coverage 2,3). Use --quorum to set a threshold in conjunction with each coverage (e.g., --quorum 0.5,0.9)", default_value = "1")] 
        coverage: String, 
        #[clap(short, long, help = "Unlike the --coverage parameter, which specifies a minimum constant number of paths for all growth point m (1 <= m <= num_paths), --quorum adjust the threshold based on m. At each m, a countable is counted in the average growth if the countable is contained in at least floor(m*quorum) paths. Example: A quorum of 0.9 requires a countable to be in 90% of paths for each subset size m. At m=10, it must appear in at least 9 paths. At m=100, it must appear in at least 90 paths. A quorum of 1 (100%) requires presence in all paths of the subset, corresponding to the core. Default: 0, a countable counts if it is present in any path at each growth point. Specify multiple quorum values with a comma-separated list (e.g., --quorum 0.5,0.9). Use --coverage to set static path thresholds in conjunction with variable quorum percentages (e.g., --coverage 5,10).", default_value = "0")] 
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

    #[clap(alias = "S", about = "Return general graph and paths statistics")]
    Stats {
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

//This is just used for tests, but we use it in multiple tests in other files as well
impl Params {
    pub fn default_histgrowth() -> Self {
        Params::Histgrowth {
            gfa_file: String::new(),
            count: CountType::Node,
            positive_list: String::new(),
            negative_list: String::new(),
            groupby: String::new(),
            groupby_haplotype: false,
            groupby_sample: false,
            coverage: "1".to_string(),
            quorum: "0".to_string(),
            hist: false,
            output_format: OutputFormat::Table,
            threads: 0,
        }
    }
}


pub fn read_params() -> Params {
    Command::parse().cmd
}

pub fn parse_threshold_cli(
    threshold_str: &str,
    require: RequireThreshold,
) -> Result<Vec<Threshold>, Error> {
    threshold_str
        .split(',')
        .enumerate()
        .map(|(i, el)| {
            let trimmed = el.trim();
                
            match require {
                RequireThreshold::Absolute => {
                    let absolute_result = usize::from_str(trimmed).map(Threshold::Absolute).map_err(|_| {
                        Error::new(ErrorKind::InvalidData,
                            format!(
                                "Threshold \"{}\" ({}. element) should be a valid integer for absolute threshold.",
                                trimmed, i + 1
                            ),
                        )
                    });
                    absolute_result
                }
                RequireThreshold::Relative => {
                    // Parse as either float (relative) or integer (absolute)
                    let relative_result = f64::from_str(trimmed)
                        .map_err(|_| Error::new(ErrorKind::InvalidData, format!(
                            "Threshold \"{}\" ({}. element) should be a valid float for relative threshold.",
                            trimmed, i + 1
                        )))
                        .and_then(|t| {
                            if 0.0 <= t && t <=1.0 {
                                Ok(Threshold::Relative(t))
                            } else {
                                Err(Error::new(
                                    ErrorKind::InvalidData,
                                    format!(
                                        "Relative threshold \"{}\" ({}. element) must be within [0,1].",
                                        trimmed, i + 1
                                    ),
                                ))
                            }
                        });
                    relative_result
                }
            }
        })
        .collect() 
}

// set number of threads can be run only once, otherwise it throws an error of the 
// GlobalPoolAlreadyInitialized, which unfortunately is not pub therefore we cannot catch it.
// https://github.com/rayon-rs/rayon/issues/878
// We run this function in the main otherwise in the tests the second time we run the function
// "run" it will crush
pub fn set_number_of_threads(params: &Params) {
    if let Params::Histgrowth { threads, .. }
    | Params::Hist { threads, .. }
    | Params::Stats { threads, .. }
    | Params::Subset { threads, .. }
    | Params::OrderedHistgrowth { threads, .. }
    | Params::Table { threads, .. }
    //| Params::Cdbg { threads, .. } 
    = params {
        //if num_threads is 0 then the Rayon will select 
        //the number of threads automatically 
        rayon::ThreadPoolBuilder::new()
            .num_threads(*threads)
            .build_global()
            .expect("Failed to initialize global thread pool");
        log::info!("running panacus on {} threads", rayon::current_num_threads());
    }
}

// make sure only one of group, groupby-sample, or groupby-haplotype is set
pub fn validate_single_groupby_option(params: &Params) -> Result<(), Error> {
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
    | Params::Stats {
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
    = params {
        let options_set = [
            !(*groupby).is_empty(),
            *groupby_haplotype,
            *groupby_sample,
        ];

        let active_options = options_set.iter().filter(|&option| *option).count();

        if active_options > 1 {
            let msg = "At most only one of groupby, groupby-haplotype, or groupby-sample can be set. Multiple options were provided.";
            log::error!("{}", msg);
            return Err(Error::new(ErrorKind::InvalidInput, msg));
        }

    }
    Ok(())
}

pub fn run<W: Write>(params: Params, out: &mut BufWriter<W>) -> Result<(), Error> {

    validate_single_groupby_option(&params)?;

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
            let path_aux = PathAuxilliary::from_params(&params, &graph_aux)?;
            let abaci = AbacusByTotal::abaci_from_gfa(gfa_file, count, &graph_aux, &path_aux)?;
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
                        parse_gfa_paths_walks(&mut data, &path_aux, &graph_aux, &CountType::Node);

                    let stats = graph_aux.stats(&paths_len);
                    write_histgrowth_html(
                        &Some(hists),
                        &growths,
                        &hist_aux,
                        &filename,
                        None,
                        Some(stats),
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
            let path_aux = PathAuxilliary::from_params(&params, &graph_aux)?;
            let abaci = AbacusByTotal::abaci_from_gfa(gfa_file, count, &graph_aux, &path_aux)?;
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
                        parse_gfa_paths_walks(&mut data, &path_aux, &graph_aux, &CountType::Node);

                    let stats = graph_aux.stats(&paths_len);
                    write_hist_html(&hists, &filename, Some(stats), out)?
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
            let mut data = BufReader::new(fs::File::open(&hist_file)?);
            let (coverages, comments) = parse_hists(&mut data)?;
            for c in comments {
                out.write(&c[..])?;
                out.write(b"\n")?;
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
                    &filename,
                    None,
                    None,
                    out,
                )?,
            };
        }
        Params::Stats {
            ref gfa_file,
            output_format,
            ..
        } => {
            let graph_aux = GraphAuxilliary::from_gfa(gfa_file, CountType::All);

            let path_aux = PathAuxilliary::from_params(&params, &graph_aux)?;
            let mut data = bufreader_from_compressed_gfa(gfa_file);
            let (_, _, _, paths_len) =
                parse_gfa_paths_walks(&mut data, &path_aux, &graph_aux, &CountType::Node);

            let stats = graph_aux.stats(&paths_len);
            let filename = Path::new(&gfa_file).file_name().unwrap().to_str().unwrap();
            match output_format {
                OutputFormat::Table => write_stats(stats, out)?,
                OutputFormat::Html => write_stats_html(&filename, stats, out)?,
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
            let path_aux = PathAuxilliary::from_params(&params, &graph_aux)?;
            let mut data = bufreader_from_compressed_gfa(gfa_file);
            let abacus =
                AbacusByTotal::from_gfa(&mut data, &path_aux, &graph_aux, CountType::Node);
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
            let path_aux = PathAuxilliary::from_params(&params, &graph_aux)?;
            let mut data = bufreader_from_compressed_gfa(gfa_file);
            let abacus = AbacusByGroup::from_gfa(&mut data, &path_aux, &graph_aux, count, false)?;
            let hist_aux = HistAuxilliary::from_params(&params)?;
            match output_format {
                OutputFormat::Table => {
                    write_ordered_histgrowth_table(&abacus, &hist_aux, out)?;
                }
                OutputFormat::Html => {
                    let mut data = bufreader_from_compressed_gfa(gfa_file);
                    let (_, _, _, paths_len) =
                        parse_gfa_paths_walks(&mut data, &path_aux, &graph_aux, &CountType::Node);

                    let stats = graph_aux.stats(&paths_len);
                    write_ordered_histgrowth_html(
                        &abacus,
                        &hist_aux,
                        &gfa_file,
                        count,
                        Some(stats),
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
            let path_aux = PathAuxilliary::from_params(&params, &graph_aux)?;
            let mut data = BufReader::new(fs::File::open(&gfa_file)?);
            let abacus = AbacusByGroup::from_gfa(&mut data, &path_aux, &graph_aux, count, total)?;

            abacus.to_tsv(total, out)?;
        }
        //Params::Cdbg {
        //    ref gfa_file, k, ..
        //} => {
        //    let graph_aux = GraphAuxilliary::from_cdbg_gfa(gfa_file, k);
        //    let path_aux = PathAuxilliary::from_params(&params, &graph_aux)?;

        //    let mut hists = Vec::new();
        //    let abaci_node =
        //        AbacusByTotal::abaci_from_gfa(gfa_file, CountType::Node, &graph_aux, &path_aux)?;
        //    let abaci_bp =
        //        AbacusByTotal::abaci_from_gfa(gfa_file, CountType::Bp, &graph_aux, &path_aux)?;
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
        //        AbacusByTotal::from_cdbg_gfa(&mut data, &path_aux, &graph_aux, k, &unimer);

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

#[cfg(test)]
mod tests {
    use super::*;


    #[test]
    fn test_parse_threshold_cli_relative_success() {
        let threshold_str = "0.2,0.5,0.9";
        let result = parse_threshold_cli(threshold_str, RequireThreshold::Relative);
        assert!(result.is_ok());
        let thresholds = result.unwrap();
        assert_eq!(thresholds.len(), 3);
        assert_eq!(thresholds[0], Threshold::Relative(0.2));
        assert_eq!(thresholds[1], Threshold::Relative(0.5));
        assert_eq!(thresholds[2], Threshold::Relative(0.9));
    }

    #[test]
    fn test_parse_threshold_cli_absolute_success() {
        let threshold_str = "5,10,15";
        let result = parse_threshold_cli(threshold_str, RequireThreshold::Absolute);
        assert!(result.is_ok());
        let thresholds = result.unwrap();
        assert_eq!(thresholds.len(), 3);
        assert_eq!(thresholds[0], Threshold::Absolute(5));
        assert_eq!(thresholds[1], Threshold::Absolute(10));
        assert_eq!(thresholds[2], Threshold::Absolute(15));
    }

    #[test]
    fn test_parse_threshold_cli_invalid_float_in_absolute() {
        let threshold_str = "5.5,10,15";
        let result = parse_threshold_cli(threshold_str, RequireThreshold::Absolute);
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().kind(),
            ErrorKind::InvalidData
        );
    }

    #[test]
    fn test_parse_threshold_cli_invalid_value_in_relative() {
        let threshold_str = "0.2,1.2,0.9"; // 1.2 is out of range for relative threshold
        let result = parse_threshold_cli(threshold_str, RequireThreshold::Relative);
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().kind(),
            ErrorKind::InvalidData
        );
    }

    #[test]
    fn test_validate_single_groupby_option() {
        let test_cases = vec![
            // Valid cases
            ("", false, false, true),       // None set
            ("group1", false, false, true), // Only groupby is set
            ("", true, false, true),        // Only groupby_haplotype is set
            ("", false, true, true),        // Only groupby_sample is set

            // Invalid cases
            ("group1", true, false, false), // groupby and groupby_haplotype set
            ("group1", false, true, false), // groupby and groupby_sample set
            ("", true, true, false),        // groupby_haplotype and groupby_sample set
            ("group1", true, true, false),  // All options set
        ];

        for (test_groupby, test_groupby_haplotype, test_groupby_sample, should_pass) in test_cases {
            let mut params = Params::default_histgrowth();
            if let Params::Histgrowth {
                ref mut groupby,
                ref mut groupby_haplotype,
                ref mut groupby_sample,
                ..
            } = params {
                *groupby = test_groupby.to_string();
                *groupby_haplotype = test_groupby_haplotype;
                *groupby_sample = test_groupby_sample;
            }

            let result = validate_single_groupby_option(&params);
            if should_pass {
                assert!(
                    result.is_ok(),
                    "Expected OK, but got error for input: groupby = '{}', groupby_haplotype = {}, groupby_sample = {}",
                    test_groupby, test_groupby_haplotype, test_groupby_sample
                );
            } else {
                assert!(
                    result.is_err(),
                    "Expected error, but got OK for input: groupby = '{}', groupby_haplotype = {}, groupby_sample = {}",
                    test_groupby, test_groupby_haplotype, test_groupby_sample
                );
            }
        }
    }
   
    #[test]
    #[should_panic(expected = "Error opening gfa file non_existent_file.gfa")]
    fn test_run_function_should_panic_when_file_not_found() {
        let mut params = Params::default_histgrowth();
        if let Params::Histgrowth { ref mut gfa_file, .. } = params {
            *gfa_file = "non_existent_file.gfa".to_string();
        }

        let mut output = BufWriter::new(Vec::new());
        let _ = run(params, &mut output); // This should panic due to the non-existent file
    }
}
