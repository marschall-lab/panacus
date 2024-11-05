/* private use */
// mod abacus;
pub mod analyses;
// mod cli;
pub mod graph_broker;
mod html_report;
// mod graph;
// mod hist;
// mod html;
mod analysis_parameter;
mod io;
mod util;

use std::io::Write;

use analyses::{hist::Hist, Analysis};
use analysis_parameter::AnalysisParameter;
use graph_broker::GraphBroker;
use html_report::AnalysisSection;

pub fn run_cli() -> Result<(), std::io::Error> {
    let mut out = std::io::BufWriter::new(std::io::stdout());

    // read parameters and store them in memory
    // let params = cli::read_params();
    // cli::set_number_of_threads(&params);

    // ride on!
    execute_pipeline(vec![AnalysisParameter::Hist {
        name: None,
        count_type: util::CountType::Node,
        graph: "../simple_files/simple_graphs/t_groups.gfa".to_string(),
        display: true,
        subset: None,
    }]);

    // clean up & close down
    out.flush()?;
    Ok(())
}

pub fn execute_pipeline(instructions: Vec<AnalysisParameter>) {
    let mut report = Vec::new();
    for task_param in instructions {
        match task_param {
            p @ AnalysisParameter::Hist { .. } => {
                let mut h = Hist::from_parameter(p);
                let req = h.get_graph_requirements();
                let gb = GraphBroker::from_gfa_with_view(req).expect("Can create broker");
                report.extend(h.generate_report_section(&gb));
            }
            _ => {}
        }
    }
    let mut registry = handlebars::Handlebars::new();
    let report = AnalysisSection::generate_report(report, &mut registry, "<Placeholder Filename>")
        .expect("Can generate report");
    println!("{report}");
}
