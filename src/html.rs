/* standard use */
use std::collections::HashMap;
use std::io::{BufWriter, Write};

/* external use */
use base64::{engine::general_purpose, Engine as _};
use handlebars::Handlebars;
use thousands::Separable;
use time::{macros::format_description, OffsetDateTime};

use crate::graph::Info;
/* internal use */
use crate::hist::*;
use crate::util::*;

pub const BOOTSTRAP_COLOR_MODES_JS: &[u8] = include_bytes!("../etc/color-modes.min.js");
pub const BOOTSTRAP_CSS: &[u8] = include_bytes!("../etc/bootstrap.min.css");
pub const BOOTSTRAP_JS: &[u8] = include_bytes!("../etc/bootstrap.bundle.min.js");
pub const CHART_JS: &[u8] = include_bytes!("../etc/chart.js");
pub const CUSTOM_CSS: &[u8] = include_bytes!("../etc/custom.css");
pub const CUSTOM_LIB_JS: &[u8] = include_bytes!("../etc/lib.min.js");
pub const HOOK_AFTER_JS: &[u8] = include_bytes!("../etc/hook_after.min.js");
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

pub fn generate_hist_tabs(hists: &[Hist]) -> String {
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

        tab_content.push_str(&reg.render_template(tab, &vars).unwrap());
        tab_navigation.push_str(&reg.render_template(nav, &vars).unwrap());
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

    reg.render_template(container, &vars).unwrap()
}

pub fn generate_growth_tabs(growths: &[(CountType, Vec<Vec<f64>>)]) -> String {
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

        tab_content.push_str(&reg.render_template(tab, &vars).unwrap());
        tab_navigation.push_str(&reg.render_template(nav, &vars).unwrap());
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

    reg.render_template(container, &vars).unwrap()
}

pub fn generate_info_tabs(info: Info) -> String {
    let reg = Handlebars::new();

    let mut tab_content = String::new();
    let mut tab_navigation = String::new();
    tab_navigation.push_str(r##"<button class="nav-link active" id="nav-info-1-tab" data-bs-toggle="tab" data-bs-target="#nav-info-1" type="button" role="tab" aria-controls="nav-info-1" aria-selected="true">graph</button>"##);
    tab_navigation.push_str(r##"<button class="nav-link" id="nav-info-2-tab" data-bs-toggle="tab" data-bs-target="#nav-info-2" type="button" role="tab" aria-controls="nav-info-2" aria-selected="false">node</button>"##);
    tab_navigation.push_str(r##"<button class="nav-link" id="nav-info-3-tab" data-bs-toggle="tab" data-bs-target="#nav-info-3" type="button" role="tab" aria-controls="nav-info-3" aria-selected="false">path</button>"##);
    tab_navigation.push_str(r##"<button class="nav-link" id="nav-info-4-tab" data-bs-toggle="tab" data-bs-target="#nav-info-4" type="button" role="tab" aria-controls="nav-info-4" aria-selected="false">groups</button>"##);

    let graph_info = r##"<div class="tab-pane fade{{#if is_first}} show active{{else}} d-none{{/if}}" id="nav-info-1" role="tabpanel" aria-labelledby="nav-info-1">
        <br/>
<table class="table table-striped table-hover">
  <thead>
    <tr>
      <th scope="col">category</th>
      <th scope="col">countable</th>
      <th scope="col">value</th>
    </tr>
  </thead>
  <tbody class="table-group-divider">
    <tr>
      <td>total</td>
      <td>node</td>
      <td>{{{node_count}}}</td>
    </tr>
    <tr>
      <td></td>
      <td>bp</td>
      <td>{{{basepairs}}}</td>
    </tr>
    <tr>
      <td></td>
      <td>edge</td>
      <td>{{{edge_count}}}</td>
    </tr>
    <tr>
      <td></td>
      <td>path</td>
      <td>{{{no_paths}}}</td>
    </tr>
    <tr>
      <td></td>
      <td>group</td>
      <td>{{{no_groups}}}</td>
    </tr>
    <tr>
      <td></td>
      <td>0-degree node</td>
      <td>{{{number_0_degree}}}</td>
    </tr>
    <tr>
      <td></td>
      <td>component</td>
      <td>{{{components}}}</td>
    </tr>
    <tr>
      <td>largest</td>
      <td>component</td>
      <td>{{{largest_component}}}</td>
    </tr>
    <tr>
      <td>smallest</td>
      <td>component</td>
      <td>{{{smallest_component}}}</td>
    </tr>
    <tr>
      <td>median</td>
      <td>component</td>
      <td>{{{median_component}}}</td>
    </tr>
  </tbody>
</table>
<br/>
    <div class="d-flex flex-row-reverse">
        <button id="btn-download-table-info-graph" type="button" class="d-flex align-items-center btn m-1" aria-pressed="false">
            <svg class="bi opacity-50 m-1" width="15" height="15"><use href="#download"></use></svg>
            <svg class="bi opacity-50 m-1" width="15" height="15"><use href="#table"></use></svg>
        </button>
    </div>
</div>
"##;
    let graph_vars = HashMap::from([
        (
            "node_count",
            info.graph_info.node_count.separate_with_commas(),
        ),
        (
            "basepairs",
            info.graph_info.basepairs.separate_with_commas(),
        ),
        (
            "edge_count",
            info.graph_info.edge_count.separate_with_commas(),
        ),
        ("no_paths", info.path_info.no_paths.separate_with_commas()),
        (
            "no_groups",
            info.graph_info.group_count.separate_with_commas(),
        ),
        (
            "components",
            info.graph_info.connected_components.separate_with_commas(),
        ),
        (
            "largest_component",
            info.graph_info.largest_component.separate_with_commas(),
        ),
        (
            "smallest_component",
            info.graph_info.smallest_component.separate_with_commas(),
        ),
        (
            "median_component",
            info.graph_info.median_component.separate_with_commas(),
        ),
        (
            "number_0_degree",
            info.graph_info.number_0_degree.separate_with_commas(),
        ),
        ("is_first", String::from("true")),
    ]);
    tab_content.push_str(&reg.render_template(graph_info, &graph_vars).unwrap());

    let node_info = r##"<div class="tab-pane fade{{#if is_first}} show active{{else}} d-none{{/if}}" id="nav-info-2" role="tabpanel" aria-labelledby="nav-info-2">
    </br>
<table class="table table-striped table-hover">
  <thead>
    <tr>
      <th scope="col">category</th>
      <th scope="col">countable</th>
      <th scope="col">value</th>
    </tr>
  </thead>
  <tbody class="table-group-divider">
    <tr>
      <td>average</td>
      <td>bp</td>
      <td>{{{average_node}}}</td>
    </tr>
    <tr>
      <td></td>
      <td>degree</td>
      <td>{{{average_degree}}}</td>
    </tr>
    <tr>
      <td>longest</td>
      <td>bp</td>
      <td>{{{largest_node}}}</td>
    </tr>
    <tr>
      <td>shortest</td>
      <td>bp</td>
      <td>{{{shortest_node}}}</td>
    </tr>
    <tr>
      <td>median</td>
      <td>bp</td>
      <td>{{{median_node}}}</td>
    </tr>
    <tr>
      <td>N50 node</td>
      <td>bp</td>
      <td>{{{n50_node}}}</td>
    </tr>
    <tr>
      <td>max</td>
      <td>degree</td>
      <td>{{{max_degree}}}</td>
    </tr>
    <tr>
      <td>min</td>
      <td>degree</td>
      <td>{{{min_degree}}}</td>
    </tr>
  </tbody>
</table>
<br/>
    <div class="d-flex flex-row-reverse">
        <button id="btn-download-table-info-node" type="button" class="d-flex align-items-center btn m-1" aria-pressed="false">
            <svg class="bi opacity-50 m-1" width="15" height="15"><use href="#download"></use></svg>
            <svg class="bi opacity-50 m-1" width="15" height="15"><use href="#table"></use></svg>
        </button>
    </div>
</div>
"##;
    let node_vars = HashMap::from([
        (
            "average_degree",
            info.graph_info.average_degree.separate_with_commas(),
        ),
        (
            "max_degree",
            info.graph_info.max_degree.separate_with_commas(),
        ),
        (
            "min_degree",
            info.graph_info.min_degree.separate_with_commas(),
        ),
        (
            "largest_node",
            info.graph_info.largest_node.separate_with_commas(),
        ),
        (
            "shortest_node",
            info.graph_info.shortest_node.separate_with_commas(),
        ),
        (
            "average_node",
            info.graph_info.average_node.separate_with_commas(),
        ),
        (
            "median_node",
            info.graph_info.median_node.separate_with_commas(),
        ),
        ("n50_node", info.graph_info.n50_node.separate_with_commas()),
    ]);
    tab_content.push_str(&reg.render_template(node_info, &node_vars).unwrap());

    let path_info = r##"<div class="tab-pane fade{{#if is_first}} show active{{else}} d-none{{/if}}" id="nav-info-3" role="tabpanel" aria-labelledby="nav-info-3">
    </br>
<table class="table table-striped table-hover">
  <thead>
    <tr>
      <th scope="col">category</th>
      <th scope="col">countable</th>
      <th scope="col">value</th>
    </tr>
  </thead>
  <tbody class="table-group-divider">
    <tr>
      <td>average</td>
      <td>bp</td>
      <td>{{{average_path_bp}}}</td>
    </tr>
    <tr>
      <td></td>
      <td>node</td>
      <td>{{{average_path}}}</td>
    </tr>
    <tr>
      <td>longest</td>
      <td>bp</td>
      <td>{{{longest_path_bp}}}</td>
    </tr>
    <tr>
      <td></td>
      <td>node</td>
      <td>{{{longest_path}}}</td>
    </tr>
    <tr>
      <td>shortest</td>
      <td>bp</td>
      <td>{{{shortest_path_bp}}}</td>
    </tr>
    <tr>
      <td></td>
      <td>node</td>
      <td>{{{shortest_path}}}</td>
    </tr>
  </tbody>
</table>
<br/>
    <div class="d-flex flex-row-reverse">
        <button id="btn-download-table-info-path" type="button" class="d-flex align-items-center btn m-1" aria-pressed="false">
            <svg class="bi opacity-50 m-1" width="15" height="15"><use href="#download"></use></svg>
            <svg class="bi opacity-50 m-1" width="15" height="15"><use href="#table"></use></svg>
        </button>
    </div>
</div>
"##;
    let path_vars = HashMap::from([
        (
            "longest_path",
            info.path_info.node_len.longest.separate_with_commas(),
        ),
        (
            "shortest_path",
            info.path_info.node_len.shortest.separate_with_commas(),
        ),
        (
            "average_path",
            info.path_info.node_len.average.separate_with_commas(),
        ),
        (
            "longest_path_bp",
            info.path_info.bp_len.longest.separate_with_commas(),
        ),
        (
            "shortest_path_bp",
            info.path_info.bp_len.shortest.separate_with_commas(),
        ),
        (
            "average_path_bp",
            info.path_info.bp_len.average.separate_with_commas(),
        ),
    ]);
    tab_content.push_str(&reg.render_template(path_info, &path_vars).unwrap());

    let group_info = r##"<div class="tab-pane fade{{#if is_first}} show active{{else}} d-none{{/if}}" id="nav-info-4" role="tabpanel" aria-labelledby="nav-info-4">
    </br>
    <canvas id="chart-groups-node"></canvas>
    <br/>
    <div class="d-flex flex-row-reverse">
        <button id="btn-download-plot-group-node" type="button" class="d-flex align-items-center btn m-1" aria-pressed="false">
            <svg class="bi opacity-50 m-1" width="15" height="15"><use href="#download"></use></svg>
            <svg class="bi opacity-50 m-1" width="15" height="15"><use href="#card-image"></use></svg>
        </button>
    </div>
<br/>
    <canvas id="chart-groups-bp"></canvas>
<br/>
    <div class="d-flex flex-row-reverse">
        <button id="btn-download-table-info-group" type="button" class="d-flex align-items-center btn m-1" aria-pressed="false">
            <svg class="bi opacity-50 m-1" width="15" height="15"><use href="#download"></use></svg>
            <svg class="bi opacity-50 m-1" width="15" height="15"><use href="#table"></use></svg>
        </button>
        <button id="btn-download-plot-group-bp" type="button" class="d-flex align-items-center btn m-1" aria-pressed="false">
            <svg class="bi opacity-50 m-1" width="15" height="15"><use href="#download"></use></svg>
            <svg class="bi opacity-50 m-1" width="15" height="15"><use href="#card-image"></use></svg>
        </button>
    </div>
</div>
"##;
    let group_vars = HashMap::from([(
        "groups",
        match info.group_info {
            Some(group_info) => group_info
                .groups
                .iter()
                .map(|(k, v)| {
                    HashMap::from([
                        ("name", format!("{}", k)),
                        ("node_len", format!("{}", v.0)),
                        ("bp_len", format!("{}", v.1)),
                    ])
                })
                .collect::<Vec<_>>(),
            None => Vec::new(),
        },
    )]);
    tab_content.push_str(&reg.render_template(&group_info, &group_vars).unwrap());

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

    reg.render_template(container, &vars).unwrap()
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
    hists: &[Hist],
    fname: &str,
    info: Option<Info>,
    out: &mut BufWriter<W>,
) -> Result<(), std::io::Error> {
    let mut vars: HashMap<&str, String> = HashMap::default();

    let content = r##"
<div class="d-flex align-items-start">
	<div class="nav flex-column nav-pills me-3" id="v-pills-tab" role="tablist" aria-orientation="vertical">
    	<button class="nav-link text-nowrap active" id="v-pills-hist-tab" data-bs-toggle="pill" data-bs-target="#v-pills-hist" type="button" role="tab" aria-controls="v-pills-hist" aria-selected="true">coverage histogram</button>
        <button class="nav-link text-nowrap" id="v-pills-info-tab" data-bs-toggle="pill" data-bs-target="#v-pills-info" type="button" role="tab" aria-controls="v-pills-info" aria-selected="false">pangenome info</button>
 	</div>
  	<div class="tab-content w-100" id="v-pills-tabContent">
		<div class="tab-pane fade show active" id="v-pills-hist" role="tabpanel" aria-labelledby="v-pills-hist-tab">
{{{hist_content}}}
		</div>
		<div class="tab-pane fade" id="v-pills-info" role="tabpanel" aria-labelledby="v-pills-info-tab">
{{{info_content}}}
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
    js_objects.push_str("const info = `");
    let info_text = match info {
        Some(ref s) => s.to_string(),
        _ => "".to_string(),
    };
    js_objects.push_str(info_text.as_str());
    js_objects.push_str("`;\n");

    if let Some(info_obj) = &info {
        let info_object = get_info_js_object(&info_obj);
        js_objects.push_str(&info_object[..]);
    }

    let reg = Handlebars::new();
    vars.insert("fname", fname.to_string());
    vars.insert("data_hook", js_objects);
    vars.insert(
        "content",
        reg.render_template(
            content,
            &HashMap::from([
                ("hist_content", generate_hist_tabs(hists)),
                ("info_content", generate_info_tabs(info.unwrap())),
            ]),
        )
        .unwrap(),
    );

    populate_constants(&mut vars);
    write_html(&vars, out)
}

fn bin_values(list: &Vec<u32>) -> (Vec<String>, Vec<usize>) {
    if list.is_empty() {
        return (Vec::new(), Vec::new());
    }
    let n_bins = 50;
    let max = *list.iter().max().unwrap();
    let min = *list.iter().min().unwrap();
    let bin_size = ((max - min) as f32 / n_bins as f32).round();
    let bins: Vec<_> = (min..max)
        .step_by(bin_size as usize)
        .zip((min + (bin_size as u32)..max + 1).step_by(bin_size as usize))
        .collect();
    let values = bins
        .iter()
        .map(|(s, e)| list.iter().filter(|a| **a >= *s && **a < *e).count())
        .collect::<Vec<_>>();
    let bin_names = bins
        .iter()
        .map(|(s, e)| format!("{}-{}", s, e))
        .collect::<Vec<_>>();
    (bin_names, values)
}

fn get_info_js_object(info: &Info) -> String {
    let mut js_objects = String::new();

    js_objects.push_str("const groups = [\n");
    let nodes = info
        .group_info
        .as_ref()
        .unwrap()
        .groups
        .values()
        .map(|x| x.0)
        .collect::<Vec<_>>();
    let bps = info
        .group_info
        .as_ref()
        .unwrap()
        .groups
        .values()
        .map(|x| x.1)
        .collect::<Vec<_>>();

    if nodes.len() >= 100 {
        let binned_nodes = bin_values(&nodes);
        let binned_bps = bin_values(&bps);
        js_objects.push_str(&format!(
            "new Group('node', {:?}, {:?}, true)",
            binned_nodes.0, binned_nodes.1,
        ));
        js_objects.push_str(",\n");
        js_objects.push_str(&format!(
            "new Group('bp', {:?}, {:?}, true)",
            binned_bps.0, binned_bps.1,
        ));
    } else {
        let group_names = info
            .group_info
            .as_ref()
            .unwrap()
            .groups
            .keys()
            .collect::<Vec<_>>();
        js_objects.push_str(&format!(
            "new Group('node', {:?}, {:?}, false)",
            group_names, nodes,
        ));
        js_objects.push_str(",\n");
        js_objects.push_str(&format!(
            "new Group('bp', {:?}, {:?}, false)",
            group_names, bps,
        ));
    }
    js_objects.push_str("];\n");
    js_objects
}

pub fn write_info_html<W: Write>(
    fname: &str,
    info: Info,
    out: &mut BufWriter<W>,
) -> Result<(), std::io::Error> {
    let mut vars: HashMap<&str, String> = HashMap::default();

    let content = r##"
<div class="d-flex align-items-start">
	<div class="nav flex-column nav-pills me-3" id="v-pills-tab" role="tablist" aria-orientation="vertical">
        <button class="nav-link text-nowrap active" id="v-pills-info-tab" data-bs-toggle="pill" data-bs-target="#v-pills-info" type="button" role="tab" aria-controls="v-pills-info" aria-selected="true">pangenome info</button>
 	</div>
  	<div class="tab-connologies to provide
instantly aggregated statistical or similarity measures, humans otent w-100" id="v-pills-tabContent">
		<div class="tab-pane fade show active" id="v-pills-info" role="tabpanel" aria-labelledby="v-pills-info-tab">
{{{info_content}}}
		</div>
  </div>
</div>
"##;

    let mut js_objects = String::from("const hists = [");
    js_objects.push_str("];\n\nconst growths = [];\n");
    js_objects.push_str("const fname = '");
    js_objects.push_str(fname);
    js_objects.push_str("';\n");
    js_objects.push_str("const info = `");
    let info_text = info.to_string();
    js_objects.push_str(info_text.as_str());
    js_objects.push_str("`;\n");

    let info_object = get_info_js_object(&info);
    js_objects.push_str(&info_object[..]);

    let reg = Handlebars::new();
    vars.insert("fname", fname.to_string());
    vars.insert("data_hook", js_objects);
    vars.insert(
        "content",
        reg.render_template(
            &content,
            &HashMap::from([("info_content", generate_info_tabs(info))]),
        )
        .unwrap(),
    );

    populate_constants(&mut vars);
    write_html(&vars, out)
}

pub fn write_histgrowth_html<W: Write>(
    hists: &Option<Vec<Hist>>,
    growths: &[(CountType, Vec<Vec<f64>>)],
    hist_aux: &HistAuxilliary,
    fname: &str,
    ordered_names: Option<&Vec<String>>,
    info: Option<Info>,
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
		<div class="tab-pane fade" id="v-pills-info" role="tabpanel" aria-labelledby="v-pills-info-tab">
{{{info_content}}}
		</div>
  </div>
</div>
"##;

    let mut nav = String::new();
    if hists.is_some() {
        nav.push_str(r##"<button class="nav-link text-nowrap active" id="v-pills-hist-tab" data-bs-toggle="pill" data-bs-target="#v-pills-hist" type="button" role="tab" aria-controls="v-pills-hist" aria-selected="true">coverage histogram</button>"##);
    }
    nav.push_str(&format!(r##"<button class="nav-link text-nowrap{}" id="v-pills-growth-tab" data-bs-toggle="pill" data-bs-target="#v-pills-growth" type="button" role="tab" aria-controls="v-pills-growth" aria-selected="true">{}pangenome growth</button>"##, if hists.is_some() { "" } else { " active"}, if ordered_names.is_some() { "ordered " } else {""} ));
    if info.is_some() {
        nav.push_str(r##"<button class="nav-link text-nowrap" id="v-pills-info-tab" data-bs-toggle="pill" data-bs-target="#v-pills-info" type="button" role="tab" aria-controls="v-pills-info" aria-selected="false">pangenome info</button>"##);
    }

    let mut js_objects = String::from("");
    js_objects.push_str("const hists = [\n");
    if let Some(hs) = hists {
        for (i, h) in hs.iter().enumerate() {
            if i > 0 {
                js_objects.push_str(",\n");
            }
            match ordered_names {
                Some(names) => js_objects.push_str(&format!(
                    "new Hist('{}', {:?}, {:?})",
                    h.count, names, h.coverage
                )),
                None => js_objects.push_str(&format!(
                    "new Hist('{}', {:?}, {:?})",
                    h.count,
                    (0..h.coverage.len()).collect::<Vec<usize>>(),
                    h.coverage
                )),
            }
        }
    }
    js_objects.push_str("];\n\n");
    js_objects.push_str("const growths = [\n");

    for (i, (count, columns)) in growths.iter().enumerate() {
        if i > 0 {
            js_objects.push_str(",\n");
        }
        match ordered_names {
            Some(names) => js_objects.push_str(&format!(
                "new Growth('{}', {:?}, [{}], [{}], {:?})",
                count,
                names,
                &hist_aux
                    .coverage
                    .iter()
                    .map(|x| x.get_string())
                    .collect::<Vec<String>>()
                    .join(", "),
                &hist_aux
                    .quorum
                    .iter()
                    .map(|x| x.get_string())
                    .collect::<Vec<String>>()
                    .join(", "),
                &columns
                    .iter()
                    .map(|col| col[1..]
                        .iter()
                        .map(|x| x.floor() as usize)
                        .collect::<Vec<usize>>())
                    .collect::<Vec<Vec<usize>>>()
            )),
            None => js_objects.push_str(&format!(
                "new Growth('{}', {:?}, [{}], [{}], {:?})",
                count,
                (1..columns[0].len()).collect::<Vec<usize>>(),
                &hist_aux
                    .coverage
                    .iter()
                    .map(|x| x.get_string())
                    .collect::<Vec<String>>()
                    .join(", "),
                &hist_aux
                    .quorum
                    .iter()
                    .map(|x| x.get_string())
                    .collect::<Vec<String>>()
                    .join(", "),
                &columns
                    .iter()
                    .map(|col| col[1..]
                        .iter()
                        .map(|x| x.floor() as usize)
                        .collect::<Vec<usize>>())
                    .collect::<Vec<Vec<usize>>>()
            )),
        }
    }
    js_objects.push_str("];\n\nconst fname = '");
    js_objects.push_str(fname);
    js_objects.push_str("';\n");
    js_objects.push_str("const info = `");
    let info_text = match info {
        Some(ref s) => s.to_string(),
        _ => "".to_string(),
    };
    js_objects.push_str(info_text.as_str());
    js_objects.push_str("`;\n");

    if let Some(info_obj) = &info {
        let info_object = get_info_js_object(&info_obj);
        js_objects.push_str(&info_object[..]);
    }

    let reg = Handlebars::new();
    let mut prevars = HashMap::from([
        ("nav", nav),
        ("growth_content", generate_growth_tabs(growths)),
    ]);
    if let Some(hs) = hists {
        prevars.insert("hist_content", generate_hist_tabs(hs));
    }
    if let Some(st) = info {
        prevars.insert("info_content", generate_info_tabs(st));
    }

    vars.insert("fname", fname.to_string());
    vars.insert("data_hook", js_objects);
    vars.insert("content", reg.render_template(content, &prevars).unwrap());

    populate_constants(&mut vars);
    write_html(&vars, out)
}
