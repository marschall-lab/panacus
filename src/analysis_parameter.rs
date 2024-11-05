use crate::util::CountType;

pub enum AnalysisParameter {
    Hist {
        name: Option<String>,
        count_type: CountType,
        graph: String,
        display: bool,
        subset: Option<String>,
    },
    Subset {
        name: Option<String>,
    },
    Graph {
        name: Option<String>,
        file: String,
    },
    Info,
    Growth,
    OrderedGrowth,
    Table,
}
