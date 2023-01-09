/* standard use */
use std::fs;
use std::io::Write;

/* private use */
use crate::abacus::Abacus;
use crate::cli;
use crate::io;
use crate::util::{CountType, Threshold};

#[derive(Debug, Clone)]
pub struct Hist {
    pub coverage: Vec<u32>,
}

impl Hist {
    pub fn from_tsv<R: std::io::Read>(data: &mut std::io::BufReader<R>) -> Self {
        // XXX TODO
        Self {
            coverage: Vec::new(),
        }
    }

    pub fn from_abacus(abacus: &Abacus<u32>) -> Self {
        Self {
            coverage: abacus.construct_hist(),
        }
    }

    pub fn calc_growth(&self) -> Vec<u32> {
        let n = self.coverage.len() - 1; // hist array has length n+1: from 0..n (both included)
        let mut pangrowth: Vec<u32> = Vec::with_capacity(n + 1);
        let mut n_fall_m = rug::Integer::from(1);
        let tot = rug::Integer::from(self.coverage.iter().sum::<u32>());

        // perc_mult[i] contains the percentage of combinations that
        // have an item of multiplicity i
        let mut perc_mult: Vec<rug::Integer> = Vec::with_capacity(n + 1);
        perc_mult.resize(n + 1, rug::Integer::from(1));

        for m in 1..n + 1 {
            let mut y = rug::Integer::from(0);
            for i in 1..n - m + 1 {
                perc_mult[i] *= n - m - i + 1;
                y += self.coverage[i] * &perc_mult[i];
            }
            n_fall_m *= n - m + 1;

            let dividend: rug::Integer = rug::Integer::from(&n_fall_m * &tot - &y);
            let divisor: rug::Integer = rug::Integer::from(&n_fall_m);
            let (pang_m, _) = dividend.div_rem(rug::Integer::from(divisor));
            pangrowth.push(pang_m.to_u32().unwrap());
        }
        pangrowth
    }

    pub fn to_tsv<W: std::io::Write>(
        &self,
        count: &CountType,
        out: &mut std::io::BufWriter<W>,
    ) -> Result<(), std::io::Error> {
        writeln!(out, "coverage\t{}", count)?;
        for (i, c) in self.coverage.iter().enumerate() {
            writeln!(out, "{}\t{}", i, c)?;
        }

        Ok(())
    }
}

pub struct HistAuxilliary {
    pub intersection: Option<Vec<(String, Threshold)>>,
    pub coverage: Option<Vec<(String, Threshold)>>,
}

impl HistAuxilliary {
    pub fn from_params(params: &cli::Params) -> Result<Self, std::io::Error> {
        match params {
            cli::Params::Histgrowth {
                intersection,
                coverage,
                ..
            }
            | cli::Params::Growth {
                intersection,
                coverage,
                ..
            } => Self::load(intersection, coverage),
            _ => Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "not implemented",
            )),
        }
    }

    fn load(intersection: &str, coverage: &str) -> Result<Self, std::io::Error> {
        let mut intersection_thresholds = None;
        if !intersection.is_empty() {
            if std::path::Path::new(intersection).exists() {
                log::info!("loading intersection thresholds from {}", intersection);
                let mut data = std::io::BufReader::new(fs::File::open(intersection)?);
                intersection_thresholds = Some(io::parse_coverage_threshold_file(&mut data));
            } else {
                intersection_thresholds =
                    Some(cli::parse_coverage_threshold_cli(&intersection[..]));
            }
            log::debug!(
                "loaded {} intersection thresholds:\n{}",
                intersection_thresholds.as_ref().unwrap().len(),
                intersection_thresholds
                    .as_ref()
                    .unwrap()
                    .iter()
                    .map(|(n, t)| format!("\t{}: {}", n, t))
                    .collect::<Vec<String>>()
                    .join("\n")
            );
        }

        let mut coverage_thresholds = None;
        if !coverage.is_empty() {
            if std::path::Path::new(&coverage).exists() {
                log::info!("loading coverage thresholds from {}", coverage);
                let mut data = std::io::BufReader::new(fs::File::open(coverage)?);
                coverage_thresholds = Some(io::parse_coverage_threshold_file(&mut data));
            } else {
                coverage_thresholds = Some(cli::parse_coverage_threshold_cli(&coverage[..]));
            }
            log::debug!(
                "loaded {} coverage thresholds:\n{}",
                coverage_thresholds.as_ref().unwrap().len(),
                coverage_thresholds
                    .as_ref()
                    .unwrap()
                    .iter()
                    .map(|(n, t)| format!("\t{}: {}", n, t))
                    .collect::<Vec<String>>()
                    .join("\n")
            );
        }

        Ok(Self {
            intersection: intersection_thresholds,
            coverage: coverage_thresholds,
        })
    }
}
