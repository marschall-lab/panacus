use itertools::Itertools;
use kodama::{linkage, Dendrogram};
use rayon::iter::IntoParallelIterator;
use rayon::prelude::*;

use crate::{
    analyses::InputRequirement, analysis_parameter::AnalysisParameter, html_report::ReportItem,
    io::write_metadata_comments, util::CountType,
};
use core::panic;
use std::collections::HashSet;
use std::iter::FromIterator;
use std::usize;

use super::{Analysis, AnalysisSection, ConstructibleAnalysis};

pub struct Similarity {
    parameter: AnalysisParameter,
    table: Option<Vec<Vec<f32>>>,
    count: CountType,
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
        let mut req = HashSet::from([InputRequirement::AbacusByGroupCsc]);
        req.extend(Self::count_to_input_req(self.count));
        req
    }

    fn generate_report_section(
        &mut self,
        gb: Option<&crate::graph_broker::GraphBroker>,
    ) -> anyhow::Result<Vec<AnalysisSection>> {
        if self.table.is_none() {
            self.set_table(gb);
        }
        if gb.is_none() {
            panic!("Similarity analysis needs a graph")
        }
        let gb = gb.unwrap();
        let k = match self.parameter {
            AnalysisParameter::Similarity { count_type, .. } => count_type,
            _ => panic!("Similarity analysis needs Similarity parameter"),
        };
        let table = self.generate_table(Some(gb))?;
        let table = format!("`{}`", &table);
        let id_prefix = format!(
            "sim-heat-{}",
            self.get_run_name()
                .to_lowercase()
                .replace(&[' ', '|', '\\'], "-")
        );
        let tabs = vec![AnalysisSection {
            id: format!("{id_prefix}-{k}"),
            analysis: "Similarity Heatmap".to_string(),
            table: Some(table.clone()),
            run_name: self.get_run_name(),
            countable: k.to_string(),
            items: vec![ReportItem::Heatmap {
                id: format!("{id_prefix}-{k}"),
                name: gb.get_fname(),
                x_labels: gb.get_abacus_by_group().groups.clone(),
                y_labels: gb.get_abacus_by_group().groups.clone(),
                values: self.table.as_ref().unwrap().clone(),
            }],
        }];
        Ok(tabs)
    }
}

impl ConstructibleAnalysis for Similarity {
    fn from_parameter(parameter: crate::analysis_parameter::AnalysisParameter) -> Self {
        Self {
            count: match &parameter {
                AnalysisParameter::Similarity { count_type, .. } => *count_type,
                _ => panic!("Similarity analysis needs similarity parameter"),
            },
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
        let gb = gb.as_ref().unwrap();
        let c = &gb.get_abacus_by_group().r;
        let r = &gb.get_abacus_by_group().c;
        let tuples: Vec<(_, _)> = c.iter().map(|x| *x as usize).tuple_windows().collect();
        let sets: Vec<HashSet<_>> = tuples
            .iter()
            .map(|tuple| {
                let set = HashSet::from_iter(r[tuple.0..tuple.1].iter().cloned());
                set
            })
            .collect();
        log::info!("Finished building sets");
        let mut table: Vec<(usize, Vec<f32>)> = (0..(sets.len() - 1))
            .into_par_iter()
            .map(|i| {
                let mut row = vec![0.0; tuples.len()];
                row[i] = 1.0;
                for j in (i + 1)..sets.len() {
                    let score = if self.count != CountType::Bp {
                        sets[i].intersection(&sets[j]).count() as f32
                            / sets[i].union(&sets[j]).count() as f32
                    } else {
                        sets[i]
                            .intersection(&sets[j])
                            .map(|el| gb.get_node_lens()[*el as usize] as f32)
                            .sum::<f32>()
                            / sets[i]
                                .union(&sets[j])
                                .map(|el| gb.get_node_lens()[*el as usize] as f32)
                                .sum::<f32>()
                    };
                    let score = if score == -0.0 { 0.0 } else { score }; // Prevent -0 being
                                                                         // displayed
                    row[j] = score;
                }
                log::debug!("{}/{}", i, tuples.len());
                (i, row)
            })
            .collect();
        table.sort_by_key(|el| el.0);
        let mut table: Vec<Vec<f32>> = table.into_iter().map(|(_, row)| row).collect();
        let mut last_row = vec![0.0; tuples.len() - 1];
        last_row.push(1.0);
        table.push(last_row);
        for i in 0..(table.len() - 1) {
            for j in (i + 1)..table.len() {
                table[j][i] = table[i][j];
            }
        }

        let mut distances = calculate_distances(&table);
        let dend = linkage(&mut distances, table.len(), kodama::Method::Average);
        let order = get_order_from_dendrogram(&dend);
        let mut order = order.into_iter().enumerate().collect::<Vec<_>>();
        order.sort_by_key(|el| el.1);
        let order = order.into_iter().map(|el| el.0).collect::<Vec<_>>();
        sort_by_indices(&mut table, &order);
        for row in table.iter_mut() {
            sort_by_indices(row, &order);
        }

        self.table = Some(table);
    }

    fn get_run_name(&self) -> String {
        match &self.parameter {
            AnalysisParameter::Similarity {
                graph,
                subset,
                exclude,
                grouping,
                ..
            } => {
                format!(
                    "{}-{}|{}\\{}",
                    graph,
                    match grouping.clone() {
                        Some(g) => g.to_string(),
                        None => "Ungrouped".to_string(),
                    },
                    subset.clone().unwrap_or_default(),
                    exclude.clone().unwrap_or_default()
                )
            }
            _ => panic!("Hist analysis needs to contain hist parameter"),
        }
    }
}

fn sort_by_indices<T>(list: &mut Vec<T>, indices: &Vec<usize>) {
    let mut indices = indices.clone();
    for i in 0..indices.len() {
        while i != indices[i] {
            let new_i = indices[i];
            indices.swap(i, new_i);
            list.swap(i, new_i);
        }
    }
}

fn get_order_from_dendrogram(dend: &Dendrogram<f32>) -> Vec<usize> {
    let observations = dend.observations();
    let mut indices = Vec::new();
    for step in dend.steps() {
        if step.cluster1 < observations {
            indices.push(step.cluster1);
        }
        if step.cluster2 < observations {
            indices.push(step.cluster2);
        }
    }
    indices
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

fn euclidean(row1: &Vec<f32>, row2: &Vec<f32>) -> f32 {
    row1.iter()
        .zip(row2.iter())
        .map(|(v1, v2)| (v1 - v2).powf(2.0))
        .sum::<f32>()
        .sqrt()
}

fn calculate_distances(table: &Vec<Vec<f32>>) -> Vec<f32> {
    let mut condensed = vec![];
    for row in 0..table.len() - 1 {
        for col in row + 1..table.len() {
            condensed.push(euclidean(&table[row], &table[col]));
        }
    }
    condensed
}
