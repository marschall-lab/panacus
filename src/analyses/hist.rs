use std::collections::HashSet;

use clap::{arg, value_parser, Arg, Command};

use crate::{abacus::ViewParams, clap_enum_variants, io::OutputFormat, util::CountType};

use super::{Analysis, ReportSection};

pub struct Hist { 
}

impl Analysis for Hist {
    fn build(dm: &crate::data_manager::DataManager) -> Self {
        Self { 
        }
    }

    fn generate_table(&mut self, dm: &crate::data_manager::DataManager) -> String {
        dm.get_hist()
    }

    fn generate_report_section(&mut self, dm: &crate::data_manager::DataManager) -> super::ReportSection {
        ReportSection { }
    }

    fn get_subcommand() -> Command {
        Command::new("hist")
            .about("Calculate coverage histogram")
            .args(&[
                arg!(gfa_file: <GFA_FILE> "graph in GFA1 format, accepts also compressed (.gz) file"),
                arg!(-s --subset <FILE> "Produce counts by subsetting the graph to a given list of paths (1-column list) or path coordinates (3- or 12-column BED file)"),
                arg!(-e --exclude <FILE> "Exclude bp/node/edge in growth count that intersect with paths (1-column list) or path coordinates (3- or 12-column BED-file) provided by the given file; all intersecting bp/node/edge will be exluded also in other paths not part of the given list"),
                arg!(-g --groupby <FILE> "Merge counts from paths by path-group mapping from given tab-separated two-column file"),
                arg!(-H --"groupby-haplotype" "Merge counts from paths belonging to same haplotype"),
                arg!(-S --"groupby-sample" "Merge counts from paths belonging to same sample"),
                Arg::new("output_format").help("Choose output format: table (tab-separated-values) or html report").short('o').long("output-format")
                .default_value("table").value_parser(clap_enum_variants!(OutputFormat)).ignore_case(true),
                Arg::new("threads").short('t').long("threads").help("").default_value("0").value_parser(value_parser!(usize)),
            ])
    }

    fn get_input_requirements(
            matches: &clap::ArgMatches,
        ) -> Option<(HashSet<super::InputRequirement>, ViewParams, String)> {
        let matches = matches.subcommand_matches("hist")?;
        let req = HashSet::new();
        let view = ViewParams {
            groupby: matches
                .get_one::<String>("groupby")
                .cloned()
                .unwrap_or_default(),
            groupby_haplotype: matches.get_flag("groupby-haplotype"),
            groupby_sample: matches.get_flag("groupby-sample"),
            positive_list: matches
                .get_one::<String>("subset")
                .cloned()
                .unwrap_or_default(),
            negative_list: matches
                .get_one::<String>("exclude")
                .cloned()
                .unwrap_or_default(),
            order: None,
        };
        let file_name = matches.get_one::<String>("gfa_file")?.to_owned();
        log::debug!("input params: {:?}, {:?}, {:?}", req, view, file_name);
        Some((req, view, file_name))
    }
}
