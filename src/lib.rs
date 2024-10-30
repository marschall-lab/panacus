/* private use */
// mod abacus;
pub mod analyses;
mod cli;
pub mod graph_broker;
mod html_report;
// mod graph;
// mod hist;
// mod html;
mod io;
mod util;

use std::io::Write;

pub fn run_cli() -> Result<(), std::io::Error> {
    let mut out = std::io::BufWriter::new(std::io::stdout());

    // read parameters and store them in memory
    // let params = cli::read_params();
    // cli::set_number_of_threads(&params);

    // ride on!
    cli::run(&mut out)?;

    // clean up & close down
    out.flush()?;
    Ok(())
}
