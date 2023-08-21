/* standard use */
use std::io::Write;

/* external crate */
use rayon::prelude::*;

/* private use */
use crate::abacus::AbacusByTotal;
use crate::cli;
use crate::util::{CountType, Threshold};

#[derive(Debug, Clone)]
pub struct Hist {
    pub count: CountType,
    pub coverage: Vec<usize>,
}

pub fn choose(n: usize, k: usize) -> f64 {
    let mut res: f64 = 0.0;
    if k > n {
        return 0.0;
    }

    let k = if k > n - k { n - k } else { k };

    let n = n as f64;

    for i in 0..k {
        res += (n - i as f64).log2();
        res -= (i as f64 + 1.0).log2();
    }
    res
}

impl Hist {
    pub fn from_abacus(abacus: &AbacusByTotal) -> Self {
        Self {
            count: abacus.count,
            coverage: match abacus.count {
                CountType::Node | CountType::Edge => abacus.construct_hist(),
                CountType::Bp => abacus.construct_hist_bps(),
                CountType::All => unreachable!("inadmissable count type"),
            },
        }
    }

    pub fn calc_growth(&self, t_coverage: &Threshold, t_quorum: &Threshold) -> Vec<f64> {
        let n = self.coverage.len() - 1;
        let quorum = usize::max(1, t_quorum.to_absolute(n));
        if quorum == 1 {
            self.calc_growth_union(t_coverage)
        } else if quorum >= n {
            self.calc_growth_core(t_coverage)
        } else {
            self.calc_growth_quorum(t_coverage, t_quorum)
        }
    }

    pub fn calc_all_growths(&self, hist_aux: &HistAuxilliary) -> Vec<Vec<f64>> {
        let mut growths: Vec<Vec<f64>> = hist_aux
            .coverage
            .par_iter()
            .zip(&hist_aux.quorum)
            .map(|(c, q)| {
                log::info!(
                    "calculating growth for coverage >= {} and quorum >= {}",
                    &c,
                    &q
                );
                self.calc_growth(&c, &q)
            })
            .collect();
        // insert empty row for 0 element
        for g in &mut growths {
            g.insert(0, std::f64::NAN);
        }
        growths
    }

    pub fn calc_growth_union(&self, t_coverage: &Threshold) -> Vec<f64> {
        let n = self.coverage.len() - 1; // hist array has length n+1: from 0..n (both included)
        let c = usize::max(1, t_coverage.to_absolute(n));

        let mut pangrowth: Vec<f64> = vec![0.0; n];
        let mut n_fall_m: f64 = 0.0;
        let tot = self.coverage[c..].iter().sum::<usize>() as f64;

        // perc_mult[i] contains the percentage of combinations that
        // have an item of multiplicity i
        let mut perc_mult: Vec<f64> = Vec::with_capacity(n + 1);
        perc_mult.resize(n + 1, 0.0);

        for m in 1..n + 1 {
            let mut y: f64 = 0.0;
            n_fall_m += (n as f64 - m as f64 + 1.0).log2();
            for i in c..n - m + 1 {
                perc_mult[i] += (n as f64 - m as f64 - i as f64 + 1.0).log2();
                y += ((self.coverage[i] as f64).log2() + perc_mult[i] - n_fall_m).exp2();
            }

            pangrowth[m - 1] = tot - y;
        }

        pangrowth
    }

    pub fn calc_growth_core(&self, t_coverage: &Threshold) -> Vec<f64> {
        let n = self.coverage.len() - 1; // hist array has length n+1: from 0..n (both included)
        let c = usize::max(1, t_coverage.to_absolute(n + 1));
        let mut n_fall_m: f64 = 0.0;
        let mut pangrowth: Vec<f64> = vec![0.0; n];

        // In perc_mult[i] is contained the percentage of combinations
        // that have an item of multiplicity i
        let mut perc_mult: Vec<f64> = Vec::with_capacity(n + 1);
        perc_mult.resize(n + 1, 0.0);

        for m in 1..n + 1 {
            let mut y: f64 = 0.0;
            n_fall_m += (n as f64 - m as f64 + 1.0).log2();
            for i in usize::max(m, c)..n + 1 {
                perc_mult[i] += (i as f64 - m as f64 + 1.0).log2();
                y += ((self.coverage[i] as f64).log2() + perc_mult[i] - n_fall_m).exp2();
            }
            pangrowth[m - 1] = y;
        }

        pangrowth
    }

    pub fn calc_growth_quorum(&self, t_coverage: &Threshold, t_quorum: &Threshold) -> Vec<f64> {
        let n = self.coverage.len() - 1; // hist array has length n+1: from [0..n]
        let c = usize::max(1, t_coverage.to_absolute(n));
        let quorum = t_quorum.to_relative(n);
        let mut pangrowth: Vec<f64> = vec![0.0; n];

        let mut n_fall_m: f64 = 0.0;
        let mut m_fact: f64 = 0.0;

        let mut perc_mult: Vec<f64> = vec![0.0; n + 1];
        let mut q: Vec<Vec<f64>> = vec![vec![0.0; n + 1]; n + 1];

        for m in 1..n + 1 {
            m_fact += (m as f64).log2();
            let m_quorum = (m as f64 * quorum).ceil() as usize;

            //100% quorum
            let mut yl: f64 = 0.0;
            n_fall_m += (n as f64 - m as f64 + 1.0).log2();
            for i in usize::max(m, c)..n + 1 {
                perc_mult[i] += (i as f64 - m as f64 + 1.0).log2();
                yl += ((self.coverage[i] as f64).log2() + perc_mult[i] - n_fall_m).exp2();
            }

            //[m_quorum, 100) quorum
            let mut yr: f64 = 0.0;
            for i in m_quorum..n {
                let mut sum_q = 0.0;
                let mut add = false;
                for j in usize::max(m_quorum, c)..m {
                    if n + j + 1 > i + m && j <= i {
                        if q[i][j] == 0.0 {
                            q[i][j] = choose(i, j);
                        }
                        q[i][j] += (n as f64 - i as f64 - m as f64 + 1.0 + j as f64).log2();
                        q[i][j] -= (m as f64 - j as f64).log2();
                        sum_q += q[i][j].exp2();
                        add = true;
                    }
                }
                if add {
                    yr += ((self.coverage[i] as f64).log2() + sum_q.log2() + m_fact - n_fall_m)
                        .exp2();
                }
            }
            pangrowth[m - 1] = yl + yr;
        }
        pangrowth
    }

    #[allow(dead_code)]
    pub fn to_tsv<W: std::io::Write>(
        &self,
        out: &mut std::io::BufWriter<W>,
    ) -> Result<(), std::io::Error> {
        writeln!(out, "hist\t{}", self.count)?;
        for (i, c) in self.coverage.iter().enumerate() {
            writeln!(out, "{}\t{}", i, c)?;
        }

        Ok(())
    }
}

pub struct HistAuxilliary {
    pub quorum: Vec<Threshold>,
    pub coverage: Vec<Threshold>,
}

impl HistAuxilliary {
    pub fn from_params(params: &cli::Params) -> Result<Self, std::io::Error> {
        match params {
            cli::Params::Histgrowth {
                quorum, coverage, ..
            }
            | cli::Params::Growth {
                quorum, coverage, ..
            }
            | cli::Params::OrderedHistgrowth {
                quorum, coverage, ..
            } => Self::load(quorum, coverage),
            _ => Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "not implemented",
            )),
        }
    }

    fn load(quorum: &str, coverage: &str) -> Result<Self, std::io::Error> {
        let mut quorum_thresholds = Vec::new();
        if !quorum.is_empty() {
            quorum_thresholds =
                cli::parse_threshold_cli(&quorum[..], cli::RequireThreshold::Relative)?;
            log::debug!(
                "loaded {} quorum thresholds: {}",
                quorum_thresholds.len(),
                quorum_thresholds
                    .iter()
                    .map(|t| format!("{}", t))
                    .collect::<Vec<String>>()
                    .join(", ")
            );
        }
        if quorum_thresholds.is_empty() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "quorum threshold setting requires at least one element, but none is given",
            ));
        }

        let mut coverage_thresholds = Vec::new();
        if !coverage.is_empty() {
            coverage_thresholds =
                cli::parse_threshold_cli(&coverage[..], cli::RequireThreshold::Absolute)?;
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

        if quorum_thresholds.len() != coverage_thresholds.len() {
            if quorum_thresholds.len() == 1 {
                quorum_thresholds = vec![quorum_thresholds[0]; coverage_thresholds.len()];
            } else if coverage_thresholds.len() == 1 {
                coverage_thresholds = vec![coverage_thresholds[0]; quorum_thresholds.len()];
            } else {
                return Err(std::io::Error::new(std::io::ErrorKind::InvalidData,
                        "number of coverage and quorum threshold must match, or either one must have a single value"));
            }
        }

        Ok(Self {
            quorum: quorum_thresholds,
            coverage: coverage_thresholds,
        })
    }
}
