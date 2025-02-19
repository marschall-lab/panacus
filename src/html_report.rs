use std::{collections::HashMap, str::from_utf8};
use std::{f64, fmt};

use base64::{engine::general_purpose, Engine};
use handlebars::{to_json, Handlebars, RenderError};

use itertools::Itertools;
use time::{macros::format_description, OffsetDateTime};

use crate::util::to_id;

type JsVars = HashMap<String, HashMap<String, String>>;
type RenderedHTML = Result<(String, JsVars), RenderError>;

fn combine_vars(mut a: JsVars, b: JsVars) -> JsVars {
    for (k, v) in b {
        if let Some(x) = a.get_mut(&k) {
            x.extend(v);
        }
    }
    a
}

pub struct AnalysisSection {
    pub analysis: String,
    pub run_name: String,
    pub countable: String,
    pub items: Vec<ReportItem>,
    pub id: String,
    pub table: Option<String>,
}

impl AnalysisSection {
    fn into_html(self, registry: &mut Handlebars) -> RenderedHTML {
        if !registry.has_template("analysis_tab") {
            registry
                .register_template_string("analysis_tab", from_utf8(ANALYSIS_TAB_HBS).unwrap())?;
        }
        let items = self
            .items
            .into_iter()
            .map(|x| x.into_html(registry))
            .collect::<Result<Vec<_>, _>>()?;
        let (items, mut js_objects): (Vec<_>, Vec<_>) = items.into_iter().unzip();
        if let Some(table) = &self.table {
            if !js_objects.is_empty() {
                js_objects[0].insert(
                    "tables".to_string(),
                    HashMap::from([(self.id.clone(), table.clone())]),
                );
            }
        }
        let js_objects = js_objects
            .into_iter()
            .reduce(combine_vars)
            .expect("Tab has at least one item");
        let vars = HashMap::from([
            ("id", to_json(&self.id)),
            ("analysis", to_json(&self.analysis)),
            ("run_name", to_json(&self.run_name)),
            ("countable", to_json(&self.countable)),
            ("has_table", to_json(self.table.is_some())),
            ("has_graph", to_json(true)), // TODO: real check for graph
            ("items", to_json(items)),
        ]);
        Ok((registry.render("analysis_tab", &vars)?, js_objects))
    }
}

pub const BOOTSTRAP_COLOR_MODES_JS: &[u8] = include_bytes!("../etc/color-modes.min.js");
pub const BOOTSTRAP_CSS: &[u8] = include_bytes!("../etc/bootstrap.min.css");
pub const BOOTSTRAP_JS: &[u8] = include_bytes!("../etc/bootstrap.bundle.min.js");
pub const CHART_JS: &[u8] = include_bytes!("../etc/chart.js");
pub const D3_JS: &[u8] = include_bytes!("../etc/d3.v7.min.js");
pub const D3_HEXBIN_JS: &[u8] = include_bytes!("../etc/d3-hexbin.v0.2.min.js");
pub const D3_LEGEND_JS: &[u8] = include_bytes!("../etc/d3-legend.min.js");
pub const CHART_JS_MATRIX: &[u8] = include_bytes!("../etc/chartjs-chart-matrix.min.js");
pub const CUSTOM_CSS: &[u8] = include_bytes!("../etc/custom.css");
pub const CUSTOM_LIB_JS: &[u8] = include_bytes!("../etc/lib.min.js");
pub const HOOK_AFTER_JS: &[u8] = include_bytes!("../etc/hook_after.min.js");
pub const PANACUS_LOGO: &[u8] = include_bytes!("../etc/panacus-illustration-small.png");
pub const SYMBOLS_SVG: &[u8] = include_bytes!("../etc/symbols.svg");

pub const REPORT_HBS: &[u8] = include_bytes!("../hbs/report.hbs");
pub const BAR_HBS: &[u8] = include_bytes!("../hbs/bar.hbs");
pub const TREE_HBS: &[u8] = include_bytes!("../hbs/tree.hbs");
pub const TABLE_HBS: &[u8] = include_bytes!("../hbs/table.hbs");
pub const HEATMAP_HBS: &[u8] = include_bytes!("../hbs/heatmap.hbs");
pub const ANALYSIS_TAB_HBS: &[u8] = include_bytes!("../hbs/analysis_tab.hbs");
pub const REPORT_CONTENT_HBS: &[u8] = include_bytes!("../hbs/report_content.hbs");

fn get_js_objects_string(objects: JsVars) -> String {
    let mut res = String::from("{");
    for (k, v) in objects {
        res.push('"');
        res.push_str(&k);
        res.push_str("\": {");
        for (subkey, subvalue) in v {
            res.push('"');
            res.push_str(&subkey);
            res.push_str("\": ");
            res.push_str(&subvalue);
            res.push_str(", ");
        }
        res.push_str("}, ");
    }
    res.push('}');
    res
}

impl AnalysisSection {
    pub fn generate_report(
        sections: Vec<Self>,
        registry: &mut Handlebars,
        filename: &str,
    ) -> Result<String, RenderError> {
        if !registry.has_template("report") {
            registry.register_template_string("report", from_utf8(REPORT_HBS).unwrap())?;
        }

        let tree = Self::get_tree(&sections, registry)?;

        let (content, js_objects) = Self::generate_report_content(sections, registry)?;
        let mut vars = Self::get_variables();
        vars.insert("content", content);
        vars.insert("data_hook", get_js_objects_string(js_objects));
        vars.insert("fname", filename.to_string());
        vars.insert("tree", tree);
        registry.render("report", &vars)
    }

    fn get_tree(sections: &Vec<Self>, registry: &mut Handlebars) -> Result<String, RenderError> {
        let analysis_names = sections.iter().map(|x| x.analysis.clone()).unique();
        let mut analyses = Vec::new();
        for analysis_name in analysis_names {
            let run_names = sections
                .iter()
                .filter(|x| x.analysis == analysis_name)
                .map(|x| x.run_name.clone())
                .unique();
            let analysis_sections = sections
                .iter()
                .filter(|x| x.analysis == analysis_name)
                .collect::<Vec<_>>();
            let mut runs = Vec::new();
            for run_name in run_names {
                let run_sections = analysis_sections
                    .iter()
                    .filter(|x| x.run_name == run_name)
                    .collect::<Vec<_>>();
                if run_sections.is_empty() {
                    continue;
                }
                let mut countables = Vec::new();
                for section in &run_sections {
                    let content = HashMap::from([
                        ("title", to_json(&section.countable)),
                        ("id", to_json(to_id(&section.countable))),
                        ("href", to_json(&section.id)),
                    ]);
                    countables.push(to_json(content));
                }
                let run_name = run_sections
                    .first()
                    .expect("Run section has at least one run")
                    .run_name
                    .clone();
                let content = HashMap::from([
                    ("title", to_json(&run_name)),
                    ("id", to_json(to_id(&run_name))),
                    ("countables", to_json(countables)),
                ]);
                runs.push(to_json(content));
            }
            let content = HashMap::from([
                ("title", to_json(&analysis_name)),
                ("id", to_json(to_id(&analysis_name))),
                ("icon", to_json("icon-id")),
                ("runs", to_json(runs)),
            ]);
            analyses.push(to_json(content));
        }

        let mut vars = HashMap::from([("analyses", to_json(analyses))]);
        vars.insert(
            "version",
            to_json(
                option_env!("GIT_HASH")
                    .unwrap_or(env!("CARGO_PKG_VERSION"))
                    .to_string(),
            ),
        );
        let now = OffsetDateTime::now_utc();
        vars.insert(
            "timestamp",
            to_json(
                now.format(&format_description!(
                    "[year]-[month]-[day]T[hour]:[minute]:[second]Z"
                ))
                .unwrap(),
            ),
        );
        if !registry.has_template("tree") {
            registry.register_template_string("tree", from_utf8(TREE_HBS).unwrap())?;
        }
        let tree = registry.render("tree", &vars)?;
        Ok(tree)
    }

    fn get_variables() -> HashMap<&'static str, String> {
        let mut vars = HashMap::new();
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
        vars.insert("d3_js", String::from_utf8_lossy(D3_JS).into_owned());
        vars.insert(
            "d3_hexbin_js",
            String::from_utf8_lossy(D3_HEXBIN_JS).into_owned(),
        );
        vars.insert(
            "d3_legend_js",
            String::from_utf8_lossy(D3_LEGEND_JS).into_owned(),
        );
        vars.insert(
            "chart_js_matrix",
            String::from_utf8_lossy(CHART_JS_MATRIX).into_owned(),
        );
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
        vars
    }

    fn generate_report_content(sections: Vec<Self>, registry: &mut Handlebars) -> RenderedHTML {
        if !registry.has_template("report_content") {
            registry.register_template_string(
                "report_content",
                from_utf8(REPORT_CONTENT_HBS).unwrap(),
            )?;
        }
        let mut js_objects = Vec::new();
        let sections = sections
            .into_iter()
            .map(|s| {
                let (content, js_object) = s.into_html(registry).unwrap();
                js_objects.push(js_object);
                content
            })
            .collect::<Vec<String>>();
        let text = registry.render("report_content", &sections)?;
        let js_objects = js_objects
            .into_iter()
            .reduce(combine_vars)
            .expect("Report needs to contain at least one item");
        Ok((text, js_objects))
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
    MultiBar {
        id: String,
        names: Vec<String>,
        x_label: String,
        y_label: String,
        labels: Vec<String>,
        values: Vec<Vec<f64>>,
        log_toggle: bool,
    },
    Table {
        id: String,
        header: Vec<String>,
        values: Vec<Vec<String>>,
    },
    Hexbin {
        id: String,
        bins: Vec<Bin>,
        min: (u32, f64),
        max: (u32, f64),
        radius: u32,
    },
    Heatmap {
        id: String,
        name: String,
        x_labels: Vec<String>,
        y_labels: Vec<String>,
        values: Vec<Vec<f32>>,
    },
}

impl ReportItem {
    fn into_html(self, registry: &mut Handlebars) -> RenderedHTML {
        match self {
            Self::Table { id, header, values } => {
                if !registry.has_template("table") {
                    registry.register_template_string("table", from_utf8(TABLE_HBS).unwrap())?;
                }
                let data = HashMap::from([
                    ("id".to_string(), to_json(id)),
                    ("header".to_string(), to_json(header)),
                    ("values".to_string(), to_json(values)),
                ]);
                Ok((
                    registry.render("table", &data)?,
                    HashMap::from([("datasets".to_string(), HashMap::new())]),
                ))
            }
            Self::Heatmap {
                id,
                name,
                x_labels,
                y_labels,
                values,
            } => {
                if !registry.has_template("heatmap") {
                    registry
                        .register_template_string("heatmap", from_utf8(HEATMAP_HBS).unwrap())?;
                }
                let js_object = format!(
                    "new Heatmap('{}', '{}', {:?}, {:?}, {:?})",
                    id, name, x_labels, y_labels, values,
                );
                let max_scale = format!(
                    "{:.2}",
                    values
                        .iter()
                        .flatten()
                        .fold(f32::INFINITY, |a, &b| a.min(b))
                );
                let data = HashMap::from([
                    ("id".to_string(), to_json(&id)),
                    ("max".to_string(), to_json(max_scale)),
                ]);
                Ok((
                    registry.render("heatmap", &data)?,
                    HashMap::from([(
                        "datasets".to_string(),
                        HashMap::from([(id.clone(), js_object)]),
                    )]),
                ))
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
                    registry.register_template_string("bar", from_utf8(BAR_HBS).unwrap())?;
                }
                let js_object = format!(
                    "new Bar('{}', '{}', '{}', '{}', {:?}, {:?}, {})",
                    id, name, x_label, y_label, labels, values, log_toggle
                );
                let data = HashMap::from([
                    ("id".to_string(), to_json(&id)),
                    ("log_toggle".to_string(), to_json(log_toggle)),
                ]);
                Ok((
                    registry.render("bar", &data)?,
                    HashMap::from([(
                        "datasets".to_string(),
                        HashMap::from([(id.clone(), js_object)]),
                    )]),
                ))
            }
            Self::MultiBar {
                id,
                names,
                x_label,
                y_label,
                labels,
                values,
                log_toggle,
            } => {
                if !registry.has_template("bar") {
                    registry.register_template_string("bar", from_utf8(BAR_HBS).unwrap())?;
                }
                let js_object = format!(
                    "new MultiBar('{}', {:?}, '{}', '{}', {:?}, {:?}, {})",
                    id, names, x_label, y_label, labels, values, log_toggle
                );
                let data = HashMap::from([
                    ("id".to_string(), to_json(&id)),
                    ("log_toggle".to_string(), to_json(log_toggle)),
                ]);
                Ok((
                    registry.render("bar", &data)?,
                    HashMap::from([(
                        "datasets".to_string(),
                        HashMap::from([(id.clone(), js_object)]),
                    )]),
                ))
            }
            Self::Hexbin {
                id,
                bins,
                min,
                max,
                radius,
            } => {
                if !registry.has_template("hexbin") {
                    registry.register_template_file("hexbin", "./hbs/hexbin.hbs")?;
                }
                let mut js_object = format!(
                    "new Hexbin('{}', [{}, {}], [{}, {}], {}, [",
                    id, min.0, min.1, max.0, max.1, radius
                );
                for bin in bins {
                    js_object.push_str(&format!(
                        "{{ x: {}, y: {}, length: {} }}, ",
                        bin.x, bin.y, bin.length
                    ));
                }
                js_object.push_str("])");
                let data = HashMap::from([("id".to_string(), to_json(&id))]);
                Ok((
                    registry.render("hexbin", &data)?,
                    HashMap::from([(
                        "datasets".to_string(),
                        HashMap::from([(id.clone(), js_object)]),
                    )]),
                ))
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct Bin {
    pub length: f64,
    pub x: f64,
    pub y: f64,
    pub real_x: f64,
    pub real_y: f64,
}

impl fmt::Display for Bin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{{x: {}, y: {}, length: {:?} }}",
            self.x, self.y, self.length
        )
    }
}

struct CounterBin {
    pub length: u64,
    pub x: f64,
    pub y: f64,
    pub real_x: f64,
    pub real_y: f64,
}

impl Bin {
    pub fn hexbin(points: &Vec<(u32, f64)>, _radius: f64) -> Vec<Self> {
        let x_domain = match points.iter().map(|el| el.0).minmax() {
            itertools::MinMaxResult::MinMax(a, b) => (a as f64, b as f64),
            _ => panic!("Need at least two values for hexbining"),
        };
        let y_domain = match points.iter().map(|el| el.1).minmax() {
            itertools::MinMaxResult::MinMax(a, b) => (a as f64, b as f64),
            _ => panic!("Need at least two values for hexbining"),
        };
        let mut bins_by_id: HashMap<String, CounterBin> = HashMap::new();
        let radius = 20.0;
        let (x_range, x_offset) = (928.0 - 20.0 - 82.0, 82.0);
        let (y_range, y_offset) = (928.0 - 20.0 - 50.0, 928.0 - 50.0);
        for point in points {
            let px = (point.0 as f64 - x_domain.0) / (x_domain.1 - x_domain.0) * x_range + x_offset;
            let px = px.round();

            let py =
                -(point.1 as f64 - y_domain.0) / (y_domain.1 - y_domain.0) * y_range + y_offset;
            let py = py.round();

            let dx = 2.0 * (f64::consts::PI / 3.0).sin() * radius;
            let dy = 1.5 * radius;

            let py = py / dy;
            let mut pj = py.round() as i64;
            let px = px / dx - (pj % 2) as f64 / 2.0;
            let mut pi = px.round();
            let py1 = py - pj as f64;

            if py1.abs() * 3.0 > 1.0 {
                let px1 = px - pi;
                let pi2 = pi + (if px < py { -1.0 } else { 1.0 }) / 2.0;
                let pj2 = pj as f64 + (if py < pj as f64 { -1.0 } else { 1.0 });
                let px2 = px - pi2;
                let py2 = py - pj2;
                if px1.powi(2) + py1.powi(2) > px2.powi(2) + py2.powi(2) {
                    pi = pi2 + (if (pj % 2) == 1 { 1.0 } else { -1.0 }) / 2.0;
                    pj = pj2 as i64;
                }
            }

            let id = format!("{}-{}", pi, pj);
            if let Some(bin) = bins_by_id.get_mut(&id) {
                bin.length += 1;
            } else {
                let x = (pi + (pj % 2) as f64 / 2.0) * dx;
                let y = pj as f64 * dy;
                let real_x = (x - x_offset) / x_range * (x_domain.1 - x_domain.0) + x_domain.0;
                let real_y = -(y - y_offset) / y_range * (y_domain.1 - y_domain.0) + y_domain.0;
                bins_by_id.insert(
                    id,
                    CounterBin {
                        length: 1,
                        x,
                        y,
                        real_x,
                        real_y,
                    },
                );
            }
        }
        bins_by_id
            .into_values()
            .map(|el| Self {
                length: (el.length as f64).log10(),
                x: el.x,
                y: el.y,
                real_x: el.real_x,
                real_y: el.real_y,
            })
            .collect()
    }
}
