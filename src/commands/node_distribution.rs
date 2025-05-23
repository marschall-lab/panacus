use clap::{arg, Arg, ArgMatches, Command};

use crate::analysis_parameter::AnalysisParameter;

pub fn get_subcommand() -> Command {
    Command::new("node-distribution")
        .about("Return the list of bins with there coverages, log10-lengths and log10-sizes. Due to this being the values for the centers of the hexagons shown in the html plot and not real values, some values might be negative.")
        .args(&[
            arg!(gfa_file: <GFA_FILE> "graph in GFA1 format, accepts also compressed (.gz) file"),
            Arg::new("radius")
                .help("Radius of the hexagons used to bin")
                .short('r')
                .long("radius")
                .value_parser(clap::value_parser!(u32))
                .default_value("20"),
        ])
}

pub fn get_instructions(
    args: &ArgMatches,
) -> Option<Result<Vec<AnalysisParameter>, anyhow::Error>> {
    if let Some(args) = args.subcommand_matches("node-distribution") {
        // let graph = args
        //     .get_one::<String>("gfa_file")
        //     .expect("info subcommand has gfa file")
        //     .to_owned();
        let radius = args
            .get_one::<u32>("radius")
            .expect("node-distribution has radius")
            .to_owned();
        Some(Ok(vec![AnalysisParameter::NodeDistribution { radius }]))
    } else {
        None
    }
}
