use std::io::Write;
use std::{
    collections::HashSet,
    io::{BufWriter, Error},
};

use crate::analysis_parameter::AnalysisParameter;
use crate::html_report::{AnalysisTab, ReportItem};
use crate::{analyses::InputRequirement, io::write_table, util::CountType};

use super::{Analysis, AnalysisSection};

pub struct Hist {
    parameter: AnalysisParameter,
}

impl Analysis for Hist {
    fn write_table<W: Write>(
        &mut self,
        gb: &crate::graph_broker::GraphBroker,
        out: &mut BufWriter<W>,
    ) -> Result<(), Error> {
        log::info!("reporting hist table");
        writeln!(
            out,
            "# {}",
            std::env::args().collect::<Vec<String>>().join(" ")
        )?;

        let mut header_cols = vec![vec![
            "panacus".to_string(),
            "count".to_string(),
            String::new(),
            String::new(),
        ]];
        let mut output_columns = Vec::new();
        for h in gb.get_hists().values() {
            output_columns.push(h.coverage.iter().map(|x| *x as f64).collect());
            header_cols.push(vec![
                "hist".to_string(),
                h.count.to_string(),
                String::new(),
                String::new(),
            ])
        }
        write_table(&header_cols, &output_columns, out)
    }

    fn generate_report_section(
        &mut self,
        gb: &crate::graph_broker::GraphBroker,
    ) -> Vec<AnalysisSection> {
        let mut buf = BufWriter::new(Vec::new());
        self.write_table(gb, &mut buf).expect("Can write to string");
        let bytes = buf.into_inner().unwrap();
        let table = String::from_utf8(bytes).unwrap();
        let table = format!("`{}`", &table);
        let histogram_tabs = gb
            .get_hists()
            .iter()
            .map(|(k, v)| AnalysisTab {
                id: format!("tab-cov-hist-{}", k),
                name: k.to_string(),
                is_first: false,
                items: vec![ReportItem::Bar {
                    id: format!("cov-hist-{}", k),
                    name: gb.get_fname(),
                    x_label: "taxa".to_string(),
                    y_label: format!("#{}s", k),
                    labels: (0..v.coverage.len()).map(|s| s.to_string()).collect(),
                    values: v.coverage.iter().map(|c| *c as f64).collect(),
                    log_toggle: true,
                }],
            })
            .collect::<Vec<_>>();
        vec![AnalysisSection {
            name: "coverage histogram".to_string(),
            id: "coverage-histogram".to_string(),
            is_first: true,
            table: Some(table),
            tabs: histogram_tabs,
        }
        .set_first()]
    }

    fn from_parameter(parameter: AnalysisParameter) -> Self {
        Self { parameter }
    }

    fn get_graph_requirements(&self) -> HashSet<super::InputRequirement> {
        if let AnalysisParameter::Hist {
            count_type,
            graph,
            subset,
            ..
        } = &self.parameter
        {
            let file_name = graph.to_string();
            let mut req = HashSet::from([InputRequirement::Hist]);
            req.extend(Self::count_to_input_req(*count_type));
            if let Some(subset) = subset {
                req.insert(InputRequirement::Subset(subset.to_owned()));
            }
            req.insert(InputRequirement::Graph(file_name));
            req
        } else {
            HashSet::new()
        }
    }
}

impl Hist {
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
