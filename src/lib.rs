/* private use */
pub mod analyses;
mod analysis_parameter;
mod commands;
pub mod graph_broker;
mod html_report;
mod io;
mod util;

use std::{collections::HashSet, io::Write};

use analyses::{growth::Growth, hist::Hist, Analysis, ConstructibleAnalysis, InputRequirement};
use analysis_parameter::AnalysisParameter;
use clap::Command;
use graph_broker::GraphBroker;
use html_report::AnalysisSection;

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

pub fn run_cli() -> Result<(), anyhow::Error> {
    let mut out = std::io::BufWriter::new(std::io::stdout());

    // read parameters and store them in memory
    // let params = cli::read_params();
    // cli::set_number_of_threads(&params);
    let args = Command::new("panacus")
        .subcommand(commands::report::get_subcommand())
        .subcommand(commands::hist::get_subcommand())
        .subcommand(commands::growth::get_subcommand())
        .subcommand(commands::histgrowth::get_subcommand())
        .subcommand_required(true)
        .get_matches();

    let mut instructions = Vec::new();
    let mut shall_write_html = false;
    if let Some(report) = commands::report::get_instructions(&args) {
        shall_write_html = true;
        instructions.extend(report?);
    }
    if let Some(hist) = commands::hist::get_instructions(&args) {
        instructions.extend(hist?);
    }
    if let Some(growth) = commands::growth::get_instructions(&args) {
        instructions.extend(growth?);
    }
    if let Some(histgrowth) = commands::histgrowth::get_instructions(&args) {
        instructions.extend(histgrowth?);
    }

    let instructions = preprocess_instructions(instructions);

    // ride on!
    execute_pipeline(instructions, &mut out, shall_write_html)?;

    // clean up & close down
    out.flush()?;
    Ok(())
}

pub fn preprocess_instructions(instructions: Vec<AnalysisParameter>) -> Vec<AnalysisParameter> {
    instructions
}

pub fn execute_pipeline<W: Write>(
    instructions: Vec<AnalysisParameter>,
    out: &mut std::io::BufWriter<W>,
    shall_write_html: bool,
) -> anyhow::Result<()> {
    if instructions.is_empty() {
        log::warn!("No instructions supplied");
        return Ok(());
    }
    let mut report = Vec::new();
    let mut tasks: Vec<Box<dyn Analysis>> = Vec::new();
    let mut req: HashSet<InputRequirement> = HashSet::new();
    for task_param in instructions {
        match task_param {
            p @ AnalysisParameter::Hist { .. } => {
                let h = Hist::from_parameter(p);
                req.extend(h.get_graph_requirements());
                tasks.push(Box::new(h));
            }
            p @ AnalysisParameter::Growth { .. } => {
                let h = Growth::from_parameter(p);
                req.extend(h.get_graph_requirements());
                tasks.push(Box::new(h));
            }
            _ => {}
        }
    }
    let gb = match req.is_empty() {
        true => None,
        false => Some(GraphBroker::from_gfa_with_view(req).expect("Can create broker")),
    };
    if shall_write_html {
        for mut task in tasks {
            report.extend(task.generate_report_section(gb.as_ref())?);
        }
        let mut registry = handlebars::Handlebars::new();
        let report =
            AnalysisSection::generate_report(report, &mut registry, "<Placeholder Filename>")?;
        writeln!(out, "{report}")?;
    } else {
        let table = tasks.last_mut().unwrap().generate_table(gb.as_ref())?;
        writeln!(out, "{table}")?;
    }
    Ok(())
}
