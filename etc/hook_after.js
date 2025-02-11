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
                    //{x: 1, y: 1, v: 11},
                    //{x: 1, y: 2, v: 12},
                    //{x: 1, y: 3, v: 13},
                    //{x: 2, y: 1, v: 21},
                    //{x: 2, y: 2, v: 22},
                    //{x: 2, y: 3, v: 23},
                    //{x: 3, y: 1, v: 31},
                    //{x: 3, y: 2, v: 32},
                    //{x: 3, y: 3, v: 33}
                backgroundColor(context) {
                    const value = context.dataset.data[context.dataIndex].v;
                    return getColor(value, 0.0);
                },
                //borderColor(context) {
                //    const value = context.dataset.data[context.dataIndex].v;
                //    const alpha = value;
                //    return colorize(0, 100, 0, alpha);
                //},
                // borderWidth: 0,
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

// var tabs = document.querySelectorAll('button[data-bs-toggle="tab"]')
// tabs.forEach(function(tab) {
//     tab.addEventListener('show.bs.tab', function (event) {
//         document.querySelector(event.target.dataset.bsTarget).classList.remove('d-none');
//         document.querySelector(event.relatedTarget.dataset.bsTarget).classList.add('d-none');
//     });
// });
