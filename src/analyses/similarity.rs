use itertools::Itertools;
use kodama::{linkage, Dendrogram};
use rayon::iter::IntoParallelIterator;
use rayon::prelude::*;

use crate::graph_broker::GraphBroker;
use crate::{
    analyses::InputRequirement, analysis_parameter::AnalysisParameter, html_report::ReportItem,
    io::write_metadata_comments, util::CountType,
};
use core::panic;
use std::collections::{HashMap, HashSet};
use std::usize;

use super::{Analysis, AnalysisSection, ConstructibleAnalysis};

pub struct Similarity {
    parameter: AnalysisParameter,
    table: Option<Vec<Vec<f32>>>,
    labels: Option<Vec<String>>,
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
        let mut text = write_metadata_comments()?;
        let table = self.table.as_ref().unwrap();
        let labels = self.labels.as_ref().unwrap();
        text.push_str(&get_table_string(table, labels));
        Ok(text)
    }

    fn get_type(&self) -> String {
        "Similarity".to_string()
    }

    fn get_graph_requirements(&self) -> HashSet<InputRequirement> {
        let mut req = HashSet::from([InputRequirement::AbacusByGroup(self.count)]);
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
            self.get_run_name(gb)
                .to_lowercase()
                .replace(&[' ', '|', '\\'], "-")
        );
        let tabs = vec![AnalysisSection {
            id: format!("{id_prefix}-{k}"),
            analysis: "Similarity Heatmap".to_string(),
            table: Some(table.clone()),
            run_name: self.get_run_name(gb),
            countable: k.to_string(),
            items: vec![ReportItem::Heatmap {
                id: format!("{id_prefix}-{k}"),
                name: gb.get_fname(),
                x_labels: self.labels.as_ref().unwrap().clone(),
                y_labels: self.labels.as_ref().unwrap().clone(),
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
            labels: None,
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
        let r = &gb.get_abacus_by_group().r;
        let c = &gb.get_abacus_by_group().c;
        let mut labels = gb.get_abacus_by_group().groups.clone();

        let tuples: Vec<(_, _)> = r.iter().map(|x| *x as usize).tuple_windows().collect();

        let mut path_similarities: HashMap<u128, usize> = HashMap::new();
        let mut path_lens: HashMap<u64, usize> = HashMap::new();
        let node_lens = gb.get_node_lens();
        for (index, tuple) in tuples.iter().enumerate() {
            let node_length = node_lens[index] as usize;
            for x in &c[tuple.0..tuple.1] {
                if self.count == CountType::Bp {
                    *path_lens.entry(*x).or_insert(0) += node_length;
                } else {
                    *path_lens.entry(*x).or_insert(0) += 1;
                }
                for y in &c[tuple.0..tuple.1] {
                    if self.count == CountType::Bp {
                        *path_similarities
                            .entry((*x as u128) << 64 | *y as u128)
                            .or_insert(0) += node_length;
                    } else {
                        *path_similarities
                            .entry((*x as u128) << 64 | *y as u128)
                            .or_insert(0) += 1;
                    }
                }
            }
        }

        eprintln!("path_lens: {:?}", path_lens);

        let group_count = gb.get_group_count();
        let mut table: Vec<Vec<f32>> = vec![vec![0.0; group_count]; group_count];
        for i in 0..group_count {
            for j in 0..group_count {
                let intersection = path_similarities
                    .get(&((i as u128) << 64 | j as u128))
                    .copied()
                    .unwrap_or_default();
                table[i][j] = intersection as f32
                    / (path_lens[&(i as u64)] + path_lens[&(j as u64)] - intersection) as f32;
            }
        }

        let mut distances = calculate_distances(&table);

        let method = match self.parameter {
            AnalysisParameter::Similarity { cluster_method, .. } => cluster_method,
            _ => panic!("Similarity analysis needs to contain similarity parameter"),
        }
        .to_kodama();
        let dend = linkage(&mut distances, table.len(), method);
        let order = get_order_from_dendrogram(&dend);
        let mut order = order.into_iter().enumerate().collect::<Vec<_>>();
        order.sort_by_key(|el| el.1);
        let order = order.into_iter().map(|el| el.0).collect::<Vec<_>>();
        sort_by_indices(&mut table, &order);
        for row in table.iter_mut() {
            sort_by_indices(row, &order);
        }
        sort_by_indices(&mut labels, &order);

        self.table = Some(table);
        self.labels = Some(labels);
    }

    fn get_run_name(&self, gb: &GraphBroker) -> String {
        format!("{}-similarity", gb.get_run_name())
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
