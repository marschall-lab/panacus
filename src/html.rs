/* standard use */
use std::collections::HashMap;
use std::io::{BufWriter, Write};
/* external use */
use base64::{engine::general_purpose, Engine as _};
use handlebars::Handlebars;
use time::{macros::format_description, OffsetDateTime};

/* internal use */
//use crate::abacus::*;
//use crate::graph::*;
use crate::hist::*;
use crate::util::*;

pub const BOOTSTRAP_COLOR_MODES_JS: &[u8] = include_bytes!("../etc/color-modes.js");
pub const BOOTSTRAP_CSS: &[u8] = include_bytes!("../etc/bootstrap.min.css");
pub const BOOTSTRAP_JS: &[u8] = include_bytes!("../etc/bootstrap.bundle.min.js");
pub const CHART_JS: &[u8] = include_bytes!("../etc/chart.js");
pub const CHART_UTILS_JS: &[u8] = include_bytes!("../etc/chart-utils.js");
pub const CUSTOM_CSS: &[u8] = include_bytes!("../etc/custom.css");
pub const CUSTOM_LIB_JS: &[u8] = include_bytes!("../etc/lib.js");
pub const HOOK_AFTER_JS: &[u8] = include_bytes!("../etc/hook_after.js");
pub const HTML_TEMPLATE: &[u8] = include_bytes!("../etc/report_template.html");
pub const PANACUS_LOGO: &[u8] = include_bytes!("../etc/panacus-illustration-small.png");
pub const SYMBOLS_SVG: &[u8] = include_bytes!("../etc/symbols.svg");

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
        "chart_utils_js",
        String::from_utf8_lossy(CHART_UTILS_JS).into_owned(),
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

pub fn generate_hist_tabs(hists: &Vec<Hist>) -> String {
    let reg = Handlebars::new();

    let mut tab_content = String::new();
    let mut tab_navigation = String::new();
    for (i, h) in hists.iter().enumerate() {
        let tab = r##"<div class="tab-pane fade{{#if is_first}} show active{{else}} d-none{{/if}}" id="nav-hist-{{count}}" role="tabpanel" aria-labelledby="nav-hist-{{count}}">
    <div class="d-flex flex-row-reverse">
        <div class="form-check form-switch">
            <input class="form-check-input" type="checkbox" role="switch" id="btn-logscale-plot-hist-{{count}}">
            <label class="form-check-label" for="btn-logscale-plot-hist-{{count}}">log-scale</label>
        </div>
    </div>
    <canvas id="chart-hist-{{count}}"></canvas>
    <div class="d-flex flex-row-reverse">
        <button id="btn-download-table-hist-{{count}}" type="button" class="d-flex align-items-center btn m-1" aria-pressed="false">
            <svg class="bi opacity-50 m-1" width="15" height="15"><use href="#download"></use></svg>
            <svg class="bi opacity-50 m-1" width="15" height="15"><use href="#table"></use></svg>
        </button>
        <button id="btn-download-plot-hist-{{count}}" type="button" class="d-flex align-items-center btn m-1" aria-pressed="false">
            <svg class="bi opacity-50 m-1" width="15" height="15"><use href="#download"></use></svg>
            <svg class="bi opacity-50" width="15" height="15"><use href="#card-image"></use></svg>
        </button>
    </div>
</div>
"##;

        let nav = r##"<button class="nav-link{{#if is_first}} active{{/if}}" id="nav-hist-{{count}}-tab" data-bs-toggle="tab" data-bs-target="#nav-hist-{{count}}" type="button" role="tab" aria-controls="nav-hist-{{count}}" aria-selected="{{is_first}}">{{count}}</button>
"##;

        let mut vars = HashMap::from([("count", format!("{}", h.count))]);
        if i == 0 {
            vars.insert("is_first", String::from("true"));
        }

        tab_content.push_str(&reg.render_template(&tab, &vars).unwrap());
        tab_navigation.push_str(&reg.render_template(&nav, &vars).unwrap());
    }

    let container = r##"<div class="container">
	<nav>
		<div class="nav nav-tabs" id="nav-tab" role="tablist">
			{{{tab_navigation}}}
		</div>
	</nav>
	{{{tab_content}}}
</div>
"##;

    let vars = HashMap::from([
        ("tab_content", tab_content),
        ("tab_navigation", tab_navigation),
    ]);

    reg.render_template(&container, &vars).unwrap()
}

pub fn generate_growth_tabs(growths: &Vec<(CountType, Vec<Vec<f64>>)>) -> String {
    let reg = Handlebars::new();

    let mut tab_content = String::new();
    let mut tab_navigation = String::new();
    for (i, (count, _)) in growths.iter().enumerate() {
        let tab = r##"<div class="tab-pane fade{{#if is_first}} show active{{else}} d-none{{/if}}" id="nav-growth-{{count}}" role="tabpanel" aria-labelledby="nav-growth-{{count}}">
    <div class="d-flex flex-row-reverse">
        <!--this is empty //-->
    </div>
    <canvas id="chart-growth-{{count}}"></canvas>
    <div class="d-flex flex-row-reverse">
        <button id="btn-download-table-growth-{{count}}" type="button" class="d-flex align-items-center btn m-1" aria-pressed="false">
            <svg class="bi opacity-50 m-1" width="15" height="15"><use href="#download"></use></svg>
            <svg class="bi opacity-50 m-1" width="15" height="15"><use href="#table"></use></svg>
        </button>
        <button id="btn-download-plot-growth-{{count}}" type="button" class="d-flex align-items-center btn m-1" aria-pressed="false">
            <svg class="bi opacity-50 m-1" width="15" height="15"><use href="#download"></use></svg>
            <svg class="bi opacity-50 m-1" width="15" height="15"><use href="#card-image"></use></svg>
        </button>
    </div>
</div>
"##;

        let nav = r##"<button class="nav-link{{#if is_first}} active{{/if}}" id="nav-growth-{{count}}-tab" data-bs-toggle="tab" data-bs-target="#nav-growth-{{count}}" type="button" role="tab" aria-controls="nav-growth-{{count}}" aria-selected="{{is_first}}">{{count}}</button>
"##;

        let mut vars = HashMap::from([("count", format!("{}", count))]);
        if i == 0 {
            vars.insert("is_first", String::from("true"));
        }

        tab_content.push_str(&reg.render_template(&tab, &vars).unwrap());
        tab_navigation.push_str(&reg.render_template(&nav, &vars).unwrap());
    }

    let container = r##"<div class="container p-5">
	<nav>
		<div class="nav nav-tabs" id="nav-tab" role="tablist">
			{{{tab_navigation}}}
		</div>
	</nav>
	{{{tab_content}}}
</div>
"##;

    let vars = HashMap::from([
        ("tab_content", tab_content),
        ("tab_navigation", tab_navigation),
    ]);

    reg.render_template(&container, &vars).unwrap()
}

pub fn write_html<W: Write>(
    vars: &HashMap<&str, String>,
    out: &mut BufWriter<W>,
) -> Result<(), std::io::Error> {
    let reg = Handlebars::new();
    let html = String::from_utf8_lossy(HTML_TEMPLATE);
    out.write(reg.render_template(&html, vars).unwrap().as_bytes())
        .map(|_| ())
}

pub fn write_hist_html<W: Write>(
    hists: &Vec<Hist>,
    fname: &str,
    out: &mut BufWriter<W>,
) -> Result<(), std::io::Error> {
    let mut vars: HashMap<&str, String> = HashMap::default();

    let content = r##"
<div class="d-flex align-items-start">
	<div class="nav flex-column nav-pills me-3" id="v-pills-tab" role="tablist" aria-orientation="vertical">
    	<button class="nav-link text-nowrap active" id="v-pills-hist-tab" data-bs-toggle="pill" data-bs-target="#v-pills-hist" type="button" role="tab" aria-controls="v-pills-hist" aria-selected="true">coverage histogram</button>
 	</div>
  	<div class="tab-content w-100" id="v-pills-tabContent">
		<div class="tab-pane fade show active" id="v-pills-hist" role="tabpanel" aria-labelledby="v-pills-hist-tab">
{{{hist_content}}}
		</div>
  </div>
</div>
"##;

    let mut js_objects = String::from("const hists = [\n");
    for (i, h) in hists.iter().enumerate() {
        if i > 0 {
            js_objects.push_str(",\n");
        }
        js_objects.push_str(&format!(
            "new Hist('{}', {:?}, {:?})",
            h.count,
            (0..h.coverage.len()).collect::<Vec<usize>>(),
            h.coverage
        ));
    }
    js_objects.push_str("];\n\nconst growths = [];\n");
    js_objects.push_str("const fname = '");
    js_objects.push_str(fname);
    js_objects.push_str("';\n");

    let reg = Handlebars::new();
    vars.insert("fname", fname.to_string());
    vars.insert("data_hook", js_objects);
    vars.insert(
        "content",
        reg.render_template(
            &content,
            &HashMap::from([("content", generate_hist_tabs(hists))]),
        )
        .unwrap(),
    );

    populate_constants(&mut vars);
    write_html(&vars, out)
}

pub fn write_histgrowth_html<W: Write>(
    hists: &Option<Vec<Hist>>,
    growths: &Vec<(CountType, Vec<Vec<f64>>)>,
    hist_aux: &HistAuxilliary,
    fname: &str,
    out: &mut BufWriter<W>,
) -> Result<(), std::io::Error> {
    let mut vars: HashMap<&str, String> = HashMap::default();

    let content = r##"
<div class="d-flex align-items-start">
	<div class="nav flex-column nav-pills me-3" id="v-pills-tab" role="tablist" aria-orientation="vertical">
{{{nav}}}
 	</div>
  	<div class="tab-content w-100" id="v-pills-tabContent">{{#if hist_content}}
        <div class="tab-pane fade show active" id="v-pills-hist" role="tabpanel" aria-labelledby="v-pills-hist-tab">
{{{hist_content}}}
		</div>{{/if}}
		<div class="tab-pane fade{{#unless hist_content}} show active{{/unless}}" id="v-pills-growth" role="tabpanel" aria-labelledby="v-pills-growth-tab">
{{{growth_content}}}
		</div>
  </div>
</div>
"##;

    let mut nav = String::new();
    if hists.is_some() {
        nav.push_str(r##"<button class="nav-link text-nowrap active" id="v-pills-hist-tab" data-bs-toggle="pill" data-bs-target="#v-pills-hist" type="button" role="tab" aria-controls="v-pills-hist" aria-selected="true">coverage histogram</button>"##);
    }
    nav.push_str(&format!(r##"<button class="nav-link text-nowrap{}" id="v-pills-growth-tab" data-bs-toggle="pill" data-bs-target="#v-pills-growth" type="button" role="tab" aria-controls="v-pills-growth" aria-selected="true">pangenome growth</button>"##, if hists.is_some() { "" } else { " active"}));

    let mut js_objects = String::from("");
    js_objects.push_str("const hists = [\n");
    if let Some(hs) = hists {
        for (i, h) in hs.iter().enumerate() {
            if i > 0 {
                js_objects.push_str(",\n");
            }
            js_objects.push_str(&format!(
                "new Hist('{}', {:?}, {:?})",
                h.count,
                (0..h.coverage.len()).collect::<Vec<usize>>(),
                h.coverage
            ));
        }
    }
    js_objects.push_str("];\n\n");
    js_objects.push_str("const growths = [\n");

    for (i, (count, columns)) in growths.into_iter().enumerate() {
        if i > 0 {
            js_objects.push_str(",\n");
        }
        js_objects.push_str(&format!(
            "new Growth('{}', {:?}, [{}], [{}], {:?})",
            count,
            (1..columns[0].len()).collect::<Vec<usize>>(),
            &hist_aux
                .coverage
                .iter()
                .map(|x| x.to_string())
                .collect::<Vec<String>>()
                .join(", "),
            &hist_aux
                .quorum
                .iter()
                .map(|x| x.to_string())
                .collect::<Vec<String>>()
                .join(", "),
            &columns
                .iter()
                .map(|col| col[1..]
                    .into_iter()
                    .map(|x| x.floor() as usize)
                    .collect::<Vec<usize>>())
                .collect::<Vec<Vec<usize>>>()
        ));
    }
    js_objects.push_str("];\n\nconst fname = '");
    js_objects.push_str(fname);
    js_objects.push_str("';\n");

    let reg = Handlebars::new();
    let mut prevars = HashMap::from([
        ("nav", nav),
        ("growth_content", generate_growth_tabs(growths)),
    ]);
    if let Some(hs) = hists {
        prevars.insert("hist_content", generate_hist_tabs(hs));
    }

    vars.insert("fname", fname.to_string());
    vars.insert("data_hook", js_objects);
    vars.insert("content", reg.render_template(&content, &prevars).unwrap());

    populate_constants(&mut vars);
    write_html(&vars, out)
}
