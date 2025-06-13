use std::collections::HashSet;

use crate::{
    analysis_parameter::AnalysisParameter,
    graph_broker::GraphBroker,
    html_report::{AnalysisSection, ReportItem},
    io::write_table_with_start_index,
    util::get_default_plot_downloads,
    util::CountType,
};

use super::{Analysis, ConstructibleAnalysis, InputRequirement};

pub struct CoverageLine {
    parameter: AnalysisParameter,
}

impl Analysis for CoverageLine {
    fn get_type(&self) -> String {
        "CoverageLine".to_string()
    }

    fn generate_table(
        &mut self,
        gb: Option<&crate::graph_broker::GraphBroker>,
    ) -> anyhow::Result<String> {
        log::info!("reporting coverage line table");
        if gb.is_none() {
            panic!("CoverageLine analysis needs a graph")
        }
        let gb = gb.unwrap();
        let mut res = String::new();
        res.push_str(&crate::io::write_metadata_comments()?);

        let mut header_cols = vec![vec![
            "panacus".to_string(),
            "count".to_string(),
            String::new(),
            String::new(),
        ]];
        let mut output_columns = Vec::new();
        for h in gb.get_hists().values() {
            output_columns.push(h.coverage.iter().map(|x| *x as f64).skip(1).collect());
            header_cols.push(vec![
                "hist".to_string(),
                h.count.to_string(),
                String::new(),
                String::new(),
            ])
        }
        res.push_str(&write_table_with_start_index(
            &header_cols,
            &output_columns,
            1,
        )?);
        Ok(res)
    }

    fn generate_report_section(
        &mut self,
        gb: Option<&crate::graph_broker::GraphBroker>,
    ) -> anyhow::Result<Vec<crate::html_report::AnalysisSection>> {
        if gb.is_none() {
            panic!("CoverageLine analysis needs a graph")
        }
        let gb = gb.unwrap();
        let table = self.generate_table(Some(gb))?;
        let table = format!("`{}`", &table);
        let id_prefix = format!(
            "coverage-line-{}",
            self.get_run_id(gb)
                .to_lowercase()
                .replace(&[' ', '|', '\\'], "-")
        );
        let coverage_line_tabs = gb
            .get_hists()
            .iter()
            .map(|(k, v)| {
                let mut values: Vec<_> = v.coverage.clone();
                while let Some(last) = values.pop() {
                    if last != 0 {
                        values.push(0);
                        break;
                    }
                }
                let values: Vec<f32> = values.into_iter().skip(1).map(|c| c as f32).collect();
                AnalysisSection {
                    id: format!("{id_prefix}-{k}"),
                    analysis: "Coverage Line".to_string(),
                    table: Some(table.clone()),
                    run_name: self.get_run_name(gb),
                    run_id: self.get_run_id(gb),
                    countable: k.to_string(),
                    items: vec![ReportItem::Line {
                        id: format!("{id_prefix}-{k}"),
                        name: gb.get_fname(),
                        x_label: "Allele count".to_string(),
                        y_label: format!("#{}s", k),
                        x_values: (1..=values.len()).map(|s| s as f32).collect(),
                        y_values: values,
                        log_x: true,
                        log_y: true,
                    }],
                    plot_downloads: get_default_plot_downloads(),
                }
            })
            .collect::<Vec<_>>();
        Ok(coverage_line_tabs)
    }

    fn get_graph_requirements(&self) -> std::collections::HashSet<super::InputRequirement> {
        if let AnalysisParameter::CoverageLine { count_type, .. } = &self.parameter {
            let mut req = HashSet::from([InputRequirement::Hist]);
            req.extend(Self::count_to_input_req(*count_type));
            req
        } else {
            HashSet::new()
        }
    }
}

impl ConstructibleAnalysis for CoverageLine {
    fn from_parameter(parameter: AnalysisParameter) -> Self {
        Self { parameter }
    }
}

impl CoverageLine {
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
        format!("{}", gb.get_run_name())
    }

    fn get_run_id(&self, gb: &GraphBroker) -> String {
        format!("{}-coverageline", gb.get_run_id())
    }
}
