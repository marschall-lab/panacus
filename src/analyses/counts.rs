use std::collections::HashSet;

use super::{Analysis, ConstructibleAnalysis};

struct Count {}

impl Analysis for Count {
    fn get_type(&self) -> String {
        "Count".to_string()
    }

    fn generate_table(
        &mut self,
        gb: Option<&crate::graph_broker::GraphBroker>,
    ) -> anyhow::Result<String> {
        Ok(String::new())
    }

    fn get_graph_requirements(&self) -> std::collections::HashSet<super::InputRequirement> {
        HashSet::new()
    }

    fn generate_report_section(
        &mut self,
        gb: Option<&crate::graph_broker::GraphBroker>,
    ) -> anyhow::Result<Vec<crate::html_report::AnalysisSection>> {
        Ok(Vec::new())
    }
}

impl ConstructibleAnalysis for Count {
    fn from_parameter(parameter: crate::analysis_parameter::AnalysisParameter) -> Self {
        Self {}
    }
}
