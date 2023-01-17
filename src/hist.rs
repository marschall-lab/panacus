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
    pub coverage: Vec<usize>,
}

impl Hist {
    pub fn from_tsv<R: std::io::Read>(
        data: &mut std::io::BufReader<R>,
    ) -> Result<Self, std::io::Error> {
        let coverage = io::parse_hist(data)?;
        Ok(Self { coverage })
    }

    pub fn from_abacus(abacus: &Abacus) -> Self {
        Self {
            coverage: match abacus.count {
                CountType::Nodes | CountType::Edges => abacus.construct_hist(),
                CountType::Bps => abacus.construct_hist_bps(),
            },
        }
    }

    pub fn calc_growth(&self, t_coverage: &Threshold, t_intersection: &Threshold) -> Vec<usize> {
        let mut n = self.coverage.len() - 1; // hist array has length n+1: from 0..n (both included)
        let c = usize::max(1, t_coverage.to_absolute(n + 1));
        while self.coverage[n] == 0 {
            n -= 1;
        }

        //
        // Coverage threshold setting can be used to cut computation time, because the c-1 last
        // values do not change and thus can be copied from the previous value. We have
        // self.coverage.len() - 1 - (c - 1) =  self.coverage.len() - c
        //
        n = usize::min(n, self.coverage.len() - c);

        let mut pangrowth: Vec<usize> = vec![0; self.coverage.len() - 1];
        let mut n_fall_m = rug::Integer::from(1);
        let tot = rug::Integer::from(self.coverage.iter().sum::<usize>());

        // perc_mult[i] contains the percentage of combinations that
        // have an item of multiplicity i
        let mut perc_mult: Vec<rug::Integer> = Vec::with_capacity(n + 1);
        perc_mult.resize(n + 1, rug::Integer::from(1));

        for m in 1..n + 1 {
            let mut y = rug::Integer::from(0);
            for i in c..n - m + 1 {
                perc_mult[i] *= n - m - i + 1;
                y += self.coverage[i] * &perc_mult[i];
            }
            n_fall_m *= n - m + 1;

            let dividend: rug::Integer = rug::Integer::from(&n_fall_m * &tot - &y);
            let divisor: rug::Integer = rug::Integer::from(&n_fall_m);
            let (pang_m, _) = dividend.div_rem(rug::Integer::from(divisor));
            pangrowth[m - 1] = pang_m.to_usize().unwrap();
        }

        for x in n..self.coverage.len() - 1 {
            log::debug!("value {} copied from previous", x + 1);
            pangrowth[x] = pangrowth[x - 1];
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
    pub intersection: Vec<Threshold>,
    pub coverage: Vec<Threshold>,
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
        let mut intersection_thresholds = Vec::new();
        if !intersection.is_empty() {
            if std::path::Path::new(intersection).exists() {
                log::info!("loading intersection thresholds from {}", intersection);
                let mut data = std::io::BufReader::new(fs::File::open(intersection)?);
                intersection_thresholds = io::parse_threshold_file(&mut data)?;
            } else {
                intersection_thresholds = cli::parse_threshold_cli(&intersection[..])?;
            }
            log::debug!(
                "loaded {} intersection thresholds: {}",
                intersection_thresholds.len(),
                intersection_thresholds
                    .iter()
                    .map(|t| format!("{}", t))
                    .collect::<Vec<String>>()
                    .join(", ")
            );
        }
        if intersection_thresholds.is_empty() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "intersection threshold setting requires at least one element, but none is given",
            ));
        }

        let mut coverage_thresholds = Vec::new();
        if !coverage.is_empty() {
            if std::path::Path::new(&coverage).exists() {
                log::info!("loading coverage thresholds from {}", coverage);
                let mut data = std::io::BufReader::new(fs::File::open(coverage)?);
                coverage_thresholds = io::parse_threshold_file(&mut data)?;
            } else {
                coverage_thresholds = cli::parse_threshold_cli(&coverage[..])?;
            }
            log::debug!(
                "loaded {} coverage thresholds: {}",
                coverage_thresholds.len(),
                coverage_thresholds
                    .iter()
                    .map(|t| format!("{}", t))
                    .collect::<Vec<String>>()
                    .join(", ")
            );
        }
        if coverage_thresholds.is_empty() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "coverage threshold setting requires at least one element, but none is given",
            ));
        }

        if intersection_thresholds.len() != coverage_thresholds.len() {
            if intersection_thresholds.len() == 1 {
                intersection_thresholds =
                    vec![intersection_thresholds[0]; coverage_thresholds.len()];
            } else if coverage_thresholds.len() == 1 {
                coverage_thresholds = vec![coverage_thresholds[0]; intersection_thresholds.len()];
            } else {
                return Err(std::io::Error::new(std::io::ErrorKind::InvalidData,
                        "number of coverage and intersection threshold must match, or either one must have a single value"));
            }
        }

        Ok(Self {
            intersection: intersection_thresholds,
            coverage: coverage_thresholds,
        })
    }
}
