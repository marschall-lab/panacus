# Generate an HTML report for your pangenome graph!

Instead of tab-separated tables, `panacus` supports for many commands also HTML output. The generated report page is interactive and self-contained. 

1. Follow steps 1-2 from above.
2. Run `panacus histgrowth` with settings to output stats for all graph features (`-c all`), include coverage histogram in output (`-a`), and set
   output to HTML (`-o html`):
```shell
RUST_LOG=info panacus histgrowth -t4 -l 1,2,1,1,1 -q 0,0,1,0.5,0.1 -S -s chr22.hprc-v1.0-pggb.paths.haplotypes.txt -c all -a -o html chr22.hprc-v1.0-pggb.gfa > chr22.hprc-v1.0-pggb.histgrowth.html
```

:point_right: :point_right: :point_right: **view the resulting [HTML report here](https://htmlpreview.github.io/?https://github.com/marschall-lab/panacus/blob/main/docs/chr22.hprc-v1.0-pggb.histgrowth.html)!**

![panacus report (coverage histogram) for chr22.hprc-v1.0-pggb.gfa](docs/chr22.hprc-v1.0-pggb.report.histogram.logscale.highlight.png?raw=true "pangenome report of chr22.hprc-v1.0-pggb.gfa showing coverage histogram in logsacle")

### Figure legend
1. Navigate between coverage histograms for bp, node, and edge through tabs 
2. Toggle log-scale on Y-axis
3. Download plot as PNG file
4. Download raw data as tab-separated-values (TSV) file
5. Choose between light and dark theme
6. Proceed to pangenome growth plots

![panacus report (pangenome growth) for chr22.hprc-v1.0-pggb.gfa](docs/chr22.hprc-v1.0-pggb.report.growth.disabled.highlight.png?raw=true "pangenome report of chr22.hprc-v1.0-pggb.gfa showing pangenome growth plots with disabled curves")

### Figure legend
1. Navigate between coverage histograms for bp, node, and edge through tabs 
2. Disable curves that you do not want to view by clicking on legend
3. Download plot as PNG file
4. Download raw data as tab-separated-values (TSV) file
5. Choose between light and dark theme

