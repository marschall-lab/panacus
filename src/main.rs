/* standard use */
use std::fs;
use std::io::prelude::*;
use std::time::Instant;

/* private use */
mod abacus;
mod cli;
mod graph;
mod io;

fn main() -> Result<(), std::io::Error> {
    env_logger::init();
    let timer = Instant::now();

    // print output to stdout
    let mut out = std::io::BufWriter::new(std::io::stdout());

    // read parameters and store them in memory
    let params = cli::read_params().unwrap();

    match params {
        cli::Params::Growth
           {
            gfa_file,
            count,
            groups,
            subset_coords,
            exclude_coords,
            intersection,
            coverage,
        } => {
            let hist = calc_hist(gfa_file, count, groups, subset_coords, exclude_coords)?;

            log::info!("constructing pangenome growth");
            abacus::Abacus::hist2pangrowth(hist);

            out.flush()?;
            let duration = timer.elapsed();
            log::info!("done; time elapsed: {:?} ", duration);
        },

        cli::Params::Hist {
            gfa_file,
            count,
            groups,
            subset_coords,
            exclude_coords,
        } => {
            let hist = calc_hist(gfa_file, count, groups, subset_coords, exclude_coords)?;
            let duration = timer.elapsed();
            log::info!("done; time elapsed: {:?} ", duration);
        }
    };

    Ok(())
}

fn calc_hist(
        gfa_file: String,
        count: cli::CountType,
        groups: Option<Vec<(graph::PathSegment, String)>>,
        subset_coords: Option<Vec<graph::PathSegment>>,
        exclude_coords: Option<Vec<graph::PathSegment>>) -> Result<Vec<u32>,std::io::Error> {

        // preprocessing
        log::info!("preprocessing: processing nodes and counting P/W lines");
        let mut data = std::io::BufReader::new(fs::File::open(&gfa_file)?);
        let prep: abacus::Prep = io::preprocessing(&mut data, groups, subset_coords);
        log::info!(
            "..done; found {} paths/walks and {} nodes",
            prep.path_segments.len(),
            prep.node2id.len()
        );

        // creating the abacus from the gfa
        log::info!("loading graph from {}", gfa_file);
        let mut data = std::io::BufReader::new(fs::File::open(&gfa_file)?);
        let abacus = abacus::Abacus::from_gfa(&mut data, prep);
        log::info!(
            "abacus has {} path groups and {} countables",
            abacus.groups.len(),
            abacus.countable.len()
        );

        // constructing histogram
        log::info!("constructing histogram..");
        Ok(abacus.construct_hist())
}

