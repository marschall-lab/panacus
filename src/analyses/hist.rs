use std::io::Write;
use std::{
    collections::HashSet,
    io::{BufWriter, Error},
};

use clap::{arg, Arg, ArgMatches, Command};

use crate::analyses::{AnalysisTab, ReportItem};
use crate::clap_enum_variants;
use crate::{
    analyses::InputRequirement, data_manager::ViewParams, io::write_table, util::CountType,
};

use super::{Analysis, AnalysisSection};

pub struct Hist {}

impl Analysis for Hist {
    fn build(
        _dm: &crate::data_manager::DataManager,
        _matches: &ArgMatches,
    ) -> Result<Box<Self>, Error> {
        Ok(Box::new(Self {}))
    }

    fn write_table<W: Write>(
        &mut self,
        dm: &crate::data_manager::DataManager,
        out: &mut BufWriter<W>,
    ) -> Result<(), Error> {
        log::info!("reporting hist table");
        writeln!(
            out,
            "# {}",
            std::env::args().collect::<Vec<String>>().join(" ")
        )?;

        let mut header_cols = vec![vec![
            "panacus".to_string(),
            "count".to_string(),
            String::new(),
            String::new(),
        ]];
        let mut output_columns = Vec::new();
        for h in dm.get_hists().values() {
            output_columns.push(h.coverage.iter().map(|x| *x as f64).collect());
            header_cols.push(vec![
                "hist".to_string(),
                h.count.to_string(),
                String::new(),
                String::new(),
            ])
        }
        write_table(&header_cols, &output_columns, out)
    }

    fn generate_report_section(
        &mut self,
        dm: &crate::data_manager::DataManager,
    ) -> Vec<AnalysisSection> {
        let mut buf = BufWriter::new(Vec::new());
        self.write_table(dm, &mut buf).expect("Can write to string");
        let bytes = buf.into_inner().unwrap();
        let table = String::from_utf8(bytes).unwrap();
        let table = format!("`{}`", &table);
        let histogram_tabs = dm
            .get_hists()
            .iter()
            .map(|(k, v)| AnalysisTab {
                id: format!("tab-cov-hist-{}", k),
                name: k.to_string(),
                is_first: false,
                items: vec![ReportItem::Bar {
                    id: format!("cov-hist-{}", k),
                    name: dm.get_fname(),
                    x_label: "taxa".to_string(),
                    y_label: format!("#{}s", k),
                    labels: (0..v.coverage.len()).map(|s| s.to_string()).collect(),
                    values: v.coverage.iter().map(|c| *c as f64).collect(),
                    log_toggle: true,
                }],
            })
            .collect::<Vec<_>>();
        vec![AnalysisSection {
            name: "coverage histogram".to_string(),
            id: "coverage-histogram".to_string(),
            is_first: true,
            table: Some(table),
            tabs: histogram_tabs,
        }
        .set_first()]
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
                Arg::new("count").help("Graph quantity to be counted").default_value("node").ignore_case(true).short('c').long("count").value_parser(clap_enum_variants!(CountType)),
            ])
    }

    fn get_input_requirements(
        matches: &clap::ArgMatches,
    ) -> Option<(HashSet<super::InputRequirement>, ViewParams, String)> {
        let matches = matches.subcommand_matches("hist")?;
        let mut req = HashSet::from([InputRequirement::Hist]);
        let count = matches.get_one::<CountType>("count").cloned().unwrap();
        req.extend(Self::count_to_input_req(count));
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

impl Hist {
    fn count_to_input_req(count: CountType) -> HashSet<InputRequirement> {
        match count {
            CountType::Bp => HashSet::from([InputRequirement::Bp]),
            CountType::Node => HashSet::from([InputRequirement::Node]),
            CountType::Edge => HashSet::from([InputRequirement::Edge]),
            CountType::All => HashSet::from([
                InputRequirement::Bp,
                InputRequirement::Node,
                InputRequirement::Edge,
            ]),
        }
    }
}