/* standard use */
use std::fs;
use std::io::Write;

/* private use */
use crate::abacus::AbacusByTotal;
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

    pub fn from_abacus(abacus: &AbacusByTotal) -> Self {
        Self {
            coverage: match abacus.count {
                CountType::Nodes | CountType::Edges => abacus.construct_hist(),
                CountType::Bps => abacus.construct_hist_bps(),
            },
        }
    }

   pub fn calc_growth(&self, t_coverage: &Threshold, t_quorum: &Threshold) -> Vec<usize>{
        let n = self.coverage.len() - 1;
        let quorum = usize::max(1, t_quorum.to_absolute(n + 1));
        if quorum == 1 {
            self.calc_growth_union(t_coverage)
        } else if quorum == n {
            self.calc_growth_core(t_coverage)
        } else {
            self.calc_growth_quorum(t_coverage, t_quorum)
        }
    }

    pub fn calc_growth_union(&self, t_coverage: &Threshold) -> Vec<usize> {
        let mut n = self.coverage.len() - 1; // hist array has length n+1: from 0..n (both included)
        let c = usize::max(1, t_coverage.to_absolute(n + 1));
        //while self.coverage[n] == 0 {
        //    n -= 1;
        //}
        
        // coverage threshold setting can be used to cut computation time, because the c-1 last
        // values do not change and thus can be copied from the previous value. We have
        // self.coverage.len() - 1 - (c - 1) =  self.coverage.len() - c
        n = usize::min(n, self.coverage.len() - c);

        let mut pangrowth: Vec<usize> = vec![0; n + 1];
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
            pangrowth[m] = pang_m.to_usize().unwrap();
        }

        for x in n..self.coverage.len() - 1 {
            log::debug!("value {} copied from previous", x + 1);
            pangrowth[x] = pangrowth[x - 1];
        }

        pangrowth
    }

    pub fn calc_growth_core(&self, t_coverage: &Threshold) -> Vec<usize> {
        let mut n = self.coverage.len() - 1; // hist array has length n+1: from 0..n (both included)
        let c = usize::max(1, t_coverage.to_absolute(n + 1));
        let mut n_fall_m = rug::Integer::from(1);
        let mut pangrowth: Vec<usize> = vec![0; n + 1];

        // In perc_mult[i] is contained the percentage of combinations 
        // that have an item of multiplicity i
        let mut perc_mult: Vec<rug::Integer> = Vec::with_capacity(n + 1);
        perc_mult.resize(n + 1, rug::Integer::from(1));

        for m in 1..n + 1 {
            let mut y = rug::Integer::from(0);
            for i in usize::max(m,c)..n + 1 {
                perc_mult[i] *= i-m+1;
                y += self.coverage[i] * &perc_mult[i];
            }
            n_fall_m *= n - m + 1;

            let (pang_m, _) = y.div_rem(rug::Integer::from(&n_fall_m));
            pangrowth[m] = pang_m.to_usize().unwrap();
            //println!("{} {}", m, pang_m);
        }

        pangrowth
    }

   pub fn calc_growth_quorum(&self, t_coverage: &Threshold, t_quorum: &Threshold) -> Vec<usize>{
        let mut n = self.coverage.len() - 1; // hist array has length n+1: from 0..n (both included)
        let c = usize::max(1, t_coverage.to_absolute(n + 1));
        let absolute_quorum = usize::max(1, t_quorum.to_absolute(n + 1));
        let relative_quorum = (absolute_quorum as f64)/(n as f64);
        let mut pangrowth: Vec<usize> = vec![0; n + 1];

        let mut n_fall_m = rug::Integer::from(1);
        let mut m_fact = rug::Integer::from(1);

        let mut perc_mult = vec![rug::Integer::from(1); n+1];
        let mut q = vec![vec![rug::Integer::from(0); n+1]; n+1];

        for m in 1..n + 1 {
            m_fact *= m;
            let m_quorum = (m as f64 * relative_quorum).ceil() as usize;

            //100% quorum
            let mut yl = rug::Integer::from(0);
            for i in usize::max(m,c)..n + 1 {
                perc_mult[i] *= i-m+1;
                yl += self.coverage[i] * &perc_mult[i];
            }
            n_fall_m *= n - m + 1;

            //[m_quorum, 100) quorum
            let mut yr = rug::Integer::from(0);
            for i in m_quorum..n+1 {
                let mut sum_q = rug::Integer::from(0);
                for j in usize::max(m_quorum, c)..m {
                    if n+j+1>i+m {
                        if q[i][j] == 0 {
                            let ii = rug::Integer::from(i);
                            q[i][j] = ii.binomial(j as u32);
                        }
                        q[i][j] *= n-i-m+1+j;
                        q[i][j] /= m-j;
                        sum_q += &q[i][j];
                    }
                }
                yr += self.coverage[i] * sum_q;
            }

            let y = yl + yr * &m_fact;
            let (pang_m, _) = y.div_rem(rug::Integer::from(&n_fall_m));
            pangrowth[m] = pang_m.to_usize().unwrap();
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
    pub quorum: Vec<Threshold>,
    pub coverage: Vec<Threshold>,
}

impl HistAuxilliary {
    pub fn from_params(params: &cli::Params) -> Result<Self, std::io::Error> {
        match params {
            cli::Params::Histgrowth {
                quorum,
                coverage,
                ..
            }
            | cli::Params::Growth {
                quorum,
                coverage,
                ..
            }
            | cli::Params::OrderedHistgrowth {
                quorum,
                coverage,
                ..
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
            if std::path::Path::new(quorum).exists() {
                log::info!("loading quorum thresholds from {}", quorum);
                let mut data = std::io::BufReader::new(fs::File::open(quorum)?);
                quorum_thresholds = io::parse_threshold_file(&mut data)?;
            } else {
                quorum_thresholds = cli::parse_threshold_cli(&quorum[..])?;
            }
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

        if quorum_thresholds.len() != coverage_thresholds.len() {
            if quorum_thresholds.len() == 1 {
                quorum_thresholds =
                    vec![quorum_thresholds[0]; coverage_thresholds.len()];
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
