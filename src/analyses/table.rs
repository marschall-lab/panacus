use crate::{
    analyses::InputRequirement, analysis_parameter::AnalysisParameter, io::write_metadata_comments,
    util::CountType,
};
use core::panic;
use std::{collections::HashSet, io::BufWriter};

use super::{Analysis, AnalysisSection, ConstructibleAnalysis};

pub struct Table {
    parameter: AnalysisParameter,
}

impl Analysis for Table {
    fn generate_table(
        &mut self,
        gb: Option<&crate::graph_broker::GraphBroker>,
    ) -> anyhow::Result<String> {
        if let Some(gb) = gb {
            let total = match self.parameter {
                AnalysisParameter::Table { total, .. } => total,
                _ => {
                    panic!("Table analysis needs a table parameter")
                }
            };
            let mut buf = BufWriter::new(Vec::new());
            gb.write_abacus_by_group(total, &mut buf)?;
            let bytes = buf.into_inner()?;
            let mut string = write_metadata_comments()?;
            string.push_str(&String::from_utf8(bytes)?);
            Ok(string)
        } else {
            panic!("Table table generation should get Graph");
        }
    }

    fn get_type(&self) -> String {
        "Table".to_string()
    }

    fn get_graph_requirements(&self) -> HashSet<InputRequirement> {
        if let AnalysisParameter::Table { count_type, .. } = &self.parameter {
            let mut req = HashSet::from([InputRequirement::AbacusByGroup(*count_type)]);
            req.extend(Self::count_to_input_req(*count_type));
            req
        } else {
            HashSet::new()
        }
    }

    fn generate_report_section(
        &mut self,
        _dm: Option<&crate::graph_broker::GraphBroker>,
    ) -> anyhow::Result<Vec<AnalysisSection>> {
        Ok(Vec::new())
    }
}

impl ConstructibleAnalysis for Table {
    fn from_parameter(parameter: crate::analysis_parameter::AnalysisParameter) -> Self {
        Table { parameter }
    }
}

impl Table {
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
}
