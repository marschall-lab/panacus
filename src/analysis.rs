use std::collections::HashSet;

use clap::Command;

pub trait Analysis {
    fn run(&mut self);
    fn generate_table(&mut self) -> String;
    fn generate_report_section(&mut self) -> ReportSection;
    fn get_subcommand() -> Command;
    fn get_input_requirements() -> HashSet<InputRequirement>;
}

pub struct ReportSection {}

#[derive(PartialEq, Eq, Debug, Clone, Copy, Hash)]
pub enum InputRequirement {
    Ga,
    GaEdge,
    PwNode,
    PwBp,
    PwEdge,
    PwAll,
}
