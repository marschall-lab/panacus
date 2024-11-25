/* private use */
pub mod analyses;
mod analysis_parameter;
mod commands;
pub mod graph_broker;
mod html_report;
mod io;
mod util;

use std::{
    collections::{HashMap, HashSet},
    fmt::Debug,
    io::Write,
};
use thiserror::Error;

use analyses::{Analysis, ConstructibleAnalysis, InputRequirement};
use analysis_parameter::AnalysisParameter;
use clap::Command;
use graph_broker::GraphBroker;
use html_report::AnalysisSection;

#[macro_export]
macro_rules! clap_enum_variants {
    // Credit: Johan Andersson (https://github.com/repi)
    // Code from https://github.com/clap-rs/clap/discussions/4264
    ($e: ty) => {{
        use clap::builder::TypedValueParser;
        use strum::VariantNames;
        clap::builder::PossibleValuesParser::new(<$e>::VARIANTS).map(|s| s.parse::<$e>().unwrap())
    }};
}

#[macro_export]
macro_rules! clap_enum_variants_no_all {
    ($e: ty) => {{
        use clap::builder::TypedValueParser;
        clap::builder::PossibleValuesParser::new(<$e>::VARIANTS.iter().filter(|&x| x != &"all"))
            .map(|s| s.parse::<$e>().unwrap())
    }};
}

#[macro_export]
macro_rules! some_or_return {
    ($x:expr, $y:expr) => {
        match $x {
            Some(v) => v,
            None => return $y,
        }
    };
}

pub fn run_cli() -> Result<(), anyhow::Error> {
    let mut out = std::io::BufWriter::new(std::io::stdout());

    // read parameters and store them in memory
    // let params = cli::read_params();
    // cli::set_number_of_threads(&params);
    let args = Command::new("panacus")
        .subcommand(commands::report::get_subcommand())
        .subcommand(commands::hist::get_subcommand())
        .subcommand(commands::growth::get_subcommand())
        .subcommand(commands::histgrowth::get_subcommand())
        .subcommand_required(true)
        .get_matches();

    let mut instructions = Vec::new();
    let mut shall_write_html = false;
    if let Some(report) = commands::report::get_instructions(&args) {
        shall_write_html = true;
        instructions.extend(report?);
    }
    if let Some(hist) = commands::hist::get_instructions(&args) {
        instructions.extend(hist?);
    }
    if let Some(growth) = commands::growth::get_instructions(&args) {
        instructions.extend(growth?);
    }
    if let Some(histgrowth) = commands::histgrowth::get_instructions(&args) {
        instructions.extend(histgrowth?);
    }

    let instructions = get_tasks(instructions)?;
    eprintln!("Tasks: {:?}", instructions);

    // ride on!
    execute_pipeline(instructions, &mut out, shall_write_html)?;

    // clean up & close down
    out.flush()?;
    Ok(())
}

#[derive(Error, Debug)]
pub enum ConfigParseError {
    #[error("no config block with name {name} was found")]
    NameNotFound { name: String },
}

fn get_tasks(instructions: Vec<AnalysisParameter>) -> anyhow::Result<Vec<Task>> {
    let instructions = preprocess_instructions(instructions)?;
    let mut tasks = Vec::new();
    let mut current_graph = "".to_string();
    let mut reqs = HashSet::new();
    let mut last_graph_change = 0usize;
    let mut current_subset = None;
    let mut current_exclude = String::new();
    let mut current_grouping = String::new();
    eprintln!("AnalysisParameters: {:?}", instructions);
    for instruction in instructions {
        match instruction {
            h @ AnalysisParameter::Hist { .. } => {
                if let AnalysisParameter::Hist {
                    graph,
                    subset,
                    exclude,
                    grouping,
                    ..
                } = &h
                {
                    let graph = graph.to_owned();
                    let subset = subset.to_owned();
                    let exclude = exclude.clone().unwrap_or_default();
                    let grouping = grouping.clone().unwrap_or_default();
                    if graph != current_graph {
                        tasks.push(Task::GraphChange(HashSet::new()));
                        tasks[last_graph_change] = Task::GraphChange(std::mem::take(&mut reqs));
                        last_graph_change = tasks.len() - 1;
                        current_graph = graph;
                    }
                    if subset != current_subset {
                        tasks.push(Task::SubsetChange(subset.clone()));
                        current_subset = subset;
                    }
                    if exclude != current_exclude {
                        tasks.push(Task::ExcludeChange(exclude.clone()));
                        current_exclude = exclude;
                    }
                    if grouping != current_grouping {
                        tasks.push(Task::GroupingChange(grouping.clone()));
                        current_grouping = grouping;
                    }
                }
                let hist = analyses::hist::Hist::from_parameter(h);
                reqs.extend(hist.get_graph_requirements());
                tasks.push(Task::Analysis(Box::new(hist)));
            }
            g @ AnalysisParameter::Growth { .. } => {
                tasks.push(Task::Analysis(Box::new(
                    analyses::growth::Growth::from_parameter(g),
                )));
            }
            section @ _ => panic!(
                "YAML section {:?} should not exist after preprocessing",
                section
            ),
        }
    }
    if matches!(tasks[last_graph_change], Task::GraphChange(..)) {
        tasks[last_graph_change] = Task::GraphChange(reqs);
    }
    Ok(tasks)
}

fn preprocess_instructions(
    instructions: Vec<AnalysisParameter>,
) -> anyhow::Result<Vec<AnalysisParameter>> {
    let graphs: HashMap<String, String> = instructions
        .iter()
        .filter_map(|instruct| match instruct {
            AnalysisParameter::Graph { name, file } => Some((name.to_string(), file.to_string())),
            _ => None,
        })
        .collect();
    let subsets: HashMap<String, String> = instructions
        .iter()
        .filter_map(|instruct| match instruct {
            AnalysisParameter::Subset { name, file } => Some((name.to_string(), file.to_string())),
            _ => None,
        })
        .collect();
    let groupings: HashMap<String, String> = instructions
        .iter()
        .filter_map(|instruct| match instruct {
            AnalysisParameter::Grouping { name, file } => {
                Some((name.to_string(), file.to_string()))
            }
            _ => None,
        })
        .collect();
    let instructions: Vec<AnalysisParameter> = instructions
        .into_iter()
        .filter(|instruct| !matches!(instruct, AnalysisParameter::Graph { .. }))
        .filter(|instruct| !matches!(instruct, AnalysisParameter::Subset { .. }))
        .filter(|instruct| !matches!(instruct, AnalysisParameter::Grouping { .. }))
        .map(|instruct| match instruct {
            AnalysisParameter::Hist {
                name,
                count_type,
                graph,
                display,
                subset,
                exclude,
                grouping,
            } => {
                let graph = if graphs.contains_key(&graph) {
                    graphs[&graph].to_string()
                } else {
                    graph
                };
                let subset = match subset {
                    Some(subset) => {
                        if subsets.contains_key(&subset) {
                            Some(subsets[&subset].to_string())
                        } else {
                            Some(subset)
                        }
                    }
                    None => None,
                };
                let grouping = match grouping {
                    Some(grouping) => {
                        if groupings.contains_key(&grouping) {
                            Some(groupings[&grouping].to_string())
                        } else {
                            Some(grouping)
                        }
                    }
                    None => None,
                };
                AnalysisParameter::Hist {
                    name,
                    count_type,
                    graph,
                    display,
                    subset,
                    exclude,
                    grouping,
                }
            }
            p @ _ => p,
        })
        .collect();
    let mut instructions: Vec<AnalysisParameter> = instructions;
    instructions.sort();
    let instructions = group_growths_to_hists(instructions)?;
    Ok(instructions)
}

fn group_growths_to_hists(
    instructions: Vec<AnalysisParameter>,
) -> anyhow::Result<Vec<AnalysisParameter>> {
    let mut instructions = instructions;
    while has_ungrouped_growth(&instructions) {
        group_first_ungrouped_growth(&mut instructions)?;
    }
    Ok(instructions)
}

fn group_first_ungrouped_growth(instructions: &mut Vec<AnalysisParameter>) -> anyhow::Result<()> {
    let index_growth = instructions
        .iter()
        .position(|i| matches!(i, AnalysisParameter::Growth { .. }))
        .expect("Instructions need to have at least one growth");
    let hist_name = match &instructions[index_growth] {
        AnalysisParameter::Growth { hist, .. } => hist.to_string(),
        _ => panic!("index_growth should point to growth"),
    };
    let growth_instruction = instructions.remove(index_growth);
    let index_hist = instructions
        .iter()
        .position(
            |i| matches!(i, AnalysisParameter::Hist { name: Some(name), .. } if name == &hist_name),
        )
        .ok_or(ConfigParseError::NameNotFound {
            name: hist_name.clone(),
        })?;
    instructions.insert(index_hist + 1, growth_instruction);
    Ok(())
}

fn has_ungrouped_growth(instructions: &Vec<AnalysisParameter>) -> bool {
    for i in instructions {
        match i {
            AnalysisParameter::Growth { hist, .. } => {
                // Growth can only be ungrouped if it does not use a .tsv hist
                if !hist.ends_with(".tsv") {
                    return true;
                } else {
                    continue;
                }
            }
            AnalysisParameter::Hist { .. } => {
                return false;
            }
            _ => {
                continue;
            }
        }
    }
    false
}

pub enum Task {
    Analysis(Box<dyn Analysis>),
    GraphChange(HashSet<InputRequirement>),
    SubsetChange(Option<String>),
    ExcludeChange(String),
    GroupingChange(String),
}

impl Debug for Task {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Analysis(_) => f.debug_struct("Analysis").finish(),
            Self::GraphChange(reqs) => f.debug_tuple("GraphChange").field(&reqs).finish(),
            Self::SubsetChange(subset) => f.debug_tuple("SubsetChange").field(&subset).finish(),
            Self::ExcludeChange(exclude) => f.debug_tuple("ExcludeChange").field(&exclude).finish(),
            Self::GroupingChange(grouping) => {
                f.debug_tuple("GroupingChange").field(&grouping).finish()
            }
        }
    }
}

pub fn execute_pipeline<W: Write>(
    mut instructions: Vec<Task>,
    out: &mut std::io::BufWriter<W>,
    shall_write_html: bool,
) -> anyhow::Result<()> {
    if instructions.is_empty() {
        log::warn!("No instructions supplied");
        return Ok(());
    }
    let mut report = Vec::new();
    let mut gb = match instructions[0] {
        _ => None,
    };
    for index in 0..instructions.len() {
        let is_next_analysis =
            instructions.len() > index + 1 && matches!(instructions[index + 1], Task::Analysis(..));
        match &mut instructions[index] {
            Task::Analysis(analysis) => {
                log::info!("Executing Analysis: {:?}", get_type_of(analysis));
                report.extend(analysis.generate_report_section(gb.as_ref())?);
            }
            Task::GraphChange(input_reqs) => {
                log::info!("Executing graph change: {:?}", input_reqs);
                gb = Some(GraphBroker::from_gfa(&input_reqs));
                if is_next_analysis {
                    gb = Some(gb.expect("GraphBroker is some").finish()?);
                }
            }
            Task::SubsetChange(subset) => {
                log::info!("Executing subset change: {:?}", subset);
                gb = Some(
                    gb.expect("SubsetChange after Graph")
                        .include_coords(subset.as_ref().expect("Subset exists")),
                );
                if is_next_analysis {
                    gb = Some(gb.expect("GraphBroker is some").finish()?);
                }
            }
            Task::ExcludeChange(exclude) => {
                log::info!("Executing exclude change: {}", exclude);
                gb = Some(
                    gb.expect("ExcludeChange after Graph")
                        .exclude_coords(exclude),
                );
                if is_next_analysis {
                    gb = Some(gb.expect("GraphBroker is some").finish()?);
                }
            }
            Task::GroupingChange(grouping) => {
                log::info!("Executing grouping change: {}", grouping);
                gb = Some(gb.expect("GroupingChange after Graph").with_group(grouping));
                if is_next_analysis {
                    gb = Some(gb.expect("GraphBroker is some").finish()?);
                }
            }
        }
    }
    if shall_write_html {
        let mut registry = handlebars::Handlebars::new();
        let report =
            AnalysisSection::generate_report(report, &mut registry, "<Placeholder Filename>")?;
        writeln!(out, "{report}")?;
    } else {
        if let Task::Analysis(analysis) = instructions.last_mut().unwrap() {
            let table = analysis.generate_table(gb.as_ref())?;
            writeln!(out, "{table}")?;
        }
    }
    Ok(())
}

fn get_type_of<T>(_: &T) -> String {
    format!("{}", std::any::type_name::<T>())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn get_hist_with_graph(graph: &str) -> AnalysisParameter {
        AnalysisParameter::Hist {
            name: None,
            count_type: util::CountType::Node,
            graph: graph.to_string(),
            display: false,
            subset: None,
            exclude: None,
            grouping: None,
        }
    }

    fn get_hist_with_subset(graph: &str, subset: &str) -> AnalysisParameter {
        AnalysisParameter::Hist {
            name: None,
            count_type: util::CountType::Node,
            graph: graph.to_string(),
            display: false,
            subset: Some(subset.to_string()),
            exclude: None,
            grouping: None,
        }
    }

    fn get_hist_with_exclude(graph: &str, exclude: &str) -> AnalysisParameter {
        AnalysisParameter::Hist {
            name: None,
            count_type: util::CountType::Node,
            graph: graph.to_string(),
            display: false,
            subset: None,
            exclude: Some(exclude.to_string()),
            grouping: None,
        }
    }

    fn get_hist_with_grouping(graph: &str, grouping: &str) -> AnalysisParameter {
        AnalysisParameter::Hist {
            name: None,
            count_type: util::CountType::Node,
            graph: graph.to_string(),
            display: false,
            subset: None,
            exclude: None,
            grouping: Some(grouping.to_string()),
        }
    }

    fn get_hist_with_name(name: &str) -> AnalysisParameter {
        AnalysisParameter::Hist {
            name: Some(name.to_string()),
            count_type: util::CountType::Node,
            graph: "test_graph".to_string(),
            display: false,
            subset: None,
            exclude: None,
            grouping: None,
        }
    }

    fn get_growth_with_hist(hist: &str) -> AnalysisParameter {
        AnalysisParameter::Growth {
            name: None,
            coverage: None,
            quorum: None,
            hist: hist.to_string(),
            display: false,
        }
    }

    #[test]
    fn test_replace_graph_name() {
        let instructions = vec![
            get_hist_with_graph("test_graph_name"),
            AnalysisParameter::Graph {
                name: "test_graph_name".to_string(),
                file: "test_graph_file".to_string(),
            },
        ];
        let expected = vec![get_hist_with_graph("test_graph_file")];
        let calculated = preprocess_instructions(instructions).unwrap();
        assert_eq!(calculated, expected);
    }

    #[test]
    fn test_replace_subset_name() {
        let instructions = vec![
            AnalysisParameter::Hist {
                name: None,
                count_type: util::CountType::Node,
                graph: "test".to_string(),
                display: false,
                subset: Some("test_subset".to_string()),
                exclude: None,
                grouping: None,
            },
            AnalysisParameter::Subset {
                name: "test_subset".to_string(),
                file: "subset_file.bed".to_string(),
            },
        ];
        let expected = vec![AnalysisParameter::Hist {
            name: None,
            count_type: util::CountType::Node,
            graph: "test".to_string(),
            display: false,
            subset: Some("subset_file.bed".to_string()),
            exclude: None,
            grouping: None,
        }];
        let calculated = preprocess_instructions(instructions).unwrap();
        assert_eq!(calculated, expected);
    }

    #[test]
    fn test_replace_grouping_name() {
        let instructions = vec![
            AnalysisParameter::Hist {
                name: None,
                count_type: util::CountType::Node,
                graph: "test".to_string(),
                display: false,
                subset: None,
                exclude: None,
                grouping: Some("test_grouping".to_string()),
            },
            AnalysisParameter::Grouping {
                name: "test_grouping".to_string(),
                file: "grouping_file.tsv".to_string(),
            },
        ];
        let expected = vec![AnalysisParameter::Hist {
            name: None,
            count_type: util::CountType::Node,
            graph: "test".to_string(),
            display: false,
            subset: None,
            exclude: None,
            grouping: Some("grouping_file.tsv".to_string()),
        }];
        let calculated = preprocess_instructions(instructions).unwrap();
        assert_eq!(calculated, expected);
    }

    #[test]
    fn test_sort_hist_by_name() {
        let instructions = vec![
            get_hist_with_name("B"),
            get_hist_with_name("Z"),
            get_hist_with_name("A"),
        ];
        let expected = vec![
            get_hist_with_name("A"),
            get_hist_with_name("B"),
            get_hist_with_name("Z"),
        ];
        let calculated = preprocess_instructions(instructions).unwrap();
        assert_eq!(calculated, expected);
    }

    #[test]
    fn test_sort_by_graph() {
        let instructions = vec![
            get_hist_with_graph("A"),
            get_hist_with_graph("B"),
            get_hist_with_graph("A"),
        ];
        let expected = vec![
            get_hist_with_graph("A"),
            get_hist_with_graph("A"),
            get_hist_with_graph("B"),
        ];
        let calculated = preprocess_instructions(instructions).unwrap();
        assert_eq!(calculated, expected);
    }

    #[test]
    fn test_sort_by_subset() {
        let instructions = vec![
            get_hist_with_subset("graph_a", "subset_a"),
            get_hist_with_subset("graph_b", "subset_a"),
            get_hist_with_subset("graph_a", "subset_b"),
            get_hist_with_subset("graph_a", "subset_a"),
        ];
        let expected = vec![
            get_hist_with_subset("graph_a", "subset_a"),
            get_hist_with_subset("graph_a", "subset_a"),
            get_hist_with_subset("graph_a", "subset_b"),
            get_hist_with_subset("graph_b", "subset_a"),
        ];
        let calculated = preprocess_instructions(instructions).unwrap();
        assert_eq!(calculated, expected);
    }

    #[test]
    fn test_sort_by_exclude() {
        let instructions = vec![
            get_hist_with_exclude("graph_a", "exclude_a"),
            get_hist_with_exclude("graph_b", "exclude_a"),
            get_hist_with_exclude("graph_a", "exclude_b"),
            get_hist_with_exclude("graph_a", "exclude_a"),
        ];
        let expected = vec![
            get_hist_with_exclude("graph_a", "exclude_a"),
            get_hist_with_exclude("graph_a", "exclude_a"),
            get_hist_with_exclude("graph_a", "exclude_b"),
            get_hist_with_exclude("graph_b", "exclude_a"),
        ];
        let calculated = preprocess_instructions(instructions).unwrap();
        assert_eq!(calculated, expected);
    }

    #[test]
    fn test_sort_by_grouping() {
        let instructions = vec![
            get_hist_with_grouping("graph_a", "grouping_a"),
            get_hist_with_grouping("graph_b", "grouping_a"),
            get_hist_with_grouping("graph_a", "grouping_b"),
            get_hist_with_grouping("graph_a", "grouping_a"),
        ];
        let expected = vec![
            get_hist_with_grouping("graph_a", "grouping_a"),
            get_hist_with_grouping("graph_a", "grouping_a"),
            get_hist_with_grouping("graph_a", "grouping_b"),
            get_hist_with_grouping("graph_b", "grouping_a"),
        ];
        let calculated = preprocess_instructions(instructions).unwrap();
        assert_eq!(calculated, expected);
    }

    #[test]
    fn test_group_growth_to_hist() {
        let instructions = vec![
            get_growth_with_hist("B"),
            get_growth_with_hist("C"),
            get_growth_with_hist("A"),
            get_hist_with_name("C"),
            get_hist_with_name("B"),
            get_hist_with_name("A"),
        ];
        let expected = vec![
            get_hist_with_name("A"),
            get_growth_with_hist("A"),
            get_hist_with_name("B"),
            get_growth_with_hist("B"),
            get_hist_with_name("C"),
            get_growth_with_hist("C"),
        ];
        let calculated = preprocess_instructions(instructions).unwrap();
        assert_eq!(calculated, expected);
    }
}
