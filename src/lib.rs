/* private use */
pub mod analyses;
mod analysis_parameter;
mod commands;
pub mod graph_broker;
mod html_report;
mod io;
mod util;

use std::io::Write;

use analyses::{hist::Hist, Analysis};
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

    let instructions = preprocess_instructions(instructions);

    // ride on!
    execute_pipeline(instructions, &mut out, shall_write_html);

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
) {
    if instructions.is_empty() {
        log::warn!("No instructions supplied");
        return;
    }
    let mut report = Vec::new();
    for task_param in instructions {
        match task_param {
            p @ AnalysisParameter::Hist { .. } => {
                let mut h = Hist::from_parameter(p);
                let req = h.get_graph_requirements();
                let gb = GraphBroker::from_gfa_with_view(req).expect("Can create broker");
                if shall_write_html {
                    report.extend(h.generate_report_section(&gb));
                } else {
                    h.write_table(&gb, out).expect("Can write output");
                }
            }
            _ => {}
        }
    }
    if shall_write_html {
        let mut registry = handlebars::Handlebars::new();
        let report =
            AnalysisSection::generate_report(report, &mut registry, "<Placeholder Filename>")
                .expect("Can generate report");
        writeln!(out, "{report}").expect("Can write html");
    }
}
