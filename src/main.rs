/* standard use */
use std::time::Instant;
use std::fs;
use std::io::{BufReader, BufWriter, Write};
use std::io::{Error};

/* external crate */
use rayon::prelude::*;

/* private use */
mod abacus;
mod cli;
mod graph;
mod path;
mod path_parser;
mod hist;
mod html;
mod io;
mod util;

use crate::cli::*;
use crate::abacus::*;
use crate::graph::*;
use crate::path::*;
use crate::path_parser::*;
use crate::hist::*;
use crate::html::*;
use crate::io::*;
use crate::util::*;

fn main() -> Result<(), std::io::Error> {
    env_logger::init();
    let timer = Instant::now();

    // print output to stdout
    let mut out = std::io::BufWriter::new(std::io::stdout());

    // read parameters and store them in memory
    let params = cli::read_params();
    cli::set_number_of_threads(&params);

    // ride on!
    run(params, &mut out)?;

    // clean up & close down
    out.flush()?;
    let duration = timer.elapsed();
    log::info!("done; time elapsed: {:?} ", duration);

    Ok(())
}

pub fn run<W: Write>(params: Params, out: &mut BufWriter<W>) -> Result<(), Error> {

    cli::validate_single_groupby_option(&params)?;

    match params {
        Params::Histgrowth {
            ref gfa_file,
            count,
            output_format,
            ..
        } => {
            //Hist
            let (graph_aux, path_aux, hists) = run_hist(gfa_file, count, output_format, &params)?;
            let stats = get_some_stats_if_output_html(output_format, gfa_file, &graph_aux, &path_aux);
            //Growth
            let hist_aux = HistAuxilliary::from_params(&params)?;
            let growths = run_growth(&hists, &hist_aux);
            //Output
            let out_filename = path_basename(gfa_file);
            match output_format {
                OutputFormat::Table => write_histgrowth_table(&hists, &growths, &hist_aux, out)?,
                OutputFormat::Html => {
                    write_histgrowth_html(&Some(hists), &growths, &hist_aux, &out_filename, None, stats, out)?

                }
            };
        }
        Params::Hist {
            ref gfa_file,
            count,
            output_format,
            ..
        } => {
            let (graph_aux, path_aux, hists) = run_hist(gfa_file, count, output_format, &params)?;

            let stats = get_some_stats_if_output_html(output_format, gfa_file, &graph_aux, &path_aux);

            //Output
            let out_filename = path_basename(&gfa_file);
            match output_format {
                OutputFormat::Table => write_hist_table(&hists, out)?,
                OutputFormat::Html => write_hist_html(&hists, &out_filename, stats, out)?,
            };
        }
        Params::Growth {
            ref hist_file,
            output_format,
            ..
        } => {
            let hists = load_hists(hist_file, out)?;

            let hist_aux = HistAuxilliary::from_params(&params)?;
            let growths = run_growth(&hists, &hist_aux);

            //Output
            let out_filename = path_basename(&hist_file);
            match output_format {
                OutputFormat::Table => write_histgrowth_table(&hists, &growths, &hist_aux, out)?,
                OutputFormat::Html => {
                    write_histgrowth_html(&Some(hists), &growths, &hist_aux, &out_filename, None, None, out)?
                }
            };

        }
        Params::Stats {
            ref gfa_file,
            output_format,
            ..
        } => {
            let stats = run_stats(gfa_file, &params)?;

            //Output
            let out_filename = path_basename(&gfa_file);
            match output_format {
                OutputFormat::Table => write_stats(stats, out)?,
                OutputFormat::Html => write_stats_html(&out_filename, stats, out)?,
            };
        }
        Params::Subset {
            ref gfa_file,
            flt_quorum_min,
            flt_quorum_max,
            flt_length_min,
            flt_length_max,
            ..
        } => {
            let index_edges = false;
            let graph_aux = GraphAuxilliary::from_gfa(gfa_file, index_edges);
            let path_aux = PathAuxilliary::from_params(&params, &graph_aux)?;
            let mut data = bufreader_from_compressed_gfa(gfa_file);
            let abacus = AbacusByTotal::from_gfa(&mut data, &path_aux, &graph_aux, CountType::Node);
            data = bufreader_from_compressed_gfa(gfa_file);

            subset_path_gfa(
                &mut data,
                &abacus,
                &graph_aux,
                flt_quorum_min,
                flt_quorum_max,
                flt_length_min,
                flt_length_max,
            );
            //println!("{}", abacus.countable.len()-1);
        }
        Params::OrderedHistgrowth {
            ref gfa_file,
            count,
            output_format,
            ..
        } => {
            let index_edges: bool = output_format == OutputFormat::Html || count == CountType::Edge || count == CountType::All;
            let graph_aux = GraphAuxilliary::from_gfa(gfa_file, index_edges);
            let path_aux = PathAuxilliary::from_params(&params, &graph_aux)?;
            let mut data = bufreader_from_compressed_gfa(gfa_file);
            let abacus = AbacusByGroup::from_gfa(&mut data, &path_aux, &graph_aux, count, false)?;
            let hist_aux = HistAuxilliary::from_params(&params)?;

            let stats = get_some_stats_if_output_html(output_format, gfa_file, &graph_aux, &path_aux);

            match output_format {
                OutputFormat::Table => write_ordered_histgrowth_table(&abacus, &hist_aux, out)?,
                OutputFormat::Html => {
                    write_ordered_histgrowth_html(&abacus, &hist_aux, &gfa_file, count, stats, out)?;
                }
            }
        }
        Params::Table {
            ref gfa_file,
            count,
            total,
            ..
        } => {
            let index_edges: bool = count == CountType::Edge || count == CountType::All;
            let graph_aux = GraphAuxilliary::from_gfa(gfa_file, index_edges);
            let path_aux = PathAuxilliary::from_params(&params, &graph_aux)?;
            let mut data = bufreader_from_compressed_gfa(gfa_file);
            let abacus = AbacusByGroup::from_gfa(&mut data, &path_aux, &graph_aux, count, total)?;

            abacus.to_tsv(total, out)?;
        }
        //Params::Cdbg {
        //    ref gfa_file, k, ..
        //} => {
        //    let graph_aux = GraphAuxilliary::from_cdbg_gfa(gfa_file, k);
        //    let path_aux = PathAuxilliary::from_params(&params, &graph_aux)?;

        //    let mut hists = Vec::new();
        //    let abaci_node =
        //        AbacusByTotal::abaci_from_gfa(gfa_file, CountType::Node, &graph_aux, &path_aux)?;
        //    let abaci_bp =
        //        AbacusByTotal::abaci_from_gfa(gfa_file, CountType::Bp, &graph_aux, &path_aux)?;
        //    hists.push(Hist::from_abacus(&abaci_node[0], None));
        //    hists.push(Hist::from_abacus(&abaci_bp[0], Some(&graph_aux)));

        //    // k-mers and unimer
        //    let n = hists[0].coverage.len();
        //    let mut kmer: Vec<usize> = vec![0; n];
        //    let mut unimer: Vec<usize> = vec![0; n];

        //    for i in 0..n {
        //        kmer[i] = hists[1].coverage[i] - (k - 1) * hists[0].coverage[i];
        //        unimer[i] = hists[1].coverage[i] - k * hists[0].coverage[i];
        //    }

        //    let mut data = BufReader::new(fs::File::open(&gfa_file)?);
        //    let abaci_infix_eq =
        //        AbacusByTotal::from_cdbg_gfa(&mut data, &path_aux, &graph_aux, k, &unimer);

        //    println!("# infix_eq");
        //    for v in abaci_infix_eq.countable.iter() {
        //        println!("{}", v);
        //    }

        //    println!("# kmer");
        //    for i in 1..kmer.len() {
        //        println!("{}", kmer[i]);
        //    }
        //    write_hist_table(&hists, out)?;
        //}
    }

    Ok(())
}

fn run_hist(
    gfa_file: &str, 
    count: CountType, 
    output_format: OutputFormat,
    params: &Params,
) -> Result<(GraphAuxilliary, PathAuxilliary, Vec<Hist>), Error> 
{
    let index_edges: bool = output_format == OutputFormat::Html || count == CountType::Edge || count == CountType::All;
    let graph_aux = GraphAuxilliary::from_gfa(gfa_file, index_edges);
    let path_aux = PathAuxilliary::from_params(&params, &graph_aux)?;
    let abaci = AbacusByTotal::abaci_from_gfa(gfa_file, count, &graph_aux, &path_aux)?;
    let mut hists = Vec::new();
    for abacus in abaci {
        hists.push(Hist::from_abacus(&abacus, Some(&graph_aux)));
    }

    Ok((graph_aux, path_aux, hists))
}

fn get_some_stats_if_output_html(
    output_format: OutputFormat,
    gfa_file: &str,
    graph_aux: &GraphAuxilliary,
    path_aux: &PathAuxilliary,
) -> Option<Stats> 
{
    if output_format == OutputFormat::Html {
        let stats = get_stats_from_graph_and_paths(gfa_file, graph_aux, path_aux);
        Some(stats)
    } else {
        None
    }
}

fn run_growth(hists: &Vec<Hist>, hist_aux: &HistAuxilliary) -> Vec<(CountType, Vec<Vec<f64>>)> {
    let growths: Vec<(CountType, Vec<Vec<f64>>)> = hists
        .par_iter()
        .map(|h| (h.count, h.calc_all_growths(hist_aux)))
        .collect();
    growths
}

fn load_hists<W: Write>(hist_file: &str, out: &mut BufWriter<W>) -> Result<Vec<Hist>, Error>{
    log::info!("loading coverage histogram from {}", hist_file);
    let mut data = BufReader::new(fs::File::open(&hist_file)?);
    let (coverages, comments) = parse_hists(&mut data)?;
    for c in comments {
        out.write(&c[..])?;
        out.write(b"\n")?;
    }

    let hists: Vec<Hist> = coverages
        .into_iter()
        .map(|(count, coverage)| Hist { count, coverage })
        .collect();

    Ok(hists)
}

fn run_stats(gfa_file: &str, params: &Params) -> Result<Stats, Error> {
    let index_edges = true;
    let graph_aux = GraphAuxilliary::from_gfa(gfa_file, index_edges);

    let path_aux = PathAuxilliary::from_params(&params, &graph_aux)?;
    let stats = get_stats_from_graph_and_paths(gfa_file, &graph_aux, &path_aux);
    Ok(stats)
}

fn get_stats_from_graph_and_paths(gfa_file: &str, graph_aux: &GraphAuxilliary, path_aux: &PathAuxilliary) -> Stats{
    let mut data = bufreader_from_compressed_gfa(gfa_file);
    let (_, _, _, paths_len) =
        parse_gfa_paths_walks(&mut data, path_aux, graph_aux, &CountType::Node);
    graph_aux.stats(&paths_len)
}
