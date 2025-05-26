use std::collections::HashSet;

use rayon::iter::{IndexedParallelIterator, IntoParallelRefIterator, ParallelIterator};

use crate::analysis_parameter::AnalysisParameter;
use crate::graph_broker::{GraphBroker, ThresholdContainer};
use crate::html_report::ReportItem;
use crate::util::CountType;
use crate::{analyses::InputRequirement, io::write_ordered_histgrowth_table};

use super::{Analysis, AnalysisSection, ConstructibleAnalysis};

type Growths = Vec<Vec<f64>>;

pub struct OrderedHistgrowth {
    parameter: AnalysisParameter,
    inner: Option<InnerOrderedGrowth>,
}

const MAX_WIDTH: usize = 25;

impl ConstructibleAnalysis for OrderedHistgrowth {
    fn from_parameter(parameter: AnalysisParameter) -> Self {
        Self {
            parameter,
            inner: None,
        }
    }
}

impl Analysis for OrderedHistgrowth {
    fn get_type(&self) -> String {
        "OrderedHistgrowth".to_string()
    }
    fn generate_table(
        &mut self,
        gb: Option<&crate::graph_broker::GraphBroker>,
    ) -> anyhow::Result<String> {
        if let Some(gb) = gb {
            write_ordered_histgrowth_table(
                gb.get_abacus_by_group(),
                &self.inner.as_ref().unwrap().hist_aux,
                gb.get_node_lens(),
            )
        } else {
            Ok("".to_string())
        }
    }

    fn generate_report_section(
        &mut self,
        dm: Option<&GraphBroker>,
    ) -> anyhow::Result<Vec<AnalysisSection>> {
        self.set_inner(dm)?;
        let count = match self.parameter {
            AnalysisParameter::OrderedGrowth { count_type, .. } => count_type,
            _ => {
                panic!("Parameter has to fit the analysis")
            }
        };
        let hist_aux = &self.inner.as_ref().unwrap().hist_aux;
        let growth_labels = (0..hist_aux.coverage.len())
            .map(|i| {
                format!(
                    "coverage ≥ {}, quorum ≥ {}%",
                    hist_aux.coverage[i].get_string(),
                    hist_aux.quorum[i].get_string()
                )
            })
            .collect::<Vec<_>>();
        let table = self.generate_table(dm)?;
        let table = format!("`{}`", &table);
        let growths = &self.inner.as_ref().unwrap().growths;
        let id_prefix = format!(
            "pan-ordered-growth-{}",
            self.get_run_name(dm.expect("Ordered Growth should be called with a graph"))
                .to_lowercase()
                .replace(&[' ', '|', '\\'], "-")
        );
        let labels = dm.unwrap().get_abacus_by_group().groups.clone();
        let growth_tabs = vec![AnalysisSection {
            id: format!("{id_prefix}"),
            analysis: "Ordered Growth".to_string(),
            run_name: self.get_run_name(dm.expect("Ordered Growth should be called with a graph")),
            countable: count.to_string(),
            table: Some(table.clone()),
            items: vec![ReportItem::MultiBar {
                id: format!("{id_prefix}"),
                names: growth_labels.clone(),
                x_label: "taxa".to_string(),
                y_label: format!("{}s", count),
                //labels: (1..growths[0].len()).map(|i| i.to_string()).collect(),
                labels,
                values: growths.clone(),
                log_toggle: false,
            }],
        }];
        Ok(growth_tabs)
        //let mut growths: Vec<Vec<f64>> = self
        //    .hist_aux
        //    .coverage
        //    .par_iter()
        //    .zip(&self.hist_aux.quorum)
        //    .map(|(c, q)| {
        //        log::info!(
        //            "calculating ordered growth for coverage >= {} and quorum >= {}",
        //            &c,
        //            &q
        //        );
        //        gb.get_abacus_by_group()
        //            .calc_growth(c, q, gb.get_node_lens())
        //    })
        //    .collect();
        //// insert empty row for 0 element
        //for c in &mut growths {
        //    c.insert(0, f64::NAN);
        //}
        //let table = self.generate_table(Some(gb)).expect("Can write to string");
        //let k = gb.get_abacus_by_group().count;
        //Ok(vec![
        //])
    }

    fn get_graph_requirements(&self) -> HashSet<InputRequirement> {
        if let AnalysisParameter::OrderedGrowth { count_type, .. } = &self.parameter {
            let mut req = HashSet::from([InputRequirement::AbacusByGroup(*count_type)]);
            req.extend(Self::count_to_input_req(*count_type));
            req
        } else {
            HashSet::new()
        }
    }
}

impl OrderedHistgrowth {
    fn truncate(input: &str) -> String {
        let res: String = input.chars().rev().take(MAX_WIDTH).collect();
        let res: String = res.chars().rev().collect();
        if res.len() < input.len() {
            format!("...{}", res)
        } else {
            res
        }
    }

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

    fn get_run_name(&self, gb: &GraphBroker) -> String {
        format!("{}-orderedgrowth", gb.get_run_name())
    }

    fn set_inner(&mut self, gb: Option<&GraphBroker>) -> anyhow::Result<()> {
        if self.inner.is_some() {
            return Ok(());
        }

        if let AnalysisParameter::OrderedGrowth {
            coverage, quorum, ..
        } = &self.parameter
        {
            let quorum = quorum.to_owned().unwrap_or("0".to_string());
            let coverage = coverage.to_owned().unwrap_or("1".to_string());
            let hist_aux = ThresholdContainer::parse_params(&quorum, &coverage)?;

            if gb.is_none() {
                panic!("OrderedHistgrowth needs a graph in order to work");
            }

            let growths: Vec<Vec<f64>> = hist_aux
                .coverage
                .par_iter()
                .zip(&hist_aux.quorum)
                .map(|(c, q)| {
                    log::info!(
                        "calculating ordered growth for coverage >= {} and quorum >= {}",
                        &c,
                        &q
                    );
                    gb.unwrap()
                        .get_abacus_by_group()
                        .calc_growth(c, q, gb.unwrap().get_node_lens())
                })
                .collect();
            self.inner = Some(InnerOrderedGrowth {
                growths,
                hist_aux,
                graph: gb.unwrap().get_fname(),
            });
            Ok(())
        } else {
            panic!("OrderedGrowth should always contain ordered-growth parameter")
        }
    }
}

struct InnerOrderedGrowth {
    growths: Growths,
    hist_aux: ThresholdContainer,
    graph: String,
}
