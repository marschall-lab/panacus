/* standard use */
use std::io::Write;
use std::time::Instant;

/* private use */
mod abacus;
mod cli;
mod graph;
mod hist;
mod html;
mod io;
mod util;

fn main() -> Result<(), std::io::Error> {
    env_logger::init();
    let timer = Instant::now();

    // print output to stdout
    let mut out = std::io::BufWriter::new(std::io::stdout());

    // read parameters and store them in memory
    let params = cli::read_params();

    // ride on!
    cli::run(params, &mut out)?;

    // clean up & close down
    out.flush()?;
    let duration = timer.elapsed();
    log::info!("done; time elapsed: {:?} ", duration);

    Ok(())
}
