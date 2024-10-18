use std::io::Write;
use std::{
    collections::HashSet,
    fs,
    io::{BufReader, BufWriter, Error},
};

use clap::{arg, Arg, Command};
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};

use crate::analyses::{AnalysisTab, ReportItem};
use crate::data_manager::Hist;
use crate::{
    data_manager::{HistAuxilliary, ViewParams},
    io::{parse_hists, write_table},
    util::CountType,
};

use super::{Analysis, AnalysisSection};

pub struct Growth {
    growths: Vec<(CountType, Vec<Vec<f64>>)>,
    comments: Vec<Vec<u8>>,
    hist_aux: HistAuxilliary,
    hists: Vec<Hist>,
}

impl Analysis for Growth {
    fn build(
        _dm: &crate::data_manager::DataManager,
        matches: &clap::ArgMatches,
    ) -> Result<Box<Self>, Error> {
        let matches = matches.subcommand_matches("growth").unwrap();
        let coverage = matches.get_one::<String>("coverage").cloned().unwrap();
        let quorum = matches.get_one::<String>("quorum").cloned().unwrap();
        let hist_aux = HistAuxilliary::parse_params(&quorum, &coverage)?;
        let hist_file = matches
            .get_one::<String>("hist_file")
            .cloned()
            .unwrap_or_default();
        log::info!("loading coverage histogram from {}", hist_file);
        let mut data = BufReader::new(fs::File::open(&hist_file)?);
        let (coverages, comments) = parse_hists(&mut data)?;
        let hists: Vec<Hist> = coverages
            .into_iter()
            .map(|(count, coverage)| Hist { count, coverage })
            .collect();
        let growths: Vec<(CountType, Vec<Vec<f64>>)> = hists
            .par_iter()
            .map(|h| (h.count, h.calc_all_growths(&hist_aux)))
            .collect();
        Ok(Box::new(Self {
            growths,
            comments,
            hist_aux,
            hists,
        }))
    }

    fn write_table<W: Write>(
        &mut self,
        _dm: &crate::data_manager::DataManager,
        out: &mut BufWriter<W>,
    ) -> Result<(), Error> {
        log::info!("reporting hist table");
        for c in &self.comments {
            out.write_all(&c[..])?;
            out.write_all(b"\n")?;
        }
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

        for h in &self.hists {
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

    fn generate_report_section(
        &mut self,
        _dm: &crate::data_manager::DataManager,
    ) -> Vec<AnalysisSection> {
        let growth_labels = (0..self.hist_aux.coverage.len())
            .map(|i| {
                format!(
                    "coverage ≥ {}, quorum ≥ {}%",
                    self.hist_aux.coverage[i].get_string(),
                    self.hist_aux.quorum[i].get_string()
                )
            })
            .collect::<Vec<_>>();
        let growth_tabs = self
            .growths
            .iter()
            .map(|(k, v)| AnalysisTab {
                id: format!("tab-pan-growth-{}", k),
                name: k.to_string(),
                is_first: false,
                items: vec![ReportItem::MultiBar {
                    id: format!("pan-growth-{}", k.to_string()),
                    names: growth_labels.clone(),
                    x_label: "taxa".to_string(),
                    y_label: format!("#{}s", k.to_string()),
                    labels: (1..v[0].len()).map(|i| i.to_string()).collect(),
                    values: v.clone(),
                    log_toggle: false,
                }],
            })
            .collect();
        vec![
            AnalysisSection {
                name: "pangenome growth".to_string(),
                id: "pangenome-growth".to_string(),
                is_first: false,
                tabs: growth_tabs,
            }
            .set_first(),
        ]
    }

    fn get_subcommand() -> Command {
        Command::new("growth")
            .about("Calculate growth curve from coverage histogram")
            .args(&[
                arg!(hist_file: <HIST_FILE> "Coverage histogram as tab-separated value (tsv) file"),
                arg!(-a --hist "Also include histogram in output"),
                Arg::new("coverage").help("Ignore all countables with a coverage lower than the specified threshold. The coverage of a countable corresponds to the number of path/walk that contain it. Repeated appearances of a countable in the same path/walk are counted as one. You can pass a comma-separated list of coverage thresholds, each one will produce a separated growth curve (e.g., --coverage 2,3). Use --quorum to set a threshold in conjunction with each coverage (e.g., --quorum 0.5,0.9)")
                    .short('l').long("coverage").default_value("1"),
                Arg::new("quorum").help("Unlike the --coverage parameter, which specifies a minimum constant number of paths for all growth point m (1 <= m <= num_paths), --quorum adjust the threshold based on m. At each m, a countable is counted in the average growth if the countable is contained in at least floor(m*quorum) paths. Example: A quorum of 0.9 requires a countable to be in 90% of paths for each subset size m. At m=10, it must appear in at least 9 paths. At m=100, it must appear in at least 90 paths. A quorum of 1 (100%) requires presence in all paths of the subset, corresponding to the core. Default: 0, a countable counts if it is present in any path at each growth point. Specify multiple quorum values with a comma-separated list (e.g., --quorum 0.5,0.9). Use --coverage to set static path thresholds in conjunction with variable quorum percentages (e.g., --coverage 5,10).")
                    .short('q').long("quorum").default_value("0"),
            ])
    }

    fn get_input_requirements(
        _matches: &clap::ArgMatches,
    ) -> Option<(HashSet<super::InputRequirement>, ViewParams, String)> {
        None
    }
}
