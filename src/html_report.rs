use std::collections::HashMap;

use base64::{engine::general_purpose, Engine};
use handlebars::{to_json, Handlebars, RenderError};

use time::{macros::format_description, OffsetDateTime};

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
    pub name: String,
    pub id: String,
    pub is_first: bool,
    pub tabs: Vec<AnalysisTab>,
    pub table: Option<String>,
}

impl AnalysisSection {
    pub fn set_first(mut self) -> Self {
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
    fn into_html(self, registry: &mut Handlebars) -> RenderedHTML {
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
            ("id", to_json(&self.id)),
        ]);
        let result = registry.render("analysis_section", &vars)?;
        let mut js_objects = js_objects
            .into_iter()
            .reduce(combine_vars)
            .expect("Report needs to have at least one item");
        js_objects.insert(
            "tables".to_string(),
            match self.table {
                None => HashMap::new(),
                Some(table_content) => HashMap::from([(self.id, table_content)]),
            },
        );
        Ok((result, js_objects))
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
            registry.register_template_file("report", "./hbs/report.hbs")?;
        }
        let (content, js_objects) = Self::generate_report_content(sections, registry)?;
        let mut vars = HashMap::from([
            ("content", content),
            ("data_hook", get_js_objects_string(js_objects)),
            ("fname", filename.to_string()),
        ]);
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
        vars.insert(
            "version",
            option_env!("GIT_HASH")
                .unwrap_or(env!("CARGO_PKG_VERSION"))
                .to_string(),
        );

        let now = OffsetDateTime::now_utc();
        vars.insert(
            "timestamp",
            now.format(&format_description!(
                "[year]-[month]-[day]T[hour]:[minute]:[second]Z"
            ))
            .unwrap(),
        );
        registry.render("report", &vars)
    }

    fn generate_report_content(sections: Vec<Self>, registry: &mut Handlebars) -> RenderedHTML {
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
        let js_objects = js_objects
            .into_iter()
            .reduce(combine_vars)
            .expect("Report needs to contain at least one item");
        Ok((text, js_objects))
    }
}

pub struct AnalysisTab {
    pub name: String,
    pub id: String,
    pub is_first: bool,
    pub items: Vec<ReportItem>,
}

impl AnalysisTab {
    fn into_html(self, registry: &mut Handlebars) -> RenderedHTML {
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
            .reduce(combine_vars)
            .expect("Tab has at least one item");
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
}

impl ReportItem {
    fn into_html(self, registry: &mut Handlebars) -> RenderedHTML {
        match self {
            Self::Table { id, header, values } => {
                if !registry.has_template("table") {
                    registry.register_template_file("table", "./hbs/table.hbs")?;
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
                    registry.register_template_file("bar", "./hbs/bar.hbs")?;
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
        }
    }
}
