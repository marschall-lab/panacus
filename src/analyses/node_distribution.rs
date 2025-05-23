use std::collections::HashSet;

use itertools::Itertools;

use crate::{
    analysis_parameter::AnalysisParameter,
    graph_broker::GraphBroker,
    html_report::{AnalysisSection, Bin, ReportItem},
    util::CountType,
};

use super::{Analysis, ConstructibleAnalysis, InputRequirement};

pub struct NodeDistribution {
    parameter: AnalysisParameter,
    bins: Vec<Bin>,
    min: (u32, f64),
    max: (u32, f64),
}

impl Analysis for NodeDistribution {
    fn get_type(&self) -> String {
        "NodeDistribution".to_string()
    }

    fn generate_table(
        &mut self,
        gb: Option<&crate::graph_broker::GraphBroker>,
    ) -> anyhow::Result<String> {
        if self.bins.is_empty() {
            self.set_table(gb);
        }
        let mut result = "Bin\tCoverage\tLog-Length\tLog-Size\n".to_string();
        for (i, bin) in self.bins.iter().enumerate() {
            result.push_str(&format!(
                "{}\t{}\t{}\t{}\n",
                i, bin.real_x, bin.real_y, bin.length
            ));
        }
        Ok(result)
    }

    fn get_graph_requirements(&self) -> std::collections::HashSet<super::InputRequirement> {
        HashSet::from([InputRequirement::Node])
    }

    fn generate_report_section(
        &mut self,
        gb: Option<&crate::graph_broker::GraphBroker>,
    ) -> anyhow::Result<Vec<crate::html_report::AnalysisSection>> {
        let table = self.generate_table(gb)?;
        //let table = "".to_string();
        let table = format!("`{}`", &table);
        let id_prefix = format!(
            "node-dist-{}",
            self.get_run_name()
                .to_lowercase()
                .replace(&[' ', '|', '\\'], "-")
        );
        let radius = match self.parameter {
            AnalysisParameter::NodeDistribution { radius, .. } => {
                //(radius as f64 / 100.0 * 928.0).round() as u32
                radius
            }
            _ => panic!("NodeDistribution needs a node distribution parameter"),
        };
        let tab = vec![AnalysisSection {
            id: format!("{}-{}", id_prefix, CountType::Node.to_string()),
            analysis: "Node distribution".to_string(),
            table: Some(table),
            run_name: self.get_run_name(),
            countable: CountType::Node.to_string(),
            items: vec![ReportItem::Hexbin {
                id: format!("{id_prefix}-{}", CountType::Node),
                bins: self.bins.clone(),
                min: self.min,
                max: self.max,
                radius,
            }],
        }];
        Ok(tab)
    }
}

impl ConstructibleAnalysis for NodeDistribution {
    fn from_parameter(parameter: crate::analysis_parameter::AnalysisParameter) -> Self {
        Self {
            parameter,
            bins: Vec::new(),
            min: (0, 0.0),
            max: (0, 0.0),
        }
    }
}

impl NodeDistribution {
    fn set_table(&mut self, gb: Option<&GraphBroker>) {
        if let Some(gb) = gb {
            let countables = &gb.get_abacus_by_total(CountType::Node).countable[1..];
            let (cov_min, cov_max) = match countables.iter().minmax() {
                itertools::MinMaxResult::MinMax(min, max) => (min, max),
                _ => panic!("Node distribution needs to have at least two countables"),
            };
            let node_lens = &gb.get_node_lens()[1..]
                .iter()
                .map(|x| (*x as f64).log10())
                .collect::<Vec<f64>>();
            let (lens_min, lens_max) = match node_lens.iter().minmax() {
                itertools::MinMaxResult::MinMax(min, max) => (min, max),
                _ => panic!("Node distribution needs to have at least two countables"),
            };
            let points: Vec<(u32, f64)> = countables
                .iter()
                .copied()
                .zip(node_lens.iter().copied())
                .collect();
            let bins = Bin::hexbin(&points, 0.1);
            self.bins = bins;
            self.min = (*cov_min, *lens_min);
            self.max = (*cov_max, *lens_max);
        }
    }

    fn get_run_name(&self) -> String {
        "default-node-distribution-name".to_string()
    }
}
