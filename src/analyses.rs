pub mod growth;
pub mod hist;
pub mod histgrowth;
pub mod info;
pub mod ordered_histgrowth;
pub mod table;

use std::{
    collections::HashSet,
    io::{BufWriter, Error, Write},
};

use clap::{ArgMatches, Command};

use crate::{
    graph_broker::{GraphBroker, GraphMaskParameters},
    html_report::AnalysisSection,
};

pub trait Analysis {
    fn build(gb: &GraphBroker, matches: &ArgMatches) -> Result<Box<Self>, Error>;
    fn write_table<W: Write>(
        &mut self,
        gb: &GraphBroker,
        out: &mut BufWriter<W>,
    ) -> Result<(), Error>;
    fn generate_report_section(&mut self, gb: &GraphBroker) -> Vec<AnalysisSection>;
    fn get_subcommand() -> Command;
    fn get_input_requirements(
        matches: &ArgMatches,
    ) -> Option<(HashSet<InputRequirement>, GraphMaskParameters, String)>;
}

#[derive(PartialEq, Eq, Debug, Clone, Copy, Hash)]
pub enum InputRequirement {
    Node,
    Edge,
    Bp,
    PathLens,
    Hist,
    AbacusByGroup,
}
