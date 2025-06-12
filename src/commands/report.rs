use std::fs::File;
use std::io::BufReader;

use clap::{Arg, ArgAction, ArgMatches, Command};

use crate::analysis_parameter::AnalysisRun;

pub fn get_subcommand() -> Command {
    Command::new("report")
        .about("Create an html report from a YAML config file")
        .args(&[Arg::new("yaml_file")
            .required(false)
            .help("Specifies yaml config")])
        .args(&[Arg::new("dry_run")
            .required(false)
            .long("dry-run")
            .short('d')
            .action(ArgAction::SetTrue)
            .help(
                "If set, no actual computation is done, only the planned computation will be shown",
            )])
        .args(&[Arg::new("json")
                .required(false)
                .long("json")
                .short('j')
                .action(ArgAction::SetTrue)
                .help(
                    "Instead of an HTML report, a json result will be delivered. These can later be combined and rendered as a single HTML.",
                )
        ])
}

pub fn get_instructions(args: &ArgMatches) -> Option<Result<Vec<AnalysisRun>, anyhow::Error>> {
    if let Some(args) = args.subcommand_matches("report") {
        Some(parse_report_args(args))
    } else {
        None
    }
}

fn parse_report_args(args: &ArgMatches) -> Result<Vec<AnalysisRun>, anyhow::Error> {
    if let Some(yaml_file) = args.get_one::<String>("yaml_file").cloned() {
        let f = File::open(yaml_file)?;
        let reader = BufReader::new(f);
        let contents = serde_yaml::from_reader(reader)?;
        Ok(contents)
    } else {
        println!(
            "
# Missing YAML file!
#
# Example YAML:
# To get started copy this into a .yaml file and edit it

- graph: ../graphs/test_graph.gfa
  grouping: Haplotype
  analyses:
    - !Hist
      count_type: Bp
    - !Growth
      coverage: 1,1,2
      quorum: 0,0.9,0

# For more information see: https://github.com/codialab/panacus/wiki
                "
        );
        Ok(Vec::new())
    }
}
