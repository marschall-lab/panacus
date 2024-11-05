// pub mod growth;
pub mod hist;
// pub mod histgrowth;
// pub mod info;
// pub mod ordered_histgrowth;
// pub mod table;

use std::{
    collections::HashSet,
    io::{BufWriter, Error, Write},
};

use crate::{
    analysis_parameter::AnalysisParameter,
    graph_broker::{GraphBroker, GraphMaskParameters},
    html_report::AnalysisSection,
};

pub trait Analysis {
    fn write_table<W: Write>(
        &mut self,
        gb: &GraphBroker,
        out: &mut BufWriter<W>,
    ) -> Result<(), Error>;
    fn generate_report_section(&mut self, gb: &GraphBroker) -> Vec<AnalysisSection>;
    fn from_parameter(parameter: AnalysisParameter) -> Self;
    fn get_graph_requirements(&self) -> HashSet<InputRequirement>;
}

#[derive(PartialEq, Eq, Debug, Clone, Hash)]
pub enum InputRequirement {
    Node,
    Edge,
    Bp,
    PathLens,
    Hist,
    AbacusByGroup,
    Graph(String),
    Subset(String),
}
