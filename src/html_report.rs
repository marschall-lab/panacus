use base64::prelude::*;
use base64::{engine::general_purpose::STANDARD, Engine as _};
use std::ffi::OsStr;
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;
use std::{collections::HashMap, str::from_utf8};
use std::{f64, fmt};

use base64::{engine::general_purpose, Engine};
use handlebars::{to_json, Handlebars, RenderError};

use itertools::Itertools;
use serde::{Deserialize, Serialize};
use time::{macros::format_description, OffsetDateTime};

use crate::graph_broker::{GraphBroker, ItemId};
use crate::util::to_id;

type JsVars = HashMap<String, HashMap<String, String>>;
type RenderedHTML = Result<(String, JsVars), RenderError>;

pub const BOOTSTRAP_COLOR_MODES_JS: &[u8] = include_bytes!("../etc/color-modes.min.js");
pub const BOOTSTRAP_CSS: &[u8] = include_bytes!("../etc/bootstrap.min.css");
pub const BOOTSTRAP_JS: &[u8] = include_bytes!("../etc/bootstrap.bundle.min.js");
pub const CUSTOM_CSS: &[u8] = include_bytes!("../etc/custom.css");
pub const CUSTOM_LIB_JS: &[u8] = include_bytes!("../etc/lib.js");
pub const HOOK_AFTER_JS: &[u8] = include_bytes!("../etc/hook_after.js");
pub const PANACUS_LOGO: &[u8] = include_bytes!("../etc/panacus-illustration-small.png");
pub const SYMBOLS_SVG: &[u8] = include_bytes!("../etc/symbols.svg");
pub const VEGA: &[u8] = include_bytes!("../etc/vega@6.0.0.min.js");
pub const VEGA_EMBED: &[u8] = include_bytes!("../etc/vega-embed@6.29.0.min.js");
pub const VEGA_LITE: &[u8] = include_bytes!("../etc/vega-lite@6.1.0.min.js");

pub const REPORT_HBS: &[u8] = include_bytes!("../hbs/report.hbs");
pub const BAR_HBS: &[u8] = include_bytes!("../hbs/bar.hbs");
pub const TREE_HBS: &[u8] = include_bytes!("../hbs/tree.hbs");
pub const TABLE_HBS: &[u8] = include_bytes!("../hbs/table.hbs");
pub const HEATMAP_HBS: &[u8] = include_bytes!("../hbs/heatmap.hbs");
pub const ANALYSIS_TAB_HBS: &[u8] = include_bytes!("../hbs/analysis_tab.hbs");
pub const REPORT_CONTENT_HBS: &[u8] = include_bytes!("../hbs/report_content.hbs");
pub const HEXBIN_HBS: &[u8] = include_bytes!("../hbs/hexbin.hbs");
pub const LINE_HBS: &[u8] = include_bytes!("../hbs/line.hbs");
pub const PNG_HBS: &[u8] = include_bytes!("../hbs/png.hbs");

fn combine_vars(mut a: JsVars, b: JsVars) -> JsVars {
    for (k, v) in b {
        if let Some(x) = a.get_mut(&k) {
            x.extend(v);
        }
    }
    a
}

#[derive(Serialize, Deserialize, Debug)]
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
        let plots = if self.items.len() > 1 {
            self.items
                .iter()
                .map(|item| HashMap::from([("id", item.get_id()), ("name", item.get_name())]))
                .collect()
        } else {
            vec![HashMap::from([
                ("id", self.items[0].get_id()),
                ("name", "".to_string()),
            ])]
        };
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
            ("plot", to_json(plots)),
            ("items", to_json(items)),
        ]);
        Ok((registry.render("analysis_tab", &vars)?, js_objects))
    }

    pub fn generate_custom_section(
        gb: &GraphBroker,
        name: String,
        file: String,
    ) -> anyhow::Result<Vec<Self>> {
        let id = name.to_lowercase().replace(&[' ', '|', '\\'], "-");
        let report_item = match get_extension_from_filename(&file) {
            Some("svg") => ReportItem::Svg {
                id: format!("svg-{id}"),
                file,
            },
            Some("png") => ReportItem::Png {
                id: format!("png-{id}"),
                file,
            },
            Some("json") => ReportItem::Json {
                id: format!("json-{id}"),
                file,
            },
            _ => unimplemented!("Other formats have not been implemented yet"),
        };
        Ok(vec![AnalysisSection {
            id: format!("custom-{id}"),
            analysis: "Custom".to_string(),
            run_name: name,
            countable: "default".to_string(),
            table: None,
            items: vec![report_item],
        }])
    }
}

fn get_extension_from_filename(filename: &str) -> Option<&str> {
    Path::new(filename).extension().and_then(OsStr::to_str)
}

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
        vars.insert("vega", String::from_utf8_lossy(VEGA).into_owned());
        vars.insert(
            "vega_embed",
            String::from_utf8_lossy(VEGA_EMBED).into_owned(),
        );
        vars.insert("vega_lite", String::from_utf8_lossy(VEGA_LITE).into_owned());
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
                eprintln!("{}", s.id);
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

#[derive(Serialize, Deserialize, Debug)]
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
    },
    Heatmap {
        id: String,
        name: String,
        x_labels: Vec<String>,
        y_labels: Vec<String>,
        values: Vec<Vec<f32>>,
    },
    Line {
        id: String,
        name: String,
        x_label: String,
        y_label: String,
        x_values: Vec<f32>,
        y_values: Vec<f32>,
        log_x: bool,
        log_y: bool,
    },
    Png {
        id: String,
        file: String,
    },
    Svg {
        id: String,
        file: String,
    },
    Json {
        id: String,
        file: String,
    },
}

impl ReportItem {
    fn get_id(&self) -> String {
        match self {
            Self::Bar { id, .. } => id.to_string(),
            Self::MultiBar { id, .. } => id.to_string(),
            Self::Table { id, .. } => id.to_string(),
            Self::Heatmap { id, .. } => id.to_string(),
            Self::Hexbin { id, .. } => id.to_string(),
            Self::Line { id, .. } => id.to_string(),
            Self::Png { id, .. } => id.to_string(),
            Self::Svg { id, .. } => id.to_string(),
            Self::Json { id, .. } => id.to_string(),
        }
    }

    fn get_name(&self) -> String {
        match self {
            Self::Bar { name, .. } => name.to_string(),
            Self::MultiBar { .. } => "MultiBar".to_string(),
            Self::Table { .. } => "Table".to_string(),
            Self::Heatmap { name, .. } => name.to_string(),
            Self::Hexbin { .. } => "Hexbin".to_string(),
            Self::Line { name, .. } => name.to_string(),
            Self::Png { .. } => "Png".to_string(),
            Self::Svg { .. } => "Svg".to_string(),
            Self::Json { .. } => "Json".to_string(),
        }
    }

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
                let mut data_set = "{ 'values': [".to_string();
                for (row_i, row) in values.iter().enumerate() {
                    for (col_i, cell) in row.iter().enumerate() {
                        data_set.push_str(&format!(
                            "{{ x: '{}', y: '{}', value: {} }},",
                            x_labels[row_i], y_labels[col_i], cell
                        ));
                    }
                }
                data_set.push_str("]}");
                let js_object = format!("new Heatmap('{}', '{}', {})", id, name, data_set,);
                eprintln!("{}", data_set);
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
                let ordinal = labels.iter().all(|l| l.parse::<f64>().is_ok());
                let data: Vec<String> = labels
                    .into_iter()
                    .zip(values.into_iter())
                    .map(|(l, v)| format!("{{ 'label': '{}', 'value': {} }}", l, v))
                    .collect();
                let mut data_text = "{'values': [".to_string();
                for datum in data {
                    data_text.push_str(&datum);
                    data_text.push_str(", ");
                }
                data_text.push_str("]}");
                let js_object = format!(
                    "new Bar('{}', '{}', '{}', '{}', {}, {}, {})",
                    id, name, x_label, y_label, data_text, log_toggle, ordinal
                );
                eprintln!("{}", js_object);
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
                let data_text = (0..labels.len())
                    .cartesian_product(0..names.len())
                    .map(|(l, n)| {
                        format!(
                            "{{'label': {}, 'name': '{}', 'value': {}}}",
                            labels[l], names[n], values[n][l]
                        )
                    })
                    .join(",");
                let data_text = format!("{{'values': [{}]}}", data_text);
                let js_object = format!(
                    "new MultiBar('{}', '{}', '{}', {}, {})",
                    id, x_label, y_label, log_toggle, data_text
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
            Self::Hexbin { id, bins } => {
                if !registry.has_template("hexbin") {
                    registry.register_template_string("hexbin", from_utf8(HEXBIN_HBS).unwrap())?;
                }
                let mut js_object = format!("new Hexbin('{}', {{'values': [", id,);
                for (_i, bin) in bins.iter().enumerate() {
                    js_object.push_str(&format!(
                        "{{ coverage: {}, length: {}, size: {} }}, ",
                        bin.x, bin.y, bin.size,
                    ));
                }
                js_object.push_str("]}, [");
                for (_i, bin) in bins.into_iter().enumerate() {
                    js_object.push_str(&format!("[",));
                    for node in bin.content {
                        js_object.push_str(&format!("{},", node.0,));
                    }
                    js_object.push_str("],");
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
            Self::Line {
                id,
                name,
                x_values,
                y_values,
                log_x,
                log_y,
                x_label,
                y_label,
            } => {
                if !registry.has_template("line") {
                    registry.register_template_string("line", from_utf8(LINE_HBS).unwrap())?;
                }

                let data: Vec<String> = x_values
                    .into_iter()
                    .zip(y_values.into_iter())
                    .map(|(l, v)| format!("{{ 'x': '{}', 'y': {} }}", l, v))
                    .collect();
                let mut data_text = "{'values': [".to_string();
                for datum in data {
                    data_text.push_str(&datum);
                    data_text.push_str(", ");
                }
                data_text.push_str("]}");
                let js_object = format!(
                    "new Line('{}', '{}', '{}', '{}', {}, {}, {})",
                    id, name, x_label, y_label, log_x, log_y, data_text
                );

                let data = HashMap::from([("id".to_string(), to_json(&id))]);
                Ok((
                    registry.render("line", &data)?,
                    HashMap::from([(
                        "datasets".to_string(),
                        HashMap::from([(id.clone(), js_object)]),
                    )]),
                ))
            }
            Self::Png { id, file } => {
                if !registry.has_template("png") {
                    registry.register_template_string("png", from_utf8(PNG_HBS).unwrap())?;
                }
                let f = File::open(file)?;
                let mut reader = BufReader::new(f);
                let mut buffer = Vec::new();
                reader.read_to_end(&mut buffer)?;
                let base64_text = STANDARD.encode(buffer);
                let data = HashMap::from([("base64", &base64_text)]);
                Ok((
                    registry.render("png", &data)?,
                    HashMap::from([("datasets".to_string(), HashMap::new())]),
                ))
            }
            Self::Svg { id, file } => Ok((String::new(), HashMap::new())),
            Self::Json { id, file } => Ok((String::new(), HashMap::new())),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bin {
    pub size: u64,
    pub x: f64,
    pub y: f64,
    pub content: Vec<ItemId>,
}

impl fmt::Display for Bin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{{x: {}, y: {}, size: {:?} }}",
            self.x, self.y, self.size
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
    pub fn hexbin(points: &Vec<(ItemId, u32, f64)>, nx: u32, ny: u32) -> Vec<Self> {
        let max_coverage = points
            .iter()
            .map(|(_i, c, _l)| *c)
            .max()
            .expect("At least one point");
        let max_length = points.iter().map(|(_i, _c, l)| *l).fold(0. / 0., f64::max);
        let dx = max_coverage as f64 / (nx - 1) as f64;
        let t = dx as f64 / 3f64.sqrt();
        let dy = max_length / (ny - 1) as f64;
        // eprintln!("max c: {}, dx: {}, t: {}, dy: {}", max_coverage, dx, t, dy);
        let mut bins: HashMap<(bool, i64, i64), Self> = HashMap::new();
        for point in points {
            // Calculate positions in both grids
            let mut black_x = (point.1 as f64 / dx).floor() * dx;
            let mut black_y = (point.2 / dy).floor() * dy;
            let mut green_x = ((point.1 as f64 - dx / 2.0) / dx).floor() * dx + dx / 2.0;
            let mut green_y = ((point.2 - dy / 2.0) / dy).floor() * dy + dy / 2.0;

            // eprintln!("point: {:?}, black: {:?}, green: {:?}", point, (black_x, black_y), (green_x, green_y));

            if black_x < green_x {
                black_x += dx;
            } else {
                green_x += dx;
            }

            if black_y < green_y {
                black_y += dy;
            } else {
                green_y += dy;
            }

            // eprintln!("\tpoint: {:?}, black: {:?}, green: {:?}", point, (black_x, black_y), (green_x, green_y));

            if Self::distance(point.1 as f64, point.2, black_x, black_y)
                < Self::distance(point.1 as f64, point.2, green_x, green_y)
            {
                bins.entry((false, (black_x / dx) as i64, (black_y / dy) as i64))
                    .or_insert(Self {
                        x: black_x as f64,
                        y: black_y as f64,
                        size: 0,
                        content: Vec::new(),
                    })
                    .content
                    .push(point.0);
            } else {
                // eprintln!("\t\tGreen one");
                bins.entry((
                    true,
                    ((green_x - dx / 2.0) / dx) as i64,
                    ((green_y - dy / 2.0) / dy) as i64,
                ))
                .or_insert(Self {
                    x: green_x as f64,
                    y: green_y as f64,
                    size: 0,
                    content: Vec::new(),
                })
                .content
                .push(point.0);
            }
        }
        let mut bins: Vec<Bin> = bins.into_values().collect();
        for bin in &mut bins {
            bin.size = bin.content.len() as u64;
        }
        bins
    }

    fn distance(x1: f64, y1: f64, x2: f64, y2: f64) -> f64 {
        (((x1 - x2).powf(2.0) + (y1 - y2).powf(2.0)) as f64).sqrt()
    }
}
