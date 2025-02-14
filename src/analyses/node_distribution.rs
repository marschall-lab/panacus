use core::panic;
use std::collections::HashSet;

use crate::{
    analysis_parameter::AnalysisParameter,
    graph_broker::GraphBroker,
    html_report::{AnalysisSection, ReportItem},
    util::CountType,
};

use super::{Analysis, ConstructibleAnalysis, InputRequirement};

pub struct NodeDistribution {
    parameter: AnalysisParameter,
    inner: Vec<(u32, u32)>,
}

impl Analysis for NodeDistribution {
    fn get_type(&self) -> String {
        "NodeDistribution".to_string()
    }

    fn generate_table(
        &mut self,
        gb: Option<&crate::graph_broker::GraphBroker>,
    ) -> anyhow::Result<String> {
        if self.inner.is_empty() {
            self.set_table(gb);
        }
        let mut result = "Coverage\tLength\n".to_string();
        let body = self
            .inner
            .iter()
            .map(|(a, b)| format!("{}\t{}\n", a, b))
            .collect::<Vec<String>>()
            .join("");
        result.push_str(&body);
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
        let table = format!("`{}`", &table);
        let id_prefix = format!(
            "node-dist-{}",
            self.get_run_name()
                .to_lowercase()
                .replace(&[' ', '|', '\\'], "-")
        );
        let tab = vec![AnalysisSection {
            id: format!("{}-{}", id_prefix, CountType::Node.to_string()),
            analysis: "Node distribution".to_string(),
            table: Some(table),
            run_name: self.get_run_name(),
            countable: CountType::Node.to_string(),
            items: vec![ReportItem::Hexbin {
                id: format!("{id_prefix}-{}", CountType::Node),
                values: self.inner.clone(),
            }],
        }];
        Ok(tab)
    }
}

impl ConstructibleAnalysis for NodeDistribution {
    fn from_parameter(parameter: crate::analysis_parameter::AnalysisParameter) -> Self {
        Self {
            parameter,
            inner: Vec::new(),
        }
    }
}

impl NodeDistribution {
    fn set_table(&mut self, gb: Option<&GraphBroker>) {
        if let Some(gb) = gb {
            let countables = &gb.get_abacus_by_total(CountType::Node).countable[1..];
            let node_lens = &gb.get_node_lens()[1..];
            self.inner = countables
                .iter()
                .copied()
                .zip(node_lens.iter().copied())
                .collect();
        }
    }

    fn get_run_name(&self) -> String {
        match &self.parameter {
            AnalysisParameter::NodeDistribution { graph } => {
                format!("{}", graph)
            }
            _ => panic!("Counts analysis needs to contain counts parameter"),
        }
    }
}
