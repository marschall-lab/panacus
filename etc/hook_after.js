/* global bootstrap: false */
(() => {
'use strict'
const tooltipTriggerList = Array.from(document.querySelectorAll('[data-bs-toggle="tooltip"]'))
tooltipTriggerList.forEach(tooltipTriggerEl => {
new bootstrap.Tooltip(tooltipTriggerEl)
})
})()

console.time('hook');

// Copyright 2021, Observable Inc.
// Released under the ISC license.
// https://observablehq.com/@d3/color-legend
function Legend(color, {
  given_svg,
  title,
  tickSize = 6,
  width = 320,
  height = 44 + tickSize,
  marginTop = 18,
  marginRight = 0,
  marginBottom = 16 + tickSize,
  marginLeft = 0,
  ticks = width / 64,
  tickFormat,
  tickValues
} = {}) {

  function ramp(color, n = 256) {
    const canvas = document.createElement("canvas");
    canvas.width = n;
    canvas.height = 1;
    const context = canvas.getContext("2d");
    for (let i = 0; i < n; ++i) {
      context.fillStyle = color(i / (n - 1));
      context.fillRect(i, 0, 1, 1);
    }
    return canvas;
  }

  const svg = given_svg
      .attr("width", width)
      .attr("height", height)
      .attr("viewBox", [0, 0, width, height])
      .style("overflow", "visible")
      .style("display", "block");

  let tickAdjust = g => g.selectAll(".tick line").attr("y1", marginTop + marginBottom - height);
  let x;

  // Continuous
  if (color.interpolate) {
    const n = Math.min(color.domain().length, color.range().length);

    x = color.copy().rangeRound(d3.quantize(d3.interpolate(marginLeft, width - marginRight), n));

    svg.append("image")
        .attr("x", marginLeft)
        .attr("y", marginTop)
        .attr("width", width - marginLeft - marginRight)
        .attr("height", height - marginTop - marginBottom)
        .attr("preserveAspectRatio", "none")
        .attr("xlink:href", ramp(color.copy().domain(d3.quantize(d3.interpolate(0, 1), n))).toDataURL());
  }

  // Sequential
  else if (color.interpolator) {
    x = Object.assign(color.copy()
        .interpolator(d3.interpolateRound(marginLeft, width - marginRight)),
        {range() { return [marginLeft, width - marginRight]; }});

    svg.append("image")
        .attr("x", marginLeft)
        .attr("y", marginTop)
        .attr("width", width - marginLeft - marginRight)
        .attr("height", height - marginTop - marginBottom)
        .attr("preserveAspectRatio", "none")
        .attr("xlink:href", ramp(color.interpolator()).toDataURL());

    // scaleSequentialQuantile doesn’t implement ticks or tickFormat.
    if (!x.ticks) {
      if (tickValues === undefined) {
        const n = Math.round(ticks + 1);
        tickValues = d3.range(n).map(i => d3.quantile(color.domain(), i / (n - 1)));
      }
      if (typeof tickFormat !== "function") {
        tickFormat = d3.format(tickFormat === undefined ? ",f" : tickFormat);
      }
    }
  }

  // Threshold
  else if (color.invertExtent) {
    const thresholds
        = color.thresholds ? color.thresholds() // scaleQuantize
        : color.quantiles ? color.quantiles() // scaleQuantile
        : color.domain(); // scaleThreshold

    const thresholdFormat
        = tickFormat === undefined ? d => d
        : typeof tickFormat === "string" ? d3.format(tickFormat)
        : tickFormat;

    x = d3.scaleLinear()
        .domain([-1, color.range().length - 1])
        .rangeRound([marginLeft, width - marginRight]);

    svg.append("g")
      .selectAll("rect")
      .data(color.range())
      .join("rect")
        .attr("x", (d, i) => x(i - 1))
        .attr("y", marginTop)
        .attr("width", (d, i) => x(i) - x(i - 1))
        .attr("height", height - marginTop - marginBottom)
        .attr("fill", d => d);

    tickValues = d3.range(thresholds.length);
    tickFormat = i => thresholdFormat(thresholds[i], i);
  }

  // Ordinal
  else {
    x = d3.scaleBand()
        .domain(color.domain())
        .rangeRound([marginLeft, width - marginRight]);

    svg.append("g")
      .selectAll("rect")
      .data(color.domain())
      .join("rect")
        .attr("x", x)
        .attr("y", marginTop)
        .attr("width", Math.max(0, x.bandwidth() - 1))
        .attr("height", height - marginTop - marginBottom)
        .attr("fill", color);

    tickAdjust = () => {};
  }

  svg.append("g")
      .attr("transform", `translate(0,${height - marginBottom})`)
      .call(d3.axisBottom(x)
        .ticks(ticks, typeof tickFormat === "string" ? tickFormat : undefined)
        .tickFormat(typeof tickFormat === "function" ? tickFormat : undefined)
        .tickSize(tickSize)
        .tickValues(tickValues))
      .call(tickAdjust)
      .call(g => g.select(".domain").remove())
      .call(g => g.append("text")
        .attr("x", marginLeft)
        .attr("y", marginTop + marginBottom - height - 6)
        .attr("fill", "currentColor")
        .attr("text-anchor", "start")
        .attr("font-weight", "bold")
        .attr("class", "title")
        .text(title));

  return svg.node();
}

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
    } else if (element instanceof Hexbin) {
        console.time('hex');
        let h = element;
        var ctx = document.getElementById('chart-hexbin-' + h.id);
        buildPlotDownload(myChart, h.id, fname);
        const width = 928;
        const height = width;
        const radius = h.radius;
        const marginTop = 20;
        const marginRight = 20;
        const marginBottom = 30 + radius;
        const marginLeft = 40 + radius;

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

        // Create the color scale.
            const color = d3.scaleSequential(d3.interpolateBuPu)
            .domain([0, d3.max(bins, d => d.length) / 2]);

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
            .call(g => g.select(".domain").remove())
            .call(g => g.append("text")
                .attr("x", width - marginRight)
                .attr("y", -6)
                .attr("fill", "currentColor")
                .attr("font-weight", "bold")
                .attr("text-anchor", "end")
                .text("Coverage"));

        svg.append("g")
            .attr("transform", `translate(${marginLeft - radius},0)`)
            .call(d3.axisLeft(y).ticks(null, ".1s"))
            .call(g => g.select(".domain").remove())
            .call(g => g.append("text")
                .attr("x", 0 - radius - 20)
                .attr("y", marginTop)
                .attr("dy", ".71em")
                .attr("fill", "currentColor")
                .attr("font-weight", "bold")
                .attr("text-anchor", "start")
                .text("Length"));


        var inner_svg = svg.append("svg")
            .attr("transform", `translate(${width - 350},0)`);
        Legend(color, { given_svg: inner_svg });

        ctx.append(svg.node());
        console.timeEnd('hex');
    }
}

for (let key in objects.tables) {
    let table = objects.tables[key];
    buildTableDownload(table, key, key + '_' + fname);
}

console.timeEnd('hook');
