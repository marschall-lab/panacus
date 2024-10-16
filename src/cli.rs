/* standard crate */
use std::io::{BufWriter, Write};
use std::io::{Error, ErrorKind};
use std::str::FromStr;

/* external crate */
use clap::{crate_version, value_parser, Arg, ArgMatches, Command};
use handlebars::Handlebars;

use crate::analyses::growth::Growth;
use crate::analyses::histgrowth::Histgrowth;
/* private use */
use crate::analyses::info::Info;
use crate::analyses::ordered_histgrowth::OrderedHistgrowth;
use crate::analyses::table::Table;
use crate::analyses::{self, Analysis, AnalysisSection};
use crate::data_manager::DataManager;
use crate::io::OutputFormat;
use crate::util::*;

pub enum RequireThreshold {
    Absolute,
    Relative,
    #[allow(dead_code)]
    Either,
}

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

pub fn parse_threshold_cli(
    threshold_str: &str,
    require: RequireThreshold,
) -> Result<Vec<Threshold>, Error> {
    let mut thresholds = Vec::new();

    for (i, el) in threshold_str.split(',').enumerate() {
        let rel_val = match f64::from_str(el.trim()) {
            Ok(t) => {
                if (0.0..=1.0).contains(&t) {
                    Ok(t)
                } else {
                    Err(Error::new(
                        ErrorKind::InvalidData,
                        format!(
                            "relative threshold \"{}\" ({}. element in list) must be within [0,1].",
                            &threshold_str,
                            i + 1
                        ),
                    ))
                }
            }
            Err(_) => Err(Error::new(
                ErrorKind::InvalidData,
                format!(
                    "threshold \"{}\" ({}. element in list) is required to be float, but isn't.",
                    &threshold_str,
                    i + 1
                ),
            )),
        };

        thresholds.push(
            match require {
                RequireThreshold::Absolute => Threshold::Absolute(usize::from_str(el.trim()).map_err(|_|
                    Error::new(
                            ErrorKind::InvalidData,
                            format!("threshold \"{}\" ({}. element in list) is required to be integer, but isn't.",
                    &threshold_str,
                    i + 1)))?),
            RequireThreshold::Relative => Threshold::Relative(rel_val?),
            RequireThreshold::Either =>
        if let Ok(t) = usize::from_str(el.trim()) {
            Threshold::Absolute(t)
        } else {
            Threshold::Relative(rel_val?)
            }
            }
            );
    }
    Ok(thresholds)
}

// set number of threads can be run only once, otherwise it throws an error of the
// GlobalPoolAlreadyInitialized, which unfortunately is not pub therefore we cannot catch it.
// https://github.com/rayon-rs/rayon/issues/878
// We run this function in the main otherwise in the tests the second time we run the function
// "run" it will crush
pub fn set_number_of_threads(params: &ArgMatches) {
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

fn write_output<T: Analysis, W: Write>(mut analysis: T, dm: &DataManager, out: &mut BufWriter<W>, matches: &ArgMatches) -> Result<(), Error> {
    match matches.get_one("output_format").unwrap() {
        OutputFormat::Table => { analysis.write_table(dm, out)?; },
        OutputFormat::Html => {
            let report = analysis.generate_report_section(dm);
            let mut registry = Handlebars::new();
            let html = AnalysisSection::generate_report(report, &mut registry).unwrap();
            writeln!(out, "{}", html)?;
        }
    };
    Ok(())
} 

pub fn run<W: Write>(out: &mut BufWriter<W>) -> Result<(), Error> {
    let matches = Command::new("")
        .version(crate_version!())
        .author("Luca Parmigiani <lparmig@cebitec.uni-bielefeld.de>, Daniel Doerr <daniel.doerr@hhu.de>")
        .about("Calculate count statistics for pangenomic data")
        .args(&[
            Arg::new("output_format").global(true).help("Choose output format: table (tab-separated-values) or html report").short('o').long("output-format")
            .default_value("table").value_parser(clap_enum_variants!(OutputFormat)).ignore_case(true),
            Arg::new("threads").global(true).short('t').long("threads").help("").default_value("0").value_parser(value_parser!(usize)),
        ])
        .subcommand(Info::get_subcommand())
        .subcommand(analyses::hist::Hist::get_subcommand())
        .subcommand(Histgrowth::get_subcommand())
        .subcommand(OrderedHistgrowth::get_subcommand())
        .subcommand(Table::get_subcommand())
        .subcommand(Growth::get_subcommand())
        .get_matches();

    set_number_of_threads(&matches);
    if let Some((req, view_params, gfa_file)) = Info::get_input_requirements(&matches) {
        let dm = DataManager::from_gfa_with_view(&gfa_file, req, &view_params)?;
        let info = Info::build(&dm, &matches)?;
        write_output(*info, &dm, out, &matches)?;
    } else if let Some((req, view_params, gfa_file)) =
        crate::analyses::hist::Hist::get_input_requirements(&matches)
    {
        let dm = DataManager::from_gfa_with_view(&gfa_file, req, &view_params)?;
        let hist = analyses::hist::Hist::build(&dm, &matches)?;
        write_output(*hist, &dm, out, &matches)?;
    } else if let Some((req, view_params, gfa_file)) = Histgrowth::get_input_requirements(&matches)
    {
        let dm = DataManager::from_gfa_with_view(&gfa_file, req, &view_params)?;
        let histgrowth = Histgrowth::build(&dm, &matches)?;
        write_output(*histgrowth, &dm, out, &matches)?;
    } else if let Some((req, view_params, gfa_file)) =
        OrderedHistgrowth::get_input_requirements(&matches)
    {
        let dm = DataManager::from_gfa_with_view(&gfa_file, req, &view_params)?;
        let ordered_histgrowth = OrderedHistgrowth::build(&dm, &matches)?;
        write_output(*ordered_histgrowth, &dm, out, &matches)?;
    } else if let Some((req, view_params, gfa_file)) = Table::get_input_requirements(&matches) {
        let dm = DataManager::from_gfa_with_view(&gfa_file, req, &view_params)?;
        let table = Table::build(&dm, &matches)?;
        write_output(*table, &dm, out, &matches)?;
    } else if matches.subcommand_matches("growth").is_some() {
        let dm = DataManager::new();
        let growth = Growth::build(&dm, &matches)?;
        write_output(*growth, &dm, out, &matches)?;
    }

    //match params {
    //    Params::Histgrowth {
    //        ref gfa_file,
    //        count,
    //        output_format,
    //        ..
    //    } => {
    //        //Hist
    //        let graph_aux = match output_format {
    //            OutputFormat::Html => GraphAuxilliary::from_gfa(gfa_file, CountType::All),
    //            _ => GraphAuxilliary::from_gfa(gfa_file, count),
    //        };
    //        let abacus_aux = AbacusAuxilliary::from_params(&params, &graph_aux)?;
    //        let abaci = AbacusByTotal::abaci_from_gfa(gfa_file, count, &graph_aux, &abacus_aux)?;
    //        let mut hists = Vec::new();
    //        for abacus in abaci {
    //            hists.push(Hist::from_abacus(&abacus, Some(&graph_aux)));
    //        }
    //        //Growth
    //        let hist_aux = HistAuxilliary::from_params(&params)?;
    //        let filename = Path::new(&gfa_file).file_name().unwrap().to_str().unwrap();
    //        let growths: Vec<(CountType, Vec<Vec<f64>>)> = hists
    //            .par_iter()
    //            .map(|h| (h.count, h.calc_all_growths(&hist_aux)))
    //            .collect();
    //        log::info!("reporting histgrowth table");
    //        match output_format {
    //            OutputFormat::Table => write_histgrowth_table(&hists, &growths, &hist_aux, out)?,
    //            OutputFormat::Html => {
    //                let mut data = bufreader_from_compressed_gfa(gfa_file);
    //                let (_, _, _, paths_len) =
    //                    parse_gfa_paths_walks(&mut data, &abacus_aux, &graph_aux, &CountType::Node);
    //
    //                //let info = graph_aux.info(&paths_len, &abacus_aux.groups, true);
    //                write_histgrowth_html(
    //                    &Some(hists),
    //                    &growths,
    //                    &hist_aux,
    //                    filename,
    //                    None,
    //                    None,
    //                    out,
    //                )?
    //            }
    //        };
    //    }
    //    Params::Hist {
    //        ref gfa_file,
    //        count,
    //        output_format,
    //        ..
    //    } => {
    //        let graph_aux = match output_format {
    //            OutputFormat::Html => GraphAuxilliary::from_gfa(gfa_file, CountType::All),
    //            _ => GraphAuxilliary::from_gfa(gfa_file, count),
    //        };
    //        let abacus_aux = AbacusAuxilliary::from_params(&params, &graph_aux)?;
    //        let abaci = AbacusByTotal::abaci_from_gfa(gfa_file, count, &graph_aux, &abacus_aux)?;
    //        let mut hists = Vec::new();
    //        for abacus in abaci {
    //            hists.push(Hist::from_abacus(&abacus, Some(&graph_aux)));
    //        }
    //
    //        let filename = Path::new(&gfa_file).file_name().unwrap().to_str().unwrap();
    //        match output_format {
    //            OutputFormat::Table => write_hist_table(&hists, out)?,
    //            OutputFormat::Html => {
    //                let mut data = bufreader_from_compressed_gfa(gfa_file);
    //                let (_, _, _, paths_len) =
    //                    parse_gfa_paths_walks(&mut data, &abacus_aux, &graph_aux, &CountType::Node);
    //
    //                //let info = graph_aux.info(&paths_len, &abacus_aux.groups, true);
    //                write_hist_html(&hists, filename, None, out)?
    //            }
    //        };
    //    }
    //    Params::Growth {
    //        ref hist_file,
    //        output_format,
    //        ..
    //    } => {
    //        let hist_aux = HistAuxilliary::from_params(&params)?;
    //        log::info!("loading coverage histogram from {}", hist_file);
    //        let mut data = BufReader::new(fs::File::open(hist_file)?);
    //        let (coverages, comments) = parse_hists(&mut data)?;
    //        for c in comments {
    //            out.write_all(&c[..])?;
    //            out.write_all(b"\n")?;
    //        }
    //        let hists: Vec<Hist> = coverages
    //            .into_iter()
    //            .map(|(count, coverage)| Hist { count, coverage })
    //            .collect();
    //
    //        let filename = Path::new(&hist_file).file_name().unwrap().to_str().unwrap();
    //        let growths: Vec<(CountType, Vec<Vec<f64>>)> = hists
    //            .par_iter()
    //            .map(|h| (h.count, h.calc_all_growths(&hist_aux)))
    //            .collect();
    //        log::info!("reporting histgrowth table");
    //        match output_format {
    //            OutputFormat::Table => write_histgrowth_table(&hists, &growths, &hist_aux, out)?,
    //            OutputFormat::Html => write_histgrowth_html(
    //                &Some(hists),
    //                &growths,
    //                &hist_aux,
    //                filename,
    //                None,
    //                None,
    //                out,
    //            )?,
    //        };
    //    }
    //    Params::Info {
    //        ref gfa_file,
    //        groupby,
    //        output_format,
    //        ..
    //    } => {
    //        let req = Info::get_input_requirements();
    //        let dm = DataManager::from_gfa(gfa_file, req).finish()?;
    //        let mut info = Info::build(&dm);
    //        let table = info.generate_table();
    //        write_text(&table, out)?
    //        // let graph_aux = GraphAuxilliary::from_gfa(gfa_file, CountType::All);
    //
    //        // let abacus_aux = AbacusAuxilliary::from_params(&params, &graph_aux)?;
    //        // let mut data = bufreader_from_compressed_gfa(gfa_file);
    //        // let (_, _, _, paths_len) =
    //        //     parse_gfa_paths_walks(&mut data, &abacus_aux, &graph_aux, &CountType::Node);
    //
    //        // match output_format {
    //        //     OutputFormat::Table => {
    //        //         let has_groups = match params {
    //        //             Params::Info {
    //        //                 ref groupby,
    //        //                 groupby_haplotype,
    //        //                 groupby_sample,
    //        //                 ..
    //        //             } => !groupby.is_empty() || groupby_haplotype || groupby_sample,
    //        //             _ => false,
    //        //         };
    //        //         let info = graph_aux.info(&paths_len, &abacus_aux.groups, has_groups);
    //        //         write_info(info, out)?
    //        //     }
    //        //     OutputFormat::Html => {
    //        //         let info = graph_aux.info(&paths_len, &abacus_aux.groups, true);
    //        //         let filename = Path::new(&gfa_file).file_name().unwrap().to_str().unwrap();
    //        //         write_info_html(filename, info, out)?
    //        //     }
    //        // };
    //    }
    //    Params::OrderedHistgrowth {
    //        ref gfa_file,
    //        count,
    //        output_format,
    //        ..
    //    } => {
    //        let graph_aux = match output_format {
    //            OutputFormat::Html => GraphAuxilliary::from_gfa(gfa_file, CountType::All),
    //            _ => GraphAuxilliary::from_gfa(gfa_file, count),
    //        };
    //        let abacus_aux = AbacusAuxilliary::from_params(&params, &graph_aux)?;
    //        let mut data = bufreader_from_compressed_gfa(gfa_file);
    //        let abacus = AbacusByGroup::from_gfa(&mut data, &abacus_aux, &graph_aux, count, true)?;
    //        let hist_aux = HistAuxilliary::from_params(&params)?;
    //        match output_format {
    //            OutputFormat::Table => {
    //                write_ordered_histgrowth_table(&abacus, &hist_aux, out)?;
    //            }
    //            OutputFormat::Html => {
    //                let mut data = bufreader_from_compressed_gfa(gfa_file);
    //                let (_, _, _, paths_len) =
    //                    parse_gfa_paths_walks(&mut data, &abacus_aux, &graph_aux, &CountType::Node);
    //
    //                // let info = graph_aux.info(&paths_len, &abacus_aux.groups, true);
    //                write_ordered_histgrowth_html(&abacus, &hist_aux, gfa_file, count, None, out)?;
    //            }
    //        }
    //    }
    //    Params::Table {
    //        ref gfa_file,
    //        count,
    //        total,
    //        ..
    //    } => {
    //        let graph_aux = GraphAuxilliary::from_gfa(gfa_file, count);
    //        let abacus_aux = AbacusAuxilliary::from_params(&params, &graph_aux)?;
    //        let mut data = BufReader::new(fs::File::open(gfa_file)?);
    //        let abacus = AbacusByGroup::from_gfa(&mut data, &abacus_aux, &graph_aux, count, total)?;
    //
    //        abacus.to_tsv(total, out)?;
    //    } //Params::Cdbg {
    //      //    ref gfa_file, k, ..
    //      //} => {
    //      //    let graph_aux = GraphAuxilliary::from_cdbg_gfa(gfa_file, k);
    //      //    let abacus_aux = AbacusAuxilliary::from_params(&params, &graph_aux)?;
    //
    //      //    let mut hists = Vec::new();
    //      //    let abaci_node =
    //      //        AbacusByTotal::abaci_from_gfa(gfa_file, CountType::Node, &graph_aux, &abacus_aux)?;
    //      //    let abaci_bp =
    //      //        AbacusByTotal::abaci_from_gfa(gfa_file, CountType::Bp, &graph_aux, &abacus_aux)?;
    //      //    hists.push(Hist::from_abacus(&abaci_node[0], None));
    //      //    hists.push(Hist::from_abacus(&abaci_bp[0], Some(&graph_aux)));
    //
    //      //    // k-mers and unimer
    //      //    let n = hists[0].coverage.len();
    //      //    let mut kmer: Vec<usize> = vec![0; n];
    //      //    let mut unimer: Vec<usize> = vec![0; n];
    //
    //      //    for i in 0..n {
    //      //        kmer[i] = hists[1].coverage[i] - (k - 1) * hists[0].coverage[i];
    //      //        unimer[i] = hists[1].coverage[i] - k * hists[0].coverage[i];
    //      //    }
    //
    //      //    let mut data = BufReader::new(fs::File::open(&gfa_file)?);
    //      //    let abaci_infix_eq =
    //      //        AbacusByTotal::from_cdbg_gfa(&mut data, &abacus_aux, &graph_aux, k, &unimer);
    //
    //      //    println!("# infix_eq");
    //      //    for v in abaci_infix_eq.countable.iter() {
    //      //        println!("{}", v);
    //      //    }
    //
    //      //    println!("# kmer");
    //      //    for i in 1..kmer.len() {
    //      //        println!("{}", kmer[i]);
    //      //    }
    //      //    write_hist_table(&hists, out)?;
    //      //}
    //}

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_threshold_cli_relative_success() {
        let threshold_str = "0.2,0.5,0.9";
        let result = parse_threshold_cli(threshold_str, RequireThreshold::Relative);
        assert!(result.is_ok());
        let thresholds = result.unwrap();
        assert_eq!(thresholds.len(), 3);
        assert_eq!(thresholds[0], Threshold::Relative(0.2));
        assert_eq!(thresholds[1], Threshold::Relative(0.5));
        assert_eq!(thresholds[2], Threshold::Relative(0.9));
    }

    #[test]
    fn test_parse_threshold_cli_absolute_success() {
        let threshold_str = "5,10,15";
        let result = parse_threshold_cli(threshold_str, RequireThreshold::Absolute);
        assert!(result.is_ok());
        let thresholds = result.unwrap();
        assert_eq!(thresholds.len(), 3);
        assert_eq!(thresholds[0], Threshold::Absolute(5));
        assert_eq!(thresholds[1], Threshold::Absolute(10));
        assert_eq!(thresholds[2], Threshold::Absolute(15));
    }

    #[test]
    fn test_parse_threshold_cli_invalid_float_in_absolute() {
        let threshold_str = "5.5,10,15";
        let result = parse_threshold_cli(threshold_str, RequireThreshold::Absolute);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().kind(), ErrorKind::InvalidData);
    }

    #[test]
    fn test_parse_threshold_cli_invalid_value_in_relative() {
        let threshold_str = "0.2,1.2,0.9"; // 1.2 is out of range for relative threshold
        let result = parse_threshold_cli(threshold_str, RequireThreshold::Relative);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().kind(), ErrorKind::InvalidData);
    }
}
