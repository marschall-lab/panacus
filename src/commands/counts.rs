use clap::{arg, ArgMatches, Command};

use crate::analysis_parameter::AnalysisParameter;

pub fn get_subcommand() -> Command {
    Command::new("counts")
        .about("Return list of nodes with coverages and lenghts")
        .args(&[
            arg!(gfa_file: <GFA_FILE> "graph in GFA1 format, accepts also compressed (.gz) file"),
        ])
}

pub fn get_instructions(
    args: &ArgMatches,
) -> Option<Result<Vec<AnalysisParameter>, anyhow::Error>> {
    if let Some(args) = args.subcommand_matches("counts") {
        let graph = args
            .get_one::<String>("gfa_file")
            .expect("info subcommand has gfa file")
            .to_owned();
        Some(Ok(vec![AnalysisParameter::Counts { graph }]))
    } else {
        None
    }
}
