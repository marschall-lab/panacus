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
    let params = cli::read_params();

    // set
    if let cli::Params::Growth { threads, .. } | cli::Params::HistOnly { threads, .. } = params {
        if threads > 0 {
            log::info!("running pangenome-growth on {} threads", &threads);
            rayon::ThreadPoolBuilder::new()
                .num_threads(threads)
                .build_global()
                .unwrap();
        } else {
            log::info!("running pangenome-growth using all available CPUs");
        }
    }

    //
    // 1st step: calculation / loading of histogram
    //

    let hist: abacus::Hist = match &params {
        cli::Params::Growth { gfa_file, .. } | cli::Params::HistOnly { gfa_file, .. } => {
            // preprocessing
            let abacus_data = abacus::AbacusData::from_params(&params)?;

            log::info!(
                "..done; found {} paths/walks and {} nodes",
                abacus_data.path_segments.len(),
                abacus_data.node2id.len()
            );

            if abacus_data.path_segments.len() == 0 {
                log::error!("there's nothing to do--graph does not contain any annotated paths (P/W lines), exiting");
                return Ok(());
            }

            // creating the abacus from the gfa
            log::info!("loading graph from {}", gfa_file);
            let mut data = std::io::BufReader::new(fs::File::open(&gfa_file)?);
            let abacus = abacus::Abacus::from_gfa(&mut data, abacus_data);
            log::info!(
                "abacus has {} path groups and {} countables",
                abacus.groups.len(),
                abacus.countable.len()
            );

            // constructing histogram
            log::info!("constructing histogram..");
            abacus::Hist::from_abacus(&abacus)
        }
        cli::Params::GrowthOnly { hist_file, .. } => {
            log::info!("loading coverage histogram from {}", hist_file);
            let mut data = std::io::BufReader::new(fs::File::open(&hist_file)?);
            abacus::Hist::from_tsv(&mut data)
        }
        cli::Params::OrderedGrowth => {
            // XXX
            abacus::Hist {
                ary: Vec::new(),
                groups: Vec::new(),
            }
        }
    };

    //
    // 2nd step: calculation & output of growth curve / output of histogram
    //
    match params {
        cli::Params::Growth { .. } | cli::Params::GrowthOnly { .. } => {
            // XXX
            let hist_data = abacus::HistData::from_params(&params);

            let growth = hist.calc_growth();

            for (i, pang_m) in growth.into_iter().enumerate() {
                writeln!(out, "{}\t{}", i + 1, pang_m)?;
            }
        }
        cli::Params::HistOnly { .. } => {
            hist.to_tsv(&mut out)?;
        }
        cli::Params::OrderedGrowth => {
            unreachable!();
        }
    };

    Ok(())
}
