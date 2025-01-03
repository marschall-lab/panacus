/* global bootstrap: false */
(() => {
'use strict'
const tooltipTriggerList = Array.from(document.querySelectorAll('[data-bs-toggle="tooltip"]'))
tooltipTriggerList.forEach(tooltipTriggerEl => {
new bootstrap.Tooltip(tooltipTriggerEl)
})
})()

const plots = hists.concat(growths);

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

for (let i=0; i < hists.length; i++) {
    var h = hists[i];
    var ctx = document.getElementById('chart-hist-' + h.count);
    var myChart = new Chart(ctx, {
        type: 'bar',
        data: {
            labels: h.index,
            datasets: [{
                label: fname,
                data: h.coverage,
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
                        text: '#' + h.count + 's',
                    },
                    beginAtZero: true,
                    grid: {
                        color: '#FFFFFF',
                    }
                },
                x: {
                    title: {
                        display: true,
                        text: 'taxa',
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
    buildPlotDownload(myChart, h, fname);
    buildHistTableDownload(myChart, h, fname);
    buildLogToggle(myChart, h);
}

//console.log(groups);

if (typeof groups != "undefined") {
    for (let i=0; i < groups.length; i++) {
        var g = groups[i];
        var ctx = document.getElementById('chart-group-' + g.count);
        var myChart;
        if (g.is_hist) {
            myChart = new Chart(ctx, {
                type: 'bar',
                data: {
                    labels: g.index,
                    datasets: [{
                        label: fname,
                        data: g.length,
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
                                text: '#groups',
                            },
                            beginAtZero: true,
                            grid: {
                                color: '#FFFFFF',
                            }
                        },
                        x: {
                            title: {
                                display: true,
                                text: '#' + g.count + 's',
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
            buildPlotDownload(myChart, g, fname);
        } else {
            myChart = new Chart(ctx, {
                type: 'bar',
                data: {
                    labels: g.index,
                    datasets: [{
                        label: fname,
                        data: g.length,
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
                                text: '#' + g.count + 's',
                            },
                            beginAtZero: true,
                            grid: {
                                color: '#FFFFFF',
                            }
                        },
                        x: {
                            title: {
                                display: true,
                                text: 'groups',
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
            buildPlotDownload(myChart, g, fname);
        }
        buildLogToggle(myChart, g);
    }
}

if (typeof info != "undefined" && info != "") {
    buildInfoTableDownload(info, "graph", fname);
    buildInfoTableDownload(info, "node", fname);
    buildInfoTableDownload(info, "path", fname);
    buildInfoTableDownload(info, "group", fname);
}

for (let i=0; i < growths.length; i++) {
    var g = growths[i];
    var ctx = document.getElementById('chart-growth-' + g.count);
    var myChart = new Chart(ctx, {
        type: 'bar',
        data: {
            labels: g.index,
            datasets: Array.from(g.getThresholds().entries()).reverse().map(function([i, [c, q]]) {
                return {
                    label: 'coverage \u2265 ' + c + ', quorum \u2265 ' + (q*100).toFixed(0) + '%',
                    data: g.getGrowthFor(c, q),
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
                        text: '#' + g.count + 's',
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
                        text: 'taxa',
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
    buildPlotDownload(myChart, g, fname);
    buildGrowthTableDownload(myChart, g, fname);
}

var tabs = document.querySelectorAll('button[data-bs-toggle="tab"]')
tabs.forEach(function(tab) {
    tab.addEventListener('show.bs.tab', function (event) {
        document.querySelector(event.target.dataset.bsTarget).classList.remove('d-none');
        document.querySelector(event.relatedTarget.dataset.bsTarget).classList.add('d-none');
    });
});
