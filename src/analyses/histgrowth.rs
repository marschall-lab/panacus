use std::{collections::HashSet, io::{BufWriter, Error}};
use std::io::Write;

use clap::{arg, value_parser, Arg, Command};
use rayon::iter::{ParallelBridge, ParallelIterator};

use crate::{analyses::InputRequirement, data_manager::{HistAuxilliary, ViewParams}, io::write_table, util::CountType};
use crate::{clap_enum_variants, io::OutputFormat};

use super::{Analysis, ReportSection};

pub struct Histgrowth {
    growths: Vec<(CountType, Vec<Vec<f64>>)>,
    hist_aux: HistAuxilliary,
}

impl Analysis for Histgrowth {
    fn build(dm: &crate::data_manager::DataManager, matches: &clap::ArgMatches) -> Result<Box<Self>, Error> {
        let matches = matches.subcommand_matches("histgrowth").unwrap();
        let coverage = matches.get_one::<String>("coverage").cloned().unwrap();
        let quorum = matches.get_one::<String>("quorum").cloned().unwrap();
        let hist_aux = HistAuxilliary::parse_params(&quorum, &coverage)?;
        let growths: Vec<_> = dm.get_hists()
            .values()
            .par_bridge()
            .map(|h| (h.count, h.calc_all_growths(&hist_aux)))
            .collect();
        Ok(Box::new(Self {
            growths,
            hist_aux
        }))
    }

    fn write_table<W: Write>(&mut self, dm: &crate::data_manager::DataManager, out: &mut BufWriter<W>) -> Result<(), Error> {
        log::info!("reporting hist table");
        writeln!(
            out,
            "# {}",
            std::env::args().collect::<Vec<String>>().join(" ")
        )?;

        let mut header_cols = vec![vec![
            "panacus".to_string(),
            "count".to_string(),
            "coverage".to_string(),
            "quorum".to_string(),
        ]];
        let mut output_columns: Vec<Vec<f64>> = Vec::new();

        for h in dm.get_hists().values() {
            output_columns.push(h.coverage.iter().map(|x| *x as f64).collect());
            header_cols.push(vec![
                "hist".to_string(),
                h.count.to_string(),
                String::new(),
                String::new(),
            ])
        }

        for (count, g) in &self.growths {
            output_columns.extend(g.clone());
            let m = self.hist_aux.coverage.len();
            header_cols.extend(
                std::iter::repeat("growth")
                    .take(m)
                    .zip(std::iter::repeat(count).take(m))
                    .zip(self.hist_aux.coverage.iter())
                    .zip(&self.hist_aux.quorum)
                    .map(|(((p, t), c), q)| {
                        vec![p.to_string(), t.to_string(), c.get_string(), q.get_string()]
                    }),
            );
        }
        write_table(&header_cols, &output_columns, out)
    }

    fn generate_report_section(&mut self, _dm: &crate::data_manager::DataManager) -> super::ReportSection {
        ReportSection { }
    }

    fn get_subcommand() -> Command {
        Command::new("histgrowth")
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
                Arg::new("count").help("Graph quantity to be counted").default_value("node").ignore_case(true).short('c').long("count").value_parser(clap_enum_variants!(CountType)),
                Arg::new("coverage").help("Ignore all countables with a coverage lower than the specified threshold. The coverage of a countable corresponds to the number of path/walk that contain it. Repeated appearances of a countable in the same path/walk are counted as one. You can pass a comma-separated list of coverage thresholds, each one will produce a separated growth curve (e.g., --coverage 2,3). Use --quorum to set a threshold in conjunction with each coverage (e.g., --quorum 0.5,0.9)")
                    .short('l').long("coverage").default_value("1"),
                Arg::new("quorum").help("Unlike the --coverage parameter, which specifies a minimum constant number of paths for all growth point m (1 <= m <= num_paths), --quorum adjust the threshold based on m. At each m, a countable is counted in the average growth if the countable is contained in at least floor(m*quorum) paths. Example: A quorum of 0.9 requires a countable to be in 90% of paths for each subset size m. At m=10, it must appear in at least 9 paths. At m=100, it must appear in at least 90 paths. A quorum of 1 (100%) requires presence in all paths of the subset, corresponding to the core. Default: 0, a countable counts if it is present in any path at each growth point. Specify multiple quorum values with a comma-separated list (e.g., --quorum 0.5,0.9). Use --coverage to set static path thresholds in conjunction with variable quorum percentages (e.g., --coverage 5,10).")
                    .short('q').long("quorum").default_value("0"),
                Arg::new("threads").short('t').long("threads").help("").default_value("0").value_parser(value_parser!(usize)),
            ])
    }

    fn get_input_requirements(
            matches: &clap::ArgMatches,
        ) -> Option<(HashSet<super::InputRequirement>, ViewParams, String)> {
        let matches = matches.subcommand_matches("histgrowth")?;
        let mut req = HashSet::from([
            InputRequirement::Hist
        ]);
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

impl Histgrowth {
    fn count_to_input_req(count: CountType) -> HashSet<InputRequirement> {
        match count {
            CountType::Bp => HashSet::from([InputRequirement::Bp]),
            CountType::Node => HashSet::from([InputRequirement::Node]),
            CountType::Edge => HashSet::from([InputRequirement::Edge]),
            CountType::All => HashSet::from([
                InputRequirement::Bp,
                InputRequirement::Node,
                InputRequirement::Edge
            ]),
        }
    }
}
