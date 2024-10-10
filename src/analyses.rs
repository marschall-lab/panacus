pub mod growth;
pub mod hist;
pub mod histgrowth;
pub mod info;
pub mod ordered_histgrowth;
pub mod table;

use std::{
    collections::{HashMap, HashSet},
    io::{BufWriter, Error, Write},
};

use base64::{engine::general_purpose, Engine};
use handlebars::{to_json, Handlebars, RenderError};

use clap::{ArgMatches, Command};
use time::{macros::format_description, OffsetDateTime};

use crate::data_manager::{DataManager, ViewParams};

pub trait Analysis {
    fn build(dm: &DataManager, matches: &ArgMatches) -> Result<Box<Self>, Error>;
    fn write_table<W: Write>(
        &mut self,
        dm: &DataManager,
        out: &mut BufWriter<W>,
    ) -> Result<(), Error>;
    fn generate_report_section(&mut self, dm: &DataManager) -> Vec<AnalysisSection>;
    fn get_subcommand() -> Command;
    fn get_input_requirements(
        matches: &ArgMatches,
    ) -> Option<(HashSet<InputRequirement>, ViewParams, String)>;
}

pub trait IntoHtml {
    fn into_html(self, registry: &mut Handlebars) -> Result<String, RenderError>;
}

pub struct AnalysisSection {
    name: String,
    tabs: Vec<AnalysisTab>,
}

impl IntoHtml for AnalysisSection {
    fn into_html(self, registry: &mut Handlebars) -> Result<String, RenderError> {
        if !registry.has_template("analysis_section") {
            registry.register_template_file("analysis_section", "./hbs/analysis_section.hbs")?;
        }
        let mut tab_navigation: Vec<HashMap<&str, handlebars::JsonValue>> = Vec::new();
        let mut tab_content: Vec<String> = Vec::new();
        for tab in self.tabs {
            let (cont, id, name, is_first) = tab.into_html(registry)?;
            tab_navigation.push(HashMap::from([
                    ("id", to_json(id)),
                    ("name", to_json(name)),
                    ("is_first", to_json(is_first)),
            ]));
            tab_content.push(cont);
        } 
        let vars = HashMap::from([
            ("tab_navigation", to_json(tab_navigation)),
            ("tab_content", to_json(tab_content))
        ]);
        let result = registry.render("analysis_section", &vars)?;
        Ok(result)
    }
}

pub const BOOTSTRAP_COLOR_MODES_JS: &[u8] = include_bytes!("../etc/color-modes.min.js");
pub const BOOTSTRAP_CSS: &[u8] = include_bytes!("../etc/bootstrap.min.css");
pub const BOOTSTRAP_JS: &[u8] = include_bytes!("../etc/bootstrap.bundle.min.js");
pub const CHART_JS: &[u8] = include_bytes!("../etc/chart.js");
pub const CUSTOM_CSS: &[u8] = include_bytes!("../etc/custom.css");
pub const CUSTOM_LIB_JS: &[u8] = include_bytes!("../etc/lib.min.js");
pub const HOOK_AFTER_JS: &[u8] = include_bytes!("../etc/hook_after.min.js");
pub const PANACUS_LOGO: &[u8] = include_bytes!("../etc/panacus-illustration-small.png");
pub const SYMBOLS_SVG: &[u8] = include_bytes!("../etc/symbols.svg");

impl AnalysisSection {
    pub fn generate_report(sections: Vec<Self>, registry: &mut Handlebars) -> Result<String, RenderError> {
        if !registry.has_template("report") {
            registry.register_template_file("report", "./hbs/report.hbs")?;
        }
        let content = Self::generate_report_content(sections, registry)?;
        //eprintln!("{}", content);
        let mut vars = HashMap::from([
            ("content", content),
        ]);
        Self::populate_constants(&mut vars);
        registry.render("report", &vars)
    }

    fn generate_report_content(sections: Vec<Self>, registry: &mut Handlebars) -> Result<String, RenderError> {
        if !registry.has_template("report_content") {
            registry.register_template_file("report_content", "./hbs/report_content.hbs")?;
        }
        let sections = sections.into_iter().map(|s| s.into_html(registry)).collect::<Result<Vec<String>, RenderError>>()?;
        let text = registry.render("report_content", &sections)?;
        eprintln!("{}", text);
        Ok(text)
    }


pub fn populate_constants(vars: &mut HashMap<&str, String>) {
    vars.insert(
        "bootstrap_color_modes_js",
        String::from_utf8_lossy(BOOTSTRAP_COLOR_MODES_JS).into_owned(),
    );
    vars.insert(
        "bootstrap_css",
        String::from_utf8_lossy(BOOTSTRAP_CSS).into_owned(),
    );
    vars.insert(
        "bootstrap_js",
        String::from_utf8_lossy(BOOTSTRAP_JS).into_owned(),
    );
    vars.insert("chart_js", String::from_utf8_lossy(CHART_JS).into_owned());
    vars.insert(
        "custom_css",
        String::from_utf8_lossy(CUSTOM_CSS).into_owned(),
    );
    vars.insert(
        "custom_lib_js",
        String::from_utf8_lossy(CUSTOM_LIB_JS).into_owned(),
    );
    vars.insert(
        "hook_after_js",
        String::from_utf8_lossy(HOOK_AFTER_JS).into_owned(),
    );
    vars.insert(
        "panacus_logo",
        general_purpose::STANDARD_NO_PAD.encode(PANACUS_LOGO),
    );
    vars.insert(
        "symbols_svg",
        String::from_utf8_lossy(SYMBOLS_SVG).into_owned(),
    );
    vars.insert("version", env!("CARGO_PKG_VERSION").to_string());

    let now = OffsetDateTime::now_utc();
    vars.insert(
        "timestamp",
        now.format(&format_description!(
            "[year]-[month]-[day]T[hour]:[minute]:[second]Z"
        ))
        .unwrap(),
    );
}
}

pub struct AnalysisTab {
    name: String,
    id: String,
    is_first: bool,
    items: Vec<ReportItem>,
}

impl AnalysisTab {
    fn into_html(self, registry: &mut Handlebars) -> Result<(String, String, String, bool), RenderError> {
        if !registry.has_template("analysis_tab") {
            registry.register_template_file("analysis_tab", "./hbs/analysis_tab.hbs");
        }
        let items = self.items.into_iter().map(|x| x.into_html(registry)).collect::<Result<Vec<_>, _>>()?;
        let vars = HashMap::from([
            ("id", to_json(&self.id)),
            ("name", to_json(&self.name)),
            ("items", to_json(items)),
            ("is_first", to_json(self.is_first)),
        ]);
        Ok((registry.render("analysis_tab", &vars)?, self.id, self.name, self.is_first))
    }
}

pub enum ReportItem {
    Hist,
    Bar,
    Table {
        header: Vec<String>,
        values: Vec<Vec<String>>,
    },
}

impl IntoHtml for ReportItem {
    fn into_html(self, registry: &mut Handlebars) -> Result<String, RenderError> {
        match self {
            Self::Table { header, values } => {
                if !registry.has_template("table") {
                    registry.register_template_file("table", "./hbs/table.hbs");
                }
                let data = HashMap::from([
                    ("header".to_string(), to_json(header)),
                    ("values".to_string(), to_json(values))
                ]);
                registry.render("table", &data)
            },
            _ => Ok(String::new()),
        }
    }
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
