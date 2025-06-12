/* global bootstrap: false */
(() => {
'use strict'
const tooltipTriggerList = Array.from(document.querySelectorAll('[data-bs-toggle="tooltip"]'))
tooltipTriggerList.forEach(tooltipTriggerEl => {
new bootstrap.Tooltip(tooltipTriggerEl)
})
})()

const pluginCanvasBackgroundColor = {
  id: 'customCanvasBackgroundColor',
  beforeDraw: (chart, args, options) => {
    const {ctx, chartArea: { top, bottom, left, right, width, height },
        scales: {x, y}
    } = chart;
    ctx.save();
    ctx.globalCompositeOperation = 'destination-over';
    ctx.fillStyle = options.color || '#99ffff';
    ctx.fillRect(left, top, width, height);
    ctx.restore();
  }
}

// Adapted from https://github.com/vega/vega-embed/
function post_to_vega_editor(window, data) {
    const url = 'https://vega.github.io/editor/';
    const editor = window.open(url);
    const wait = 10000;
    const step = 250;
    const {origin} = new URL(url);

    let count = ~~(wait / step);

    function listen(evt) {
        if (evt.source === editor) {
            count = 0;
            window.removeEventListener('message', listen, false);
        }
    }
    window.addEventListener('message', listen, false);

    function send() {
        if (count <= 0) {
            return;
        }
        editor.postMessage(data, origin);
        setTimeout(send, step);
        count -= 1;
    }
    setTimeout(send, step);
}

for (let key in objects.datasets) {
    let element = objects.datasets[key];
    if (element instanceof Bar) {
        let h = element;
        let ctx = document.getElementById('chart-bar-' + h.id);
        let id = 'chart-bar-' + h.id;
        let data = {};
        if (h.ordinal) {
            data.values = h.data.values.map(d => ({
                ...d,
                label: Number(d.label)
            }));
        } else {
            data.values = h.data.values;
        }
        if (h.log_toggle) {
            data.values = data.values.filter((el) => el.value > 0);
        }
        let yourVlSpec = {
            $schema: 'https://vega.github.io/schema/vega-lite/v6.json',
            description: 'Bar',
            width: 1000,
            "autosize": {
                "type": "fit",
                "contains": "padding"
            },
            height: 350,
            // data: h.data,
            data,
            layer: [
                {
                    "params": [
                        {
                            "name": "hover",
                            "select": {"type": "point", "on": "pointerover", "clear": "pointerout"}
                        }
                    ],
                    "mark": {"type": "bar", "color": "#eee", "tooltip": true},
                    "encoding": {
                        x: {field: 'label', type: 'nominal', "axis": {"labelAngle": 65}, title: h.x_label},
                        "opacity": {
                            "condition": {"test": {"param": "hover", "empty": false}, "value": 0.5},
                            "value": 0
                        },
                        "detail": [{field: 'value', type: 'quantitative', title: h.y_label}]
                    }
                },
                {
                    mark: 'bar',
                    encoding: {
                        x: {field: 'label', type: h.ordinal ? 'ordinal' : 'nominal', "axis": {"labelAngle": 65}, title: h.x_label},
                        y: {field: 'value', title: h.y_label},
                    },
                },
            ]
        };

        function render(scaleType, thisId, vlSpec, add_listeners) {
            const copied_spec = JSON.parse(JSON.stringify(vlSpec)); // deep copy
            if (scaleType == "log") {
                copied_spec.layer[1].encoding.y.scale = { type: "log", domainMin: 1 }; // set scale type
                copied_spec.layer[1].encoding.y2 = { datum: 1 }; // set scale type
            } else {
                copied_spec.layer[1].encoding.y.scale = { type: "linear" }; // set scale type
                if ('y2' in copied_spec.layer[1].encoding) {
                    delete copied_spec.layer[1].encoding[y2]; // set scale type
                }
            }
            let opt = {
                "actions": false,
            };
            vegaEmbed(`#${CSS.escape(thisId)}`, copied_spec, opt).then(({ view, spec, vgSpec }) => {
                if (add_listeners) {
                    // Export PNG
                    let png_button = document.getElementById('btn-download-plot-png-' + h.id);
                    png_button.addEventListener('click', () => {
                        view.toImageURL('png').then(url => {
                            const a = document.createElement('a');
                            a.href = url;
                            a.download = 'visualization.png';
                            a.click();
                        });
                    });

                    // Export SVG
                    let svg_button = document.getElementById('btn-download-plot-svg-' + h.id);
                    svg_button.removeEventListener('click', svg_button);
                    svg_button.addEventListener('click', function svg_button() {
                        view.toImageURL('svg').then(url => {
                            const a = document.createElement('a');
                            a.href = url;
                            a.download = 'visualization.svg';
                            a.click();
                        });
                    });

                    // Open in Vega Editor
                    let vega_editor_button = document.getElementById('btn-download-plot-vega-editor-' + h.id);
                    vega_editor_button.addEventListener('click', () => {
                        post_to_vega_editor(window, {
                            mode: 'vega-lite',
                            spec: JSON.stringify(spec, null, 2),
                            renderer: undefined,
                            config: undefined,
                        });
                    });
                }
            });
        }

        if (h.log_toggle) {
            document.getElementById('btn-logscale-plot-bar-' + h.id).addEventListener('change', (event) => {
                if (event.currentTarget.checked) {
                    render("log", id, yourVlSpec, false);
                } else {
                    render("linear", id, yourVlSpec, false);
                }
            });
        }

        if (document.getElementById('btn-logscale-plot-bar-' + h.id).checked) {
            render("log", id, yourVlSpec, true);
        } else {
            render("linear", id, yourVlSpec, true);
        }
    } else if (element instanceof MultiBar) {
        let m = element;
        var ctx = document.getElementById('chart-bar-' + m.id);
        let id = 'chart-bar-' + m.id;
        let yourVlSpec = {
            $schema: 'https://vega.github.io/schema/vega-lite/v6.json',
            description: 'MultiBar',
            width: 1000,
            "autosize": {
                "type": "fit",
                "contains": "padding"
            },
            height: 350,
            data: m.data,
            layer: [
                {
                    mark: {"type": 'bar', "tooltip": {"content": "data"}},
                    encoding: {
                        x: {field: 'label', type: 'ordinal', title: m.x_label, sort: null},
                        "y": {
                            "aggregate": "sum", "field": "value",
                            "title": m.y_label,
                            "stack": null
                        },
                        "color": {
                            "field": "name",
                            "type": "nominal",
                            "scale": {
                                "range": ['#f77189', '#bb9832', '#50b131', '#36ada4', '#3ba3ec', '#e866f4']
                            }
                        },
                    },
                },
            ]
        };

        function render(scaleType, thisId, vlSpec, add_listeners) {
            const copied_spec = JSON.parse(JSON.stringify(vlSpec)); // deep copy
            if (scaleType == "log") {
                copied_spec.layer[0].encoding.y.scale = { type: "log", domainMin: 1 }; // set scale type
                copied_spec.layer[0].encoding.y2 = { datum: 1 }; // set scale type
            } else {
                copied_spec.layer[0].encoding.y.scale = { type: "linear" }; // set scale type
                if ('y2' in copied_spec.layer[0].encoding) {
                    delete copied_spec.layer[0].encoding[y2]; // set scale type
                }
            }
            let opt = {
                "actions": false,
            };
            vegaEmbed(`#${CSS.escape(thisId)}`, copied_spec, opt).then(({ view, spec, vgSpec }) => {
                if (add_listeners) {
                    // Export PNG
                    let png_button = document.getElementById('btn-download-plot-png-' + m.id);
                    png_button.addEventListener('click', () => {
                        view.toImageURL('png').then(url => {
                            const a = document.createElement('a');
                            a.href = url;
                            a.download = 'visualization.png';
                            a.click();
                        });
                    });

                    // Export SVG
                    let svg_button = document.getElementById('btn-download-plot-svg-' + m.id);
                    svg_button.removeEventListener('click', svg_button);
                    svg_button.addEventListener('click', function svg_button() {
                        view.toImageURL('svg').then(url => {
                            const a = document.createElement('a');
                            a.href = url;
                            a.download = 'visualization.svg';
                            a.click();
                        });
                    });

                    // Open in Vega Editor
                    let vega_editor_button = document.getElementById('btn-download-plot-vega-editor-' + m.id);
                    vega_editor_button.addEventListener('click', () => {
                        post_to_vega_editor(window, {
                            mode: 'vega-lite',
                            spec: JSON.stringify(spec, null, 2),
                            renderer: undefined,
                            config: undefined,
                        });
                    });
                }
            });
        }

        render("linear", id, yourVlSpec, true);
    } else if (element instanceof Line) {
        let l = element;
        let thisId = 'chart-line-' + l.id;
        let mySpec = {
            "$schema": "https://vega.github.io/schema/vega-lite/v6.json",
            "description": "Line",
            "data": l.data,
            "width": 1000,
            "height": 400,
            "layer": [
                {
                    "mark": {
                        "type": "line",
                        "point": {
                            "filled": false,
                            "fill": "white"
                        },
                        "tooltip": true,
                    },
                    "encoding": {
                        "x": {"field": "x", "type": "quantitative", "title": l.x_label},
                        "y": {"field": "y", "type": "quantitative", "title": l.y_label},
                    }
                }
            ]
        };

        if (l.log_x) {
            mySpec.layer[0].encoding.x.scale = { type: "log", nice: false }; // set scale type
            // mySpec.layer[1].encoding.x.scale = { type: "log", nice: false }; // set scale type
            if (!("transform" in mySpec)) {
                mySpec.transform = [];
            }
            mySpec.transform.push({"filter": "datum.x > 0"});
        }
        if (l.log_y) {
            mySpec.layer[0].encoding.y.scale = { type: "log", nice: false }; // set scale type
            // mySpec.layer[1].encoding.y.scale = { type: "log", nice: false }; // set scale type
            if (!("transform" in mySpec)) {
                mySpec.transform = [];
            }
            mySpec.transform.push({"filter": "datum.y > 0"});
        }
        let opt = {
            "actions": false,
        };
        vegaEmbed(`#${CSS.escape(thisId)}`, mySpec, opt).then(({ view, spec, vgSpec }) => {
            // Export PNG
            let png_button = document.getElementById('btn-download-plot-png-' + l.id);
            png_button.addEventListener('click', () => {
                view.toImageURL('png').then(url => {
                    const a = document.createElement('a');
                    a.href = url;
                    a.download = 'visualization.png';
                    a.click();
                });
            });

            // Export SVG
            let svg_button = document.getElementById('btn-download-plot-svg-' + l.id);
            svg_button.removeEventListener('click', svg_button);
            svg_button.addEventListener('click', function svg_button() {
                view.toImageURL('svg').then(url => {
                    const a = document.createElement('a');
                    a.href = url;
                    a.download = 'visualization.svg';
                    a.click();
                });
            });

            // Open in Vega Editor
            let vega_editor_button = document.getElementById('btn-download-plot-vega-editor-' + l.id);
            vega_editor_button.addEventListener('click', () => {
                post_to_vega_editor(window, {
                    mode: 'vega-lite',
                    spec: JSON.stringify(spec, null, 2),
                    renderer: undefined,
                    config: undefined,
                });
            });
        });
    } else if (element instanceof Hexbin) {
        let h = element;
        let thisId = 'chart-hexbin-' + h.id;
        // buildPlotDownload(myChart, h.id, fname);
        let mySpec = {
            "$schema": "https://vega.github.io/schema/vega-lite/v6.json",
            "description": "Hexbin",
            "data": h.bins,
            "width": 795,
            "height": 805,
            "params": [{"name": "highlight", "select": "point"}],
            "mark": {
                "type": "text",
                "text": "⬢",
                "size": 81,
                "clip": true,
                "tooltip": true,
            },
            "encoding": {
                "y": {
                    "field": "length",
                    "title": "log10 length in bp",
                    "type": "quantitative",
                    "scale": {"nice": false, "zero": false },
                },
                "x": {
                    "field": "coverage",
                    "type": "quantitative",
                    "scale": {"nice": false, "zero": false },
                },
                "color": {
                    "field": "size",
                    "type": "quantitative",
                    "scale": {"type": "log", "scheme": "bluepurple"}
                },
                "stroke": {
                    "condition": {
                        "param": "highlight",
                        "empty": false,
                        "value": "black"
                    },
                    "value": null
                },
                "opacity": {
                    "condition": {"param": "highlight", "value": 1},
                    "value": 0.5
                },
                "order": {"condition": {"param": "highlight", "value": 1}, "value": 0}
            }
        };

        let opt = {
            "actions": false,
        };
        vegaEmbed(`#${CSS.escape(thisId)}`, mySpec, opt).then(({ view, spec, vgSpec }) => {
            let list_button = document.getElementById('btn-download-node-list-' + h.id);
            list_button.addEventListener('click', () => {
                let ids = new Array();
                if ("vlPoint" in view.signal('highlight')) {
                    ids = view.signal('highlight').vlPoint.or.map((x) => x._vgsid_);
                }
                let table = "";
                ids.forEach((id) => {
                    h.bin_content[id - 1].forEach((dataPoint) => {
                        table += dataPoint + "\t" + id + "\n";
                    });
                });
                let blob = new Blob([table], {type: 'text/plain'});
                let a = document.createElement('a');
                a.href = URL.createObjectURL(blob);
                a.download = 'hexbin_nodes_table.tsv';
                a.click();
            });

            view.addSignalListener('highlight', (name, value) => {
                if ("vlPoint" in value) {
                    list_button.removeAttribute('disabled');
                } else {
                    list_button.setAttribute('disabled', '');
                }
            });
            // Export PNG
            let png_button = document.getElementById('btn-download-plot-png-' + h.id);
            png_button.addEventListener('click', () => {
                view.toImageURL('png').then(url => {
                    const a = document.createElement('a');
                    a.href = url;
                    a.download = 'visualization.png';
                    a.click();
                });
            });

            // Export SVG
            let svg_button = document.getElementById('btn-download-plot-svg-' + h.id);
            svg_button.removeEventListener('click', svg_button);
            svg_button.addEventListener('click', function svg_button() {
                view.toImageURL('svg').then(url => {
                    const a = document.createElement('a');
                    a.href = url;
                    a.download = 'visualization.svg';
                    a.click();
                });
            });

            // Open in Vega Editor
            let vega_editor_button = document.getElementById('btn-download-plot-vega-editor-' + h.id);
            vega_editor_button.addEventListener('click', () => {
                post_to_vega_editor(window, {
                    mode: 'vega-lite',
                    spec: JSON.stringify(spec, null, 2),
                    renderer: undefined,
                    config: undefined,
                });
            });
        });

    } else if (element instanceof Heatmap) {
        let h = element;
        let thisId = 'chart-heatmap-' + h.id;
        // buildPlotDownload(myChart, h.id, fname);
        let mySpec = {
            "$schema": "https://vega.github.io/schema/vega-lite/v6.json",
            "description": "Heatmap",
            "data": h.data_set,
            "width": 800,
            "height": 800,
            "mark": {
                "type": "rect",
                "tooltip": true,
            },
            "encoding": {
                "y": {
                    "field": "y",
                    "type": "ordinal",
                    "sort": null,
                },
                "x": {
                    "field": "x",
                    "type": "ordinal",
                    "sort": null,
                },
                "color": {
                    "field": "value",
                    "type": "quantitative",
                    "scale": {"range": ["darkred", "white"], "interpolate": "cubehelix", "domainMax": 1.0}
                },
            }
        };

        let opt = {
            "actions": false,
        };
        vegaEmbed(`#${CSS.escape(thisId)}`, mySpec, opt).then(({ view, spec, vgSpec }) => {
            // Export PNG
            let png_button = document.getElementById('btn-download-plot-png-' + h.id);
            png_button.addEventListener('click', () => {
                view.toImageURL('png').then(url => {
                    const a = document.createElement('a');
                    a.href = url;
                    a.download = 'visualization.png';
                    a.click();
                });
            });

            // Export SVG
            let svg_button = document.getElementById('btn-download-plot-svg-' + h.id);
            svg_button.removeEventListener('click', svg_button);
            svg_button.addEventListener('click', function svg_button() {
                view.toImageURL('svg').then(url => {
                    const a = document.createElement('a');
                    a.href = url;
                    a.download = 'visualization.svg';
                    a.click();
                });
            });

            // Open in Vega Editor
            let vega_editor_button = document.getElementById('btn-download-plot-vega-editor-' + h.id);
            vega_editor_button.addEventListener('click', () => {
                post_to_vega_editor(window, {
                    mode: 'vega-lite',
                    spec: JSON.stringify(spec, null, 2),
                    renderer: undefined,
                    config: undefined,
                });
            });
        });
    } else if (element instanceof VegaPlot) {
        let v = element;
        let thisId = 'chart-line-' + v.id;
        let opt = {
            "actions": false,
        };
        vegaEmbed(`#${CSS.escape(thisId)}`, v.jsonContent, opt).then(({ view, spec, vgSpec }) => {
            // Export PNG
            let png_button = document.getElementById('btn-download-plot-png-' + v.id);
            png_button.addEventListener('click', () => {
                view.toImageURL('png').then(url => {
                    const a = document.createElement('a');
                    a.href = url;
                    a.download = 'visualization.png';
                    a.click();
                });
            });

            // Export SVG
            let svg_button = document.getElementById('btn-download-plot-svg-' + v.id);
            svg_button.removeEventListener('click', svg_button);
            svg_button.addEventListener('click', function svg_button() {
                view.toImageURL('svg').then(url => {
                    const a = document.createElement('a');
                    a.href = url;
                    a.download = 'visualization.svg';
                    a.click();
                });
            });

            // Open in Vega Editor
            let vega_editor_button = document.getElementById('btn-download-plot-vega-editor-' + v.id);
            vega_editor_button.addEventListener('click', () => {
                post_to_vega_editor(window, {
                    mode: 'vega-lite',
                    spec: JSON.stringify(spec, null, 2),
                    renderer: undefined,
                    config: undefined,
                });
            });
        });
    } else if (element instanceof DownloadHelper) {
        let d = element;
        document.getElementById('btn-download-plot-' + d.id).addEventListener('click', () => {
            if (d.type == "png") {
                const a = document.createElement('a');
                let png_img = document.getElementById(d.id).getAttribute('src');
                a.href = png_img;
                a.download = 'visualization.png';
                a.click();
            } else if (d.type == "svg") {
                let svgData = document.getElementById(d.id).innerHTML;
                let svgBlob = new Blob([svgData], {type:"image/svg+xml;charset=utf-8"});
                let svgUrl = URL.createObjectURL(svgBlob);
                const a = document.createElement("a");
                a.href = svgUrl;
                a.download = 'visualization.svg';
                a.click();
            }
        });
    }
}

for (let key in objects.tables) {
    let table = objects.tables[key];
    buildTableDownload(table, key, key + '_' + fname);
}
