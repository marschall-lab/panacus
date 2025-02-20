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

for (let key in objects.datasets) {
    let element = objects.datasets[key];
    if (element instanceof Bar) {
        let h = element;
        var ctx = document.getElementById('chart-bar-' + h.id);
        var myChart = new Chart(ctx, {
            type: 'bar',
            data: {
                labels: h.labels,
                datasets: [{
                    label: h.name,
                    data: h.values,
                    borderWidth: 1,
                    backgroundColor: PCOLORS[0],
                    borderColor: '#FFFFFF'
                }]
            },
            options: {
                scales: {
                    y: {
                        title: {
                            display: true,
                            text: h.y_label,
                        },
                        beginAtZero: true,
                        grid: {
                            color: '#FFFFFF',
                        }
                    },
                    x: {
                        title: {
                            display: true,
                            text: h.x_label,
                        },
                        grid: {
                            color: '#FFFFFF',
                        },
                        ticks: {
                            maxRotation: 90,
                            minRotation: 65
                        }
                    },
                },
                plugins: {
                    customCanvasBackgroundColor: {
                        color: '#E5E4EE',
                    }
                }
            },
            plugins: [pluginCanvasBackgroundColor],
        });
        buildPlotDownload(myChart, h.id, fname);
        if (h.log_toggle) {
            buildLogToggle(myChart, "bar-" + h.id);
        }
    } else if (element instanceof MultiBar) {
        let m = element;
        var ctx = document.getElementById('chart-bar-' + m.id);
        var myChart = new Chart(ctx, {
            type: 'bar',
            data: {
                labels: m.labels,
                datasets: Array.from(m.values.entries()).reverse().map(function([i, v]) {
                    return {
                        label: m.names[i],
                        data: v,
                        borderWidth: 1,
                        backgroundColor: PCOLORS[i % PCOLORS.length],
                        borderColor: '#FFFFFF'
                    };
                }),
            },
            options: {
                scales: {
                    y: {
                        title: {
                            display: true,
                            text: m.y_label,
                        },
                        beginAtZero: true,
                        grid: {
                            color: '#FFFFFF',
                        },
                        stacked: false,
                    },
                    x: {
                        title: {
                            display: true,
                            text: m.x_label,
                        },
                        grid: {
                            color: '#FFFFFF',
                        },
                        ticks: {
                            maxRotation: 90,
                            minRotation: 65
                        },
                        stacked: true,
                    },
                },
                plugins: {
                    customCanvasBackgroundColor: {
                        color: '#E5E4EE',
                    }
                }
            },
            plugins: [pluginCanvasBackgroundColor],
        });
        buildPlotDownload(myChart, m.id, fname);
        if (m.log_toggle) {
            buildLogToggle(myChart, "bar-" + m.id);
        }
    } else if (element instanceof Line) {
        let l = element;
        var ctx = document.getElementById('chart-line-' + l.id);
        var data = {
            labels: l.x_values,
            datasets: [{
                label: l.name,
                data: l.y_values,
                fill: false,
                borderColor: 'rgb(75, 192, 192)',
            }]
        };
        var x_axis_type = 'linear';
        var y_axis_type = 'linear';
        if (l.log_x) {
            x_axis_type = 'logarithmic';
        }
        if (l.log_y) {
            y_axis_type = 'logarithmic';
        }
        var myChart = new Chart(ctx, {
            type: 'line',
            data: data,
            options: {
                scales: {
                    y: {
                        title: {
                            display: true,
                            text: l.y_label,
                        },
                        beginAtZero: true,
                        type: y_axis_type,
                        grid: {
                            color: '#FFFFFF',
                        },
                    },
                    x: {
                        title: {
                            display: true,
                            text: l.x_label,
                        },
                        grid: {
                            color: '#FFFFFF',
                        },
                        ticks: {
                            maxRotation: 90,
                            minRotation: 65
                        },
                        type: x_axis_type,
                    },
                },
                plugins: {
                    customCanvasBackgroundColor: {
                        color: '#E5E4EE',
                    }
                }
            },
            plugins: [pluginCanvasBackgroundColor],
        });
        buildPlotDownload(myChart, l.id, fname);
    } else if (element instanceof Hexbin) {
        let h = element;
        var ctx = document.getElementById('chart-hexbin-' + h.id);
        buildPlotDownload(myChart, h.id, fname);
        const width = 928;
        const height = width;
        const radius = h.radius;
        const marginTop = 20;
        const marginRight = 20;
        const marginBottom = 30 + radius;
        const marginLeft = 60 + radius;

        // Create the positional scales.
            const x = d3.scaleLinear()
            .domain([h.min[0], h.max[0]])
            .range([marginLeft, width - marginRight]);

        const y = d3.scaleLinear()
            .domain([h.min[1], h.max[1]])
            .rangeRound([height - marginBottom, marginTop]);

        // Bin the data.
        const hexbin = d3.hexbin()
            .x(d => x(d["x"]))
            .y(d => y(d["y"]))
            .radius(radius * width / 928)
            .extent([[marginLeft, marginTop], [width - marginRight, height - marginBottom]]);

        // const bins = hexbin(h.bins);
        const bins = h.bins;
        const mbin = Math.max(...bins.map(v => v.length));

        // Create the color scale.
            const color = d3.scaleSequential(d3.interpolateBuPu)
            .domain([0, d3.max(bins, d => d.length)]);

        // Create the container SVG.
            const svg = d3.create("svg")
            .attr("viewBox", [0, 0, width, height]);

        // Append the scaled hexagons.
            svg.append("g")
            .attr("fill", "#ddd")
            .attr("stroke", "black")
            .selectAll("path")
            .data(bins)
            .enter().append("path")
            .attr("transform", d => `translate(${d.x},${d.y})`)
            .attr("d", hexbin.hexagon())
            .attr("fill", bin => color(bin.length));

        // Append the axes.
            svg.append("g")
            .attr("transform", `translate(0,${height - marginBottom + radius})`)
            .call(d3.axisBottom(x).ticks(width / 80, ""))
            //.call(g => g.select(".domain").remove())
            .call(g => g.append("text")
                .attr("x", width)
                .attr("y", 28)
                .attr("fill", "currentColor")
                .attr("font-weight", "bold")
                .attr("text-anchor", "end")
                .text("Coverage"));

        svg.append("g")
            .attr("transform", `translate(${marginLeft - radius - 9},0)`)
            .call(d3.axisLeft(y).ticks(null, ".1s").tickFormat((d, i) => d3.format(".1e")(Math.pow(10, d))))
            //.call(g => g.select(".domain").remove())
            .call(g => g.append("text")
                .attr("x", 0 - radius - 10)
                .attr("y", 0)
                .attr("dy", ".71em")
                .attr("fill", "currentColor")
                .attr("font-weight", "bold")
                .attr("text-anchor", "start")
                .text("Length"));


        var inner_svg = svg.append("svg")
            .attr("transform", `translate(${width - 350},0)`);
        Legend(color, { given_svg: inner_svg, tickFormat: (d) => Math.pow(10, d) });

        ctx.append(svg.node());
    } else if (element instanceof Heatmap) {
        let h = element;
        var ctx = document.getElementById('chart-heatmap-' + h.id);
        const data_points = h.values.map(function(e, i) {
            return e.map(function(f, j) {
                return {x: i, y: j, v: f, x_label: h.x_labels[i], y_label: h.y_labels[j]};
            });
        }).flat();
        const data = {
            datasets: [{
                label: 'My Matrix',
                data: data_points,
                backgroundColor(context) {
                    const value = context.dataset.data[context.dataIndex].v;
                    return getColor(value, 0.0);
                },
                width: ({chart}) => (chart.chartArea || {}).width / h.x_labels.length,
                height: ({chart}) =>(chart.chartArea || {}).height / h.y_labels.length
            }]
        };
        var myChart = new Chart(ctx, {
            type: 'matrix',
            data: data,
            options: {
                aspectRatio: 1,
                plugins: {
                    legend: false,
                    tooltip: {
                        callbacks: {
                            title() {
                                return '';
                            },
                            label(context) {
                                const v = context.dataset.data[context.dataIndex];
                                return [v.x_label + ' - ', v.y_label + ':', v.v];
                            }
                        }
                    },
                    customCanvasBackgroundColor: {
                        color: '#E5E4EE',
                    }
                },
                scales: {
                    x: {
                        ticks: {
                            stepSize: 1,
                            callback: ((context, index) => {
                                return h.x_labels[context];
                            })
                        },
                        grid: {
                            display: false
                        },
                        position: 'top',
                    },
                    y: {
                        offset: true,
                        ticks: {
                            stepSize: 1,
                            callback: ((context, index) => {
                                return h.y_labels[context];
                            })
                        },
                        grid: {
                            display: false
                        }
                    }
                }
            },
            plugins: [pluginCanvasBackgroundColor],
        });
        buildPlotDownload(myChart, h.id, fname);
        buildColorSlider(myChart, h.id);
    }
}

for (let key in objects.tables) {
    let table = objects.tables[key];
    buildTableDownload(table, key, key + '_' + fname);
}
