use std::collections::HashSet;
use std::fmt;
use std::fmt::Debug;
use std::fmt::Display;
use strum_macros::{EnumIter, EnumString, EnumVariantNames};

use serde::{Deserialize, Serialize};

use crate::analyses::ConstructibleAnalysis;
use crate::analyses::{
    coverage_line::CoverageLine, growth::Growth, info::Info, node_distribution::NodeDistribution,
    ordered_histgrowth::OrderedHistgrowth, similarity::Similarity,
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
        (Task::Analysis(Box::new(a)), reqs)
    }};
}

pub enum Task {
    Analysis(Box<dyn Analysis>),
    GraphStateChange {
        graph: String,
        reqs: HashSet<InputRequirement>,
        nice: bool,
        subset: String,
        exclude: String,
        grouping: Option<Grouping>,
    },
    OrderChange(Option<String>),
    AbacusByGroupCSCChange,
}

impl Debug for Task {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Analysis(analysis) => write!(f, "Analysis {}", analysis.get_type()),
            Self::GraphStateChange {
                graph,
                reqs,
                nice,
                subset,
                exclude,
                grouping,
            } => f
                .debug_tuple("GraphChange")
                .field(graph)
                .field(subset)
                .field(exclude)
                .field(grouping)
                .field(&reqs)
                .field(nice)
                .finish(),
            Self::OrderChange(order) => f.debug_tuple("OrderChange").field(&order).finish(),
            Self::AbacusByGroupCSCChange => f.debug_tuple("AbacusByGroupCSCChange").finish(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Hash, Clone, PartialOrd, Ord)]
pub struct AnalysisRun {
    graph: String,
    subset: String,
    exclude: String,
    grouping: Option<Grouping>,
    nice: bool,
    analyses: Vec<AnalysisParameter>,
}

impl AnalysisRun {
    pub fn convert_to_tasks(mut runs: Vec<Self>) -> Vec<Task> {
        runs.sort();
        let mut tasks = Vec::new();
        for i in 0..runs.len() {
            if i == 0 {
                let (current_tasks, mut input_req) = runs[i].to_tasks();
                input_req.insert(InputRequirement::Graph(std::mem::take(&mut runs[i].graph)));
                tasks.push(Task::GraphStateChange {
                    graph: std::mem::take(&mut runs[i].graph),
                    reqs: input_req,
                    nice: runs[i].nice,
                    subset: std::mem::take(&mut runs[i].subset),
                    exclude: std::mem::take(&mut runs[i].exclude),
                    grouping: std::mem::take(&mut runs[i].grouping),
                });
                tasks.extend(current_tasks);
            } else {
                unimplemented!("Haven't yet implemented multiple runs");
            }
        }
        tasks
    }

    pub fn to_tasks(&mut self) -> (Vec<Task>, HashSet<InputRequirement>) {
        let mut analyses = std::mem::take(&mut self.analyses);
        analyses.sort();
        let (tasks, requirements): (Vec<Task>, Vec<HashSet<InputRequirement>>) =
            analyses.into_iter().map(|a| a.into_task()).unzip();
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
    pub fn into_task(self) -> (Task, HashSet<InputRequirement>) {
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
            o @ Self::OrderedGrowth { .. } => {
                get_analysis_task!(OrderedHistgrowth, o)
            }
            c @ Self::CoverageLine { .. } => {
                get_analysis_task!(CoverageLine, c)
            }
            s @ Self::Similarity { .. } => {
                get_analysis_task!(Similarity, s)
            }
            _ => unimplemented!("Not yet done other analyses"),
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
