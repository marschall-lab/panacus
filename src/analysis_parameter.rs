use std::collections::HashSet;
use std::fmt;
use std::fmt::Debug;
use std::fmt::Display;
use strum_macros::{EnumIter, EnumString, EnumVariantNames};

use serde::{Deserialize, Serialize};

use crate::analyses::ConstructibleAnalysis;
use crate::analyses::{
    coverage_line::CoverageLine, growth::Growth, info::Info, node_distribution::NodeDistribution,
    ordered_histgrowth::OrderedHistgrowth, similarity::Similarity, table::Table,
};
use crate::Analysis;
use crate::{
    analyses::{hist::Hist, InputRequirement},
    util::CountType,
};

macro_rules! get_analysis_task {
    ($t:ty, $v:expr) => {{
        let a = <$t>::from_parameter($v);
        let reqs = a.get_graph_requirements();
        let mut tasks = Vec::new();
        tasks.push(Task::Analysis(Box::new(a)));
        (tasks, reqs)
    }};
}

pub enum Task {
    Analysis(Box<dyn Analysis>),
    GraphStateChange {
        graph: String,
        name: Option<String>,
        reqs: HashSet<InputRequirement>,
        nice: bool,
        subset: String,
        exclude: String,
        grouping: Option<Grouping>,
    },
    OrderChange(Option<String>),
    AbacusByGroupCSCChange,
    CustomSection {
        name: String,
        file: String,
    },
}

impl Debug for Task {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Analysis(analysis) => write!(f, "Analysis {}", analysis.get_type()),
            Self::GraphStateChange {
                graph,
                name,
                reqs,
                nice,
                subset,
                exclude,
                grouping,
            } => f
                .debug_tuple("GraphStateChange")
                .field(graph)
                .field(name)
                .field(subset)
                .field(exclude)
                .field(grouping)
                .field(&reqs)
                .field(nice)
                .finish(),
            Self::OrderChange(order) => f.debug_tuple("OrderChange").field(&order).finish(),
            Self::AbacusByGroupCSCChange => f.debug_tuple("AbacusByGroupCSCChange").finish(),
            Self::CustomSection { name, file } => f
                .debug_tuple("CustomSection")
                .field(name)
                .field(file)
                .finish(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Hash, Clone, PartialOrd, Ord)]
pub struct AnalysisRun {
    graph: String,
    name: Option<String>,
    #[serde(default)]
    subset: String,
    #[serde(default)]
    exclude: String,
    grouping: Option<Grouping>,
    #[serde(default)]
    nice: bool,
    analyses: Vec<AnalysisParameter>,
}

impl AnalysisRun {
    pub fn new(
        graph: String,
        name: Option<String>,
        subset: String,
        exclude: String,
        grouping: Option<Grouping>,
        nice: bool,
        analyses: Vec<AnalysisParameter>,
    ) -> Self {
        Self {
            graph,
            name,
            subset,
            exclude,
            grouping,
            nice,
            analyses,
        }
    }
    pub fn get_example() -> Self {
        Self {
            graph: "../simple_files/pggb/chr18.gfa".to_string(),
            name: None,
            subset: String::new(),
            exclude: String::new(),
            grouping: Some(Grouping::Haplotype),
            nice: true,
            analyses: vec![
                AnalysisParameter::Hist {
                    count_type: CountType::Bp,
                },
                AnalysisParameter::Growth {
                    coverage: Some("1,1,2".to_string()),
                    quorum: Some("0,0.9,0".to_string()),
                    add_hist: false,
                },
                AnalysisParameter::Info,
                AnalysisParameter::NodeDistribution { radius: 20 },
            ],
        }
    }

    pub fn convert_to_tasks(mut runs: Vec<Self>) -> Vec<Task> {
        runs.sort();
        let mut tasks = Vec::new();
        for i in 0..runs.len() {
            let (current_tasks, mut input_req) = runs[i].to_tasks();
            input_req.insert(InputRequirement::Graph(runs[i].graph.clone()));
            tasks.push(Task::GraphStateChange {
                graph: std::mem::take(&mut runs[i].graph),
                name: std::mem::take(&mut runs[i].name),
                reqs: input_req,
                nice: runs[i].nice,
                subset: std::mem::take(&mut runs[i].subset),
                exclude: std::mem::take(&mut runs[i].exclude),
                grouping: std::mem::take(&mut runs[i].grouping),
            });
            tasks.extend(current_tasks);
        }
        tasks
    }

    pub fn to_tasks(&mut self) -> (Vec<Task>, HashSet<InputRequirement>) {
        let mut analyses = std::mem::take(&mut self.analyses);
        analyses.sort();
        let (tasks, requirements): (Vec<Vec<Task>>, Vec<HashSet<InputRequirement>>) =
            analyses.into_iter().map(|a| a.into_tasks()).unzip();
        let tasks: Vec<Task> = tasks.into_iter().flatten().collect();
        let requirements: HashSet<InputRequirement> =
            requirements
                .into_iter()
                .fold(HashSet::new(), |mut acc, el| {
                    acc.extend(el);
                    acc
                });
        (tasks, requirements)
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Hash, Clone, PartialOrd, Ord)]
pub enum AnalysisParameter {
    Hist {
        #[serde(default)]
        count_type: CountType,
    },
    Growth {
        coverage: Option<String>,
        quorum: Option<String>,
        #[serde(default)]
        add_hist: bool,
    },
    Table {
        #[serde(default)]
        count_type: CountType,

        total: bool,
        order: Option<String>,
    },
    NodeDistribution {
        #[serde(default = "get_radius")]
        radius: u32,
    },
    Info,
    OrderedGrowth {
        coverage: Option<String>,
        quorum: Option<String>,
        order: Option<String>,

        #[serde(default)]
        count_type: CountType,
    },
    CoverageLine {
        #[serde(default)]
        count_type: CountType,
        reference: String,
    },
    Similarity {
        #[serde(default)]
        count_type: CountType,
        #[serde(default)]
        cluster_method: ClusterMethod,
    },
    Custom {
        name: String,
        file: String,
    },
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Hash, Clone, PartialOrd, Ord)]
pub enum Grouping {
    Sample,
    Haplotype,
    Custom(String),
}

impl Display for Grouping {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Sample => write!(f, "Group By Sample"),
            Self::Haplotype => write!(f, "Group By Haplotype"),
            Self::Custom(file) => write!(f, "Group By {}", file),
        }
    }
}

fn get_radius() -> u32 {
    20
}

impl AnalysisParameter {
    pub fn into_tasks(self) -> (Vec<Task>, HashSet<InputRequirement>) {
        match self {
            h @ Self::Hist { .. } => {
                get_analysis_task!(Hist, h)
            }
            g @ Self::Growth { .. } => {
                get_analysis_task!(Growth, g)
            }
            n @ Self::NodeDistribution { .. } => {
                get_analysis_task!(NodeDistribution, n)
            }
            i @ Self::Info => {
                get_analysis_task!(Info, i)
            }
            ref o @ Self::OrderedGrowth { ref order, .. } => {
                let mut tasks = vec![Task::OrderChange(order.clone())];
                let (ordered_task, reqs) = get_analysis_task!(OrderedHistgrowth, o.clone());
                tasks.extend(ordered_task);
                (tasks, reqs)
            }
            c @ Self::CoverageLine { .. } => {
                get_analysis_task!(CoverageLine, c)
            }
            s @ Self::Similarity { .. } => {
                get_analysis_task!(Similarity, s)
            }
            t @ Self::Table { .. } => {
                get_analysis_task!(Table, t)
            }
            Self::Custom { name, file } => {
                (vec![Task::CustomSection { name, file }], HashSet::new())
            }
        }
    }
}

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    EnumString,
    EnumVariantNames,
    EnumIter,
    Hash,
    Eq,
    PartialOrd,
    Ord,
    Serialize,
    Deserialize,
)]
#[strum(serialize_all = "lowercase")]
pub enum ClusterMethod {
    Single,
    Complete,
    Average,
    Weighted,
    Ward,
    Centroid,
    Median,
}

impl Default for ClusterMethod {
    fn default() -> Self {
        Self::Centroid
    }
}

impl ClusterMethod {
    pub fn to_kodama(self) -> kodama::Method {
        match self {
            Self::Single => kodama::Method::Single,
            Self::Complete => kodama::Method::Complete,
            Self::Average => kodama::Method::Average,
            Self::Weighted => kodama::Method::Weighted,
            Self::Ward => kodama::Method::Ward,
            Self::Centroid => kodama::Method::Centroid,
            Self::Median => kodama::Method::Median,
        }
    }
}

impl fmt::Display for ClusterMethod {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        write!(
            formatter,
            "{}",
            match self {
                Self::Single => "single",
                Self::Complete => "complete",
                Self::Average => "average",
                Self::Weighted => "weighted",
                Self::Ward => "ward",
                Self::Centroid => "centroid",
                Self::Median => "median",
            }
        )
    }
}
