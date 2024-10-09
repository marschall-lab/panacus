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

use crate::data_manager::{DataManager, ViewParams};

pub trait Analysis {
    fn build(dm: &DataManager, matches: &ArgMatches) -> Result<Box<Self>, Error>;
    fn write_table<W: Write>(
        &mut self,
        dm: &DataManager,
        out: &mut BufWriter<W>,
    ) -> Result<(), Error>;
    fn generate_report_section(&mut self, dm: &DataManager) -> ReportSection;
    fn get_subcommand() -> Command;
    fn get_input_requirements(
        matches: &ArgMatches,
    ) -> Option<(HashSet<InputRequirement>, ViewParams, String)>;
}

pub struct ReportSection {}

#[derive(PartialEq, Eq, Debug, Clone, Copy, Hash)]
pub enum InputRequirement {
    Node,
    Edge,
    Bp,
    PathLens,
    Hist,
    AbacusByGroup,
}
