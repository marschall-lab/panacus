use std::collections::HashSet;

use itertools::multizip;
use itertools::Itertools;

use crate::{
    graph_broker::{GraphBroker, ItemId},
    html_report::{AnalysisSection, Bin, ReportItem},
    util::get_default_plot_downloads,
    util::CountType,
};

use super::{Analysis, ConstructibleAnalysis, InputRequirement};

pub struct NodeDistribution {
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
            result.push_str(&format!("{}\t{}\t{}\t{}\n", i, bin.x, bin.y, bin.size));
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
            self.get_run_id(gb.expect("Node Distribution should be called with a graph"))
                .to_lowercase()
                .replace(&[' ', '|', '\\'], "-")
        );
        let tab = vec![AnalysisSection {
            id: format!("{}-{}", id_prefix, CountType::Node.to_string()),
            analysis: "Node distribution".to_string(),
            table: Some(table),
            run_name: self
                .get_run_name(gb.expect("Node Distribution should be called with a graph")),
            run_id: self.get_run_id(gb.expect("Node Distribution should be called with a graph")),
            countable: CountType::Node.to_string(),
            items: vec![ReportItem::Hexbin {
                id: format!("{id_prefix}-{}", CountType::Node),
                bins: self.bins.clone(),
            }],
            plot_downloads: get_default_plot_downloads(),
        }];
        Ok(tab)
    }
}

impl ConstructibleAnalysis for NodeDistribution {
    fn from_parameter(_parameter: crate::analysis_parameter::AnalysisParameter) -> Self {
        Self {
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
            let node_ids = gb.get_nodes().to_owned();
            let node_lens = &gb.get_node_lens()[1..]
                .iter()
                .map(|x| (*x as f64).log10())
                .collect::<Vec<f64>>();
            let (lens_min, lens_max) = match node_lens.iter().minmax() {
                itertools::MinMaxResult::MinMax(min, max) => (min, max),
                _ => panic!("Node distribution needs to have at least two countables"),
            };
            let points: Vec<(ItemId, u32, f64)> = multizip((
                node_ids,
                countables.into_iter().copied(),
                node_lens.into_iter().copied(),
            ))
            .collect();
            let bins = Bin::hexbin(&points, 15, 9);
            self.bins = bins;
            self.min = (*cov_min, *lens_min);
            self.max = (*cov_max, *lens_max);
        }
    }

    fn get_run_name(&self, gb: &GraphBroker) -> String {
        format!("{}", gb.get_run_name())
    }
    fn get_run_id(&self, gb: &GraphBroker) -> String {
        format!("{}-nodedistribution", gb.get_run_id())
    }
}
