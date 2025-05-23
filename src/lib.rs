/* private use */
pub mod analyses;
mod analysis_parameter;
mod commands;
pub mod graph_broker;
mod html_report;
mod io;
mod util;

use env_logger::Builder;
use log::LevelFilter;
use std::{fmt::Debug, io::Write};
use thiserror::Error;

use analyses::Analysis;
use analysis_parameter::{AnalysisRun, Task};
use clap::{Arg, ArgAction, ArgMatches, Command};
use graph_broker::{GraphBroker, GraphState};
use html_report::AnalysisSection;

use std::fs::File;
use std::io::BufReader;

#[macro_export]
macro_rules! clap_enum_variants {
    // Credit: Johan Andersson (https://github.com/repi)
    // Code from https://github.com/clap-rs/clap/discussions/4264
    ($e: ty) => {{
        use clap::builder::TypedValueParser;
        use strum::VariantNames;
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

#[macro_export]
macro_rules! some_or_return {
    ($x:expr, $y:expr) => {
        match $x {
            Some(v) => v,
            None => return $y,
        }
    };
}

fn set_number_of_threads(params: &ArgMatches) {
    //if num_threads is 0 then the Rayon will select
    //the number of threads to the core number automatically
    let threads = params.get_one("threads").unwrap();
    rayon::ThreadPoolBuilder::new()
        .num_threads(*threads)
        .build_global()
        .expect("Failed to initialize global thread pool");
    log::info!(
        "running panacus on {} threads",
        rayon::current_num_threads()
    );
}

fn set_verbosity(args: &ArgMatches) {
    if args.get_flag("verbose") {
        Builder::new().filter_level(LevelFilter::Debug).init();
    } else {
        Builder::new().filter_level(LevelFilter::Info).init();
    }
}

pub fn run_cli() -> Result<(), anyhow::Error> {
    let mut out = std::io::BufWriter::new(std::io::stdout());

    // read parameters and store them in memory
    // let params = cli::read_params();
    let args = Command::new("panacus")
        .subcommand(commands::render::get_subcommand())
        .subcommand(commands::report::get_subcommand())
        .subcommand(commands::hist::get_subcommand())
        .subcommand(commands::growth::get_subcommand())
        .subcommand(commands::histgrowth::get_subcommand())
        .subcommand(commands::info::get_subcommand())
        .subcommand(commands::ordered_histgrowth::get_subcommand())
        .subcommand(commands::table::get_subcommand())
        .subcommand(commands::node_distribution::get_subcommand())
        .subcommand(commands::similarity::get_subcommand())
        .subcommand_required(true)
        .arg(
            Arg::new("threads")
                .short('t')
                .action(ArgAction::Set)
                .value_name("COUNT")
                .default_value("0")
                .value_parser(clap::value_parser!(usize))
                .global(true)
                .help("Set the number of threads used (default: use all threads)"),
        )
        .arg(
            Arg::new("verbose")
                .short('v')
                .long("verbose")
                .action(ArgAction::SetTrue)
                .global(true)
                .help("Set the number of threads used (default: use all threads)"),
        )
        .get_matches();

    set_verbosity(&args);
    set_number_of_threads(&args);

    let mut instructions: Vec<AnalysisRun> = Vec::new();
    let mut shall_write_html = false;
    let mut dry_run = false;
    let mut json = false;

    if let Some(args) = args.subcommand_matches("render") {
        let json_files: Vec<String> = args
            .get_many::<String>("json_files")
            .unwrap()
            .cloned()
            .collect();
        let mut full_report = Vec::new();
        for file_path in &json_files {
            let file = File::open(file_path)?;
            let reader = BufReader::new(file);

            // Read the JSON contents of the file as an instance of `User`.
            let report: Vec<AnalysisSection> = serde_json::from_reader(reader)?;
            full_report.extend(report);
        }
        let mut registry = handlebars::Handlebars::new();
        let report_text =
            AnalysisSection::generate_report(full_report, &mut registry, &json_files[0])?;
        writeln!(&mut out, "{report_text}")?;
        return Ok(());
    }

    if let Some(report) = commands::report::get_instructions(&args) {
        shall_write_html = true;
        instructions.extend(report?);
        if let Some(report_matches) = args.subcommand_matches("report") {
            dry_run = report_matches.get_flag("dry_run");
            json = report_matches.get_flag("json");
        }
    }
    // if let Some(hist) = commands::hist::get_instructions(&args) {
    //     instructions.extend(hist?);
    // }
    // if let Some(growth) = commands::growth::get_instructions(&args) {
    //     instructions.extend(growth?);
    // }
    // if let Some(histgrowth) = commands::histgrowth::get_instructions(&args) {
    //     instructions.extend(histgrowth?);
    // }
    // if let Some(info) = commands::info::get_instructions(&args) {
    //     instructions.extend(info?);
    // }
    // if let Some(ordered_histgrowth) = commands::ordered_histgrowth::get_instructions(&args) {
    //     instructions.extend(ordered_histgrowth?);
    // }
    // if let Some(table) = commands::table::get_instructions(&args) {
    //     instructions.extend(table?);
    // }
    // if let Some(counts) = commands::node_distribution::get_instructions(&args) {
    //     instructions.extend(counts?);
    // }
    // if let Some(similarity) = commands::similarity::get_instructions(&args) {
    //     instructions.extend(similarity?);
    // }

    let instructions: Vec<Task> = get_tasks(instructions)?;
    log::info!("{:?}", instructions);

    // ride on!
    if !dry_run {
        execute_pipeline(instructions, &mut out, shall_write_html, json)?;
    } else {
        println!("{:#?}", instructions);
    }

    // clean up & close down
    out.flush()?;
    Ok(())
}

#[derive(Error, Debug)]
pub enum ConfigParseError {
    #[error("no config block with name {name} was found")]
    NameNotFound { name: String },
}

fn get_tasks(instructions: Vec<AnalysisRun>) -> anyhow::Result<Vec<Task>> {
    let tasks = AnalysisRun::convert_to_tasks(instructions);
    Ok(tasks)
}

pub fn execute_pipeline<W: Write>(
    mut instructions: Vec<Task>,
    out: &mut std::io::BufWriter<W>,
    shall_write_html: bool,
    json: bool,
) -> anyhow::Result<()> {
    if instructions.is_empty() {
        log::warn!("No instructions supplied");
        return Ok(());
    }
    let mut report = Vec::new();
    let mut gb = GraphBroker::new();
    for index in 0..instructions.len() {
        match &mut instructions[index] {
            Task::Analysis(analysis) => {
                log::info!("Executing Analysis: {}", analysis.get_type());
                report.extend(analysis.generate_report_section(Some(&gb))?);
            }
            Task::GraphStateChange {
                graph,
                subset,
                exclude,
                grouping,
                nice,
                reqs,
            } => {
                log::info!("Executing graph change: {:?}", reqs);
                gb.change_graph_state(
                    GraphState {
                        graph: graph.to_string(),
                        subset: subset.to_string(),
                        exclude: exclude.to_string(),
                        grouping: grouping.clone(),
                    },
                    &reqs,
                    *nice,
                )?;
            }
            Task::OrderChange(order) => {
                log::info!("Executing order change: {:?}", order);
                unimplemented!("Order Change is not yet implemented");
            }
            Task::AbacusByGroupCSCChange => {
                log::info!("Executing AbacusByGroup CSC change");
                unimplemented!("CSC Change is not yet implemented");
            }
        }
    }
    if json {
        let json_text = serde_json::to_string_pretty(&report)?;
        writeln!(out, "{json_text}")?;
    } else if shall_write_html {
        let mut registry = handlebars::Handlebars::new();
        let report =
            AnalysisSection::generate_report(report, &mut registry, "<Placeholder Filename>")?;
        writeln!(out, "{report}")?;
    } else {
        if let Task::Analysis(analysis) = instructions.last_mut().unwrap() {
            let table = analysis.generate_table(Some(&gb))?;
            writeln!(out, "{table}")?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use analysis_parameter::Grouping;

    use super::*;
}
