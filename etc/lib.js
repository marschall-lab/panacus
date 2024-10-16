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

class Group {
    constructor(count_type, index, length, is_hist) {
        this.count = count_type;
        this.index = index;
        this.length = length;
        this.is_hist = is_hist;
    }
}


class Growth {
    constructor(count_type, index, coverage_t, quorum_t, growths) {
        this.count = count_type; 
        this.index = index;
        this.growths = {};
        var srt = [];
        for (let i = 0; i < coverage_t.length; i++) {
            let t = [coverage_t[i], quorum_t[i]];
            srt[i] = [quorum_t[i], coverage_t[i]];
            this.growths[t] = growths[i];
        }
        srt.sort();
        this.coverage_t = srt.map(([q, c]) => c);
        this.quorum_t = srt.map(([q, c]) => q);
    }

    getThresholds() {
        let ts = [];
        for (let i = 0; i < this.coverage_t.length; i++) {
            ts[i] = [this.coverage_t[i], this.quorum_t[i]];
        }

        return ts;
    }

    getGrowthFor(c, q) {
        return this.growths[[c, q]];
    }
}


function buildPlotDownload(chart, obj, prefix) {
    console.log('btn-download-plot-' + obj.constructor.name.toLowerCase() + '-' + obj.count);
    document.getElementById('btn-download-plot-' + obj.constructor.name.toLowerCase() + '-' + obj.count).onclick = function() {
        var a = document.createElement('a');
        a.href = chart.toBase64Image();
        a.download = prefix + '_' + obj.constructor.name.toLowerCase() + '_' + obj.count + '.png';
        a.click();
    };
}


function buildHistTableDownload(chart, obj, prefix) {
    document.getElementById('btn-download-table-hist-' + obj.count).onclick = function() {

        var table = 'panacus\thist\ncount\t' + obj.count + '\n\t\n\t\n';

        for (var i=0; i < obj.index.length; i++) {
            table += obj.index[i] + '\t' + obj.coverage[i] + '\n';
        }

        let blob = new Blob([table], {type: 'text/plain'});
        var a = document.createElement('a');
        a.href = URL.createObjectURL(blob);
        a.download = prefix + '_hist_' + obj.count + '.tsv';
        a.click();
    };
}


function buildGrowthTableDownload(chart, obj, prefix) {
    document.getElementById('btn-download-table-growth-' + obj.count).onclick = function() {

        var table = '';

        var thresholds = obj.getThresholds();
        var growths = 'panacus\tgrowth' 
        if (typeof obj.index[0] === 'string' || obj.index[0] instanceof String) {
            growths = 'panacus\tordered-growth'
        }
        var counts = '\ncount\t' + obj.count 
        cs = '\ncoverage\t' + thresholds[0][0];
        qs = '\nquorum\t' + thresholds[0][1];
        zero = '\n0\tNaN'
        for (var i=1; i < thresholds.length; i++) {
            growths += '\tgrowth';
            counts += '\t' + obj.count;
            cs += '\t' + thresholds[i][0];
            qs += '\t' + thresholds[i][1];
            zero += '\tNaN';
        }
        table += growths + counts + cs + qs + zero + '\n';

        for (var i=0; i < obj.index.length; i++) {
            table += obj.index[i];
            for (var j=0; j < thresholds.length; j++) {
                table += '\t' + obj.getGrowthFor(thresholds[j][0], thresholds[j][1])[i];
            }
            table += '\n';
        }

        let blob = new Blob([table], {type: 'text/plain'});
        var a = document.createElement('a');
        a.href = URL.createObjectURL(blob);
        a.download = prefix + '_growth_' + obj.count + '.tsv';
        if (typeof obj.index[0] === 'string' || obj.index[0] instanceof String) {
            a.download = prefix + '_orderedgrowth_' + obj.count + '.tsv';
        }
        a.click();
    };
}

function buildInfoTableDownload(table, infoType, prefix) {
    document.getElementById('btn-download-table-info-' + infoType).onclick = function() {
        let blob = new Blob([table], {type: 'text/plain'});
        var a = document.createElement('a');
        a.href = URL.createObjectURL(blob);
        a.download = prefix + '_info.tsv';
        a.click();
    };
}


function buildLogToggle(chart, name) {
    console.log(name);
    document.getElementById('btn-logscale-plot-' + name).addEventListener('change', function(event) {
        if (event.currentTarget.checked) {
            chart.options.scales.y.type = 'logarithmic';
        } else {
            chart.options.scales.y.type = 'linear';
        }
        chart.update();
    });
}

