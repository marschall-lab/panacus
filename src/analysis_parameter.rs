use serde::{Deserialize, Serialize};

use crate::util::CountType;

#[derive(Serialize, Deserialize, Debug)]
pub enum AnalysisParameter {
    Hist {
        name: Option<String>,

        #[serde(default)]
        count_type: CountType,
        graph: String,

        #[serde(default = "get_true")]
        display: bool,

        subset: Option<String>,
        exclude: Option<String>,
        grouping: Option<String>,
    },
    Subset {
        name: String,
        file: String,
    },
    Graph {
        name: String,
        file: String,
    },
    Grouping {
        name: String,
        file: String,
    },
    Info,
    Growth,
    OrderedGrowth,
    Table,
}

fn get_true() -> bool {
    true
}
