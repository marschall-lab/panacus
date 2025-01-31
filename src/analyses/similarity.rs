use itertools::Itertools;

use crate::{
    analyses::InputRequirement, analysis_parameter::AnalysisParameter, io::write_metadata_comments,
    util::CountType,
};
use std::collections::HashSet;

use super::{Analysis, AnalysisSection, ConstructibleAnalysis};

pub struct Similarity {
    parameter: AnalysisParameter,
    table: Option<Vec<Vec<f32>>>,
}

impl Analysis for Similarity {
    fn generate_table(
        &mut self,
        gb: Option<&crate::graph_broker::GraphBroker>,
    ) -> anyhow::Result<String> {
        if self.table.is_none() {
            self.set_table(gb);
        }
        if let Some(gb) = gb {
            let mut text = write_metadata_comments()?;
            let table = self.table.as_ref().unwrap();
            text.push_str(&get_table_string(table, &gb.get_abacus_by_group().groups));
            Ok(text)
        } else {
            Ok("".to_string())
        }
    }

    fn get_type(&self) -> String {
        "Similarity".to_string()
    }

    fn get_graph_requirements(&self) -> HashSet<InputRequirement> {
        if let AnalysisParameter::Similarity { count_type, .. } = &self.parameter {
            let mut req = HashSet::from([InputRequirement::AbacusByGroupCsc]);
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

impl ConstructibleAnalysis for Similarity {
    fn from_parameter(parameter: crate::analysis_parameter::AnalysisParameter) -> Self {
        Self {
            parameter,
            table: None,
        }
    }
}

impl Similarity {
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

    fn set_table(&mut self, gb: Option<&crate::graph_broker::GraphBroker>) {
        let mut table = Vec::new();
        let gb = gb.as_ref().unwrap();
        let c = &gb.get_abacus_by_group().c;
        let r = &gb.get_abacus_by_group().r;
        let v = gb.get_abacus_by_group().v.as_ref().unwrap();
        let tuples: Vec<(_, _)> = c.iter().map(|x| *x as usize).tuple_windows().collect();
        for i in 0..(tuples.len() - 1) {
            let mut row = vec![0.0; i];
            row.push(1.0);
            for j in (i + 1)..tuples.len() {
                let t1 = tuples[i];
                let t2 = tuples[j];
                let score = calc_score(
                    &r[t1.0..t1.1],
                    &r[t2.0..t2.1],
                    &v[t1.0..t1.1],
                    &v[t2.0..t2.1],
                );
                row.push(score);
            }
            table.push(row);
        }
        let mut last_row = vec![0.0; tuples.len() - 1];
        last_row.push(1.0);
        table.push(last_row);
        for i in 0..(table.len() - 1) {
            for j in (i + 1)..table.len() {
                table[j][i] = table[i][j];
            }
        }

        self.table = Some(table);
    }
}

fn get_table_string(table: &Vec<Vec<f32>>, groups: &Vec<String>) -> String {
    let mut res = String::new();
    res.push_str("group");
    for group in groups {
        res.push_str(&format!("\t{}", group));
    }
    res.push_str("\n");
    for (row_index, row) in table.iter().enumerate() {
        res.push_str(&groups[row_index]);
        for cell in row {
            res.push_str(&format!("\t{}", cell));
        }
        res.push_str("\n");
    }
    res
}

fn calc_score(r1: &[usize], r2: &[usize], v1: &[u32], v2: &[u32]) -> f32 {
    let mut s = 0;
    let mut d = 0;
    for (r1_idx, r1_value) in r1.iter().enumerate() {
        if let Some(r2_idx) = r2.iter().position(|el| el == r1_value) {
            s += std::cmp::min(v1[r1_idx], v2[r2_idx]);
            d += ((v1[r1_idx] as i64) - (v2[r2_idx] as i64)).abs() as u32;
        } else {
            d += v1[r1_idx];
        }
    }
    for (r2_idx, r2_value) in r2.iter().enumerate() {
        if r1.iter().position(|el| el == r2_value).is_none() {
            d += v2[r2_idx];
        }
    }
    (s as f32) / ((s + d) as f32)
}
