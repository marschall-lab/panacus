pub mod info;
pub mod hist;

use std::collections::HashSet;

use clap::{ArgMatches, Command};

use crate::{abacus::ViewParams, data_manager::DataManager};

pub trait Analysis {
    fn build(dm: &DataManager) -> Self;
    fn generate_table(&mut self, dm: &DataManager) -> String;
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
}
