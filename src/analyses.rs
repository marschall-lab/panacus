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

pub struct AnalysisSection {
    name: String,
    id: String,
    is_first: bool,
    tabs: Vec<AnalysisTab>,
}

impl AnalysisSection {
    fn set_first(mut self) -> Self {
        let mut is_first = true; 
        for tab in &mut self.tabs {
            if is_first {
                tab.is_first = true;
                is_first = false;
            } else {
                tab.is_first = false;
            }
        }
        self
    }
    fn into_html(self, registry: &mut Handlebars) -> Result<(String, String), RenderError> {
        if !registry.has_template("analysis_section") {
            registry.register_template_file("analysis_section", "./hbs/analysis_section.hbs")?;
        }
        let mut tab_navigation: Vec<HashMap<&str, handlebars::JsonValue>> = Vec::new();
        let mut tab_content: Vec<String> = Vec::new();
        let mut js_objects = Vec::new();
        for tab in self.tabs {
            let id = tab.id.clone();
            let name = tab.name.clone();
            let is_first = tab.is_first;
            let (cont, js_object) = tab.into_html(registry)?;
            js_objects.push(js_object);
            tab_navigation.push(HashMap::from([
                ("id", to_json(id)),
                ("name", to_json(name)),
                ("is_first", to_json(is_first)),
            ]));
            tab_content.push(cont);
        }
        let vars = HashMap::from([
            ("tab_navigation", to_json(tab_navigation)),
            ("tab_content", to_json(tab_content)),
        ]);
        let result = registry.render("analysis_section", &vars)?;
        Ok((
            result,
            js_objects
                .into_iter()
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
                .join(",\n"),
        ))
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
    pub fn generate_report(
        sections: Vec<Self>,
        registry: &mut Handlebars,
    ) -> Result<String, RenderError> {
        if !registry.has_template("report") {
            registry.register_template_file("report", "./hbs/report.hbs")?;
        }
        let (content, js_objects) = Self::generate_report_content(sections, registry)?;
        //eprintln!("{}", content);
        let mut vars = HashMap::from([("content", content), ("data_hook", js_objects)]);
        Self::populate_constants(&mut vars);
        registry.render("report", &vars)
    }

    fn generate_report_content(
        sections: Vec<Self>,
        registry: &mut Handlebars,
    ) -> Result<(String, String), RenderError> {
        if !registry.has_template("report_content") {
            registry.register_template_file("report_content", "./hbs/report_content.hbs")?;
        }
        let mut js_objects = Vec::new();
        let sections = sections
            .into_iter()
            .map(|s| {
                let is_first = to_json(s.is_first);
                let name = to_json(&s.name);
                let id = to_json(&s.id);
                let (content, js_object) = s.into_html(registry).unwrap();
                js_objects.push(js_object);
                HashMap::from([
                    ("is_first", is_first),
                    ("name", name),
                    ("id", id),
                    ("content", to_json(content)),
                ])
            })
            .collect::<Vec<HashMap<_, _>>>();
        let text = registry.render("report_content", &sections)?;
        eprintln!("{}", text);
        let js_objects = js_objects
            .into_iter()
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join(",\n");
        let js_objects = format!("const objects = [\n{}\n];", &js_objects);
        Ok((text, js_objects))
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
    fn into_html(self, registry: &mut Handlebars) -> Result<(String, String), RenderError> {
        if !registry.has_template("analysis_tab") {
            registry.register_template_file("analysis_tab", "./hbs/analysis_tab.hbs")?;
        }
        let items = self
            .items
            .into_iter()
            .map(|x| x.into_html(registry))
            .collect::<Result<Vec<_>, _>>()?;
        let (items, js_objects): (Vec<_>, Vec<_>) = items.into_iter().unzip();
        let js_objects = js_objects
            .into_iter()
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join(",\n");
        let vars = HashMap::from([
            ("id", to_json(&self.id)),
            ("name", to_json(&self.name)),
            ("items", to_json(items)),
            ("is_first", to_json(self.is_first)),
        ]);
        Ok((registry.render("analysis_tab", &vars)?, js_objects))
    }
}

pub enum ReportItem {
    Bar {
        id: String,
        name: String,
        x_label: String,
        y_label: String,
        labels: Vec<String>,
        values: Vec<f64>,
        log_toggle: bool,
    },
    Table {
        header: Vec<String>,
        values: Vec<Vec<String>>,
    },
}

impl ReportItem {
    fn into_html(self, registry: &mut Handlebars) -> Result<(String, String), RenderError> {
        match self {
            Self::Table { header, values } => {
                if !registry.has_template("table") {
                    registry.register_template_file("table", "./hbs/table.hbs")?;
                }
                let data = HashMap::from([
                    ("header".to_string(), to_json(header)),
                    ("values".to_string(), to_json(values)),
                ]);
                Ok((registry.render("table", &data)?, String::new()))
            }
            Self::Bar {
                id,
                name,
                x_label,
                y_label,
                labels,
                values,
                log_toggle,
            } => {
                if !registry.has_template("bar") {
                    registry.register_template_file("bar", "./hbs/bar.hbs")?;
                }
                let js_object = format!(
                    "new Bar('{}', '{}', '{}', '{}', {:?}, {:?}, {})",
                    id, name, x_label, y_label, labels, values, log_toggle
                );
                let data = HashMap::from([
                    ("name".to_string(), to_json(name)),
                    ("log_toggle".to_string(), to_json(log_toggle)),
                ]);
                Ok((registry.render("bar", &data)?, js_object))
            }
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
