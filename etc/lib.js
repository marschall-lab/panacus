/*!
  * Panacus JS library
  */

const PCOLORS = ['#f77189', '#bb9832', '#50b131', '#36ada4', '#3ba3ec', '#e866f4'];

class Bar {
    constructor(id, name, x_label, y_label, labels, values, log_toggle) {
        this.id = id;
        this.name = name;
        this.x_label = x_label;
        this.y_label = y_label;
        this.labels = labels;
        this.values = values;
        this.log_toggle = log_toggle;
    }
}

class MultiBar {
    constructor(id, names, x_label, y_label, labels, values, log_toggle) {
        this.id = id;
        this.names = names;
        this.x_label = x_label;
        this.y_label = y_label;
        this.labels = labels;
        this.values = values;
        this.log_toggle = log_toggle;
    }
}

class Hexbin {
    constructor(id, min, max, radius, bins) {
        this.id = id;
        this.min = min;
        this.max = max;
        this.radius = radius;
        this.bins = bins;
    }
}

class Heatmap {
    constructor(id, name, x_labels, y_labels, values) {
        this.id = id;
        this.name = name;
        this.x_labels = x_labels;
        this.y_labels = y_labels;
        this.values = values;
    }
}

class Line {
    constructor(id, name, x_label, y_label, log_x, log_y, x_values, y_values) {
        this.id = id;
        this.name = name;
        this.x_label = x_label;
        this.y_label = y_label;
        this.log_x = log_x;
        this.log_y = log_y;
        this.x_values = x_values;
        this.y_values = y_values;
    }
}

function buildPlotDownload(chart, obj, prefix) {
    document.getElementById('btn-download-plot-' + obj).onclick = function() {
        var a = document.createElement('a');
        a.href = chart.toBase64Image();
        a.download = prefix + '_' + obj + '.png';
        a.click();
    };
}

function buildTableDownload(table, id, prefix) {
    document.getElementById('btn-download-table-' + id).onclick = function() {
        let blob = new Blob([table], {type: 'text/plain'});
        var a = document.createElement('a');
        a.href = URL.createObjectURL(blob);
        a.download = prefix + '_table.tsv';
        a.click();
    };
}

function buildLogToggle(chart, name) {
    document.getElementById('btn-logscale-plot-' + name).addEventListener('change', function(event) {
        if (event.currentTarget.checked) {
            chart.options.scales.y.type = 'logarithmic';
        } else {
            chart.options.scales.y.type = 'linear';
        }
        chart.update();
    });
}

function getColor(value, zero) {
    const corrected = (value - zero) / (1.0 - zero);
    const flipped = 1.0 - corrected;
    const r = 255.0 - flipped * (255.0 - 179.0);
    const g = 255.0 - flipped * 255.0;
    const b = 255.0 - flipped * 255.0;
    const text = 'rgb(' + r + ',' + g + ',' + b + ')';
    return text;
}

function buildColorSlider(chart, name) {
    document.getElementById('btn-colorscale-' + name).addEventListener('change', function(event) {
        chart.data.datasets[0].backgroundColor = (context) => {
            const value = context.dataset.data[context.dataIndex].v;
            return getColor(value, event.currentTarget.value)
        };
        chart.options.plugins.tooltip.callbacks = {
            title() {
                return '';
            },
            label(context) {
                const v = context.dataset.data[context.dataIndex];
                return [v.x_label + ' - ', v.y_label + ':', v.v];
            }
        }
        chart.update();
    });
}
