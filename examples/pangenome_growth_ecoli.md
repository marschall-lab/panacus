# Pangenome Coverage and Growth Statistics for PGGB

*TIP:*
You can try this example by downloading this file and running:
````bash
cat pangenome_growth_ecoli.md | sed -n '/```shell/,/```/p' | sed '/```/d' | bash
````

1. Download and unpack the graph:
```shell
wget -c https://zenodo.org/record/7937947/files/ecoli50.gfa.zst
zstd -d ecoli50.gfa.zst
```

2. Run `panacus histgrowth` to calculate coverage and pangenome growth for basepairs with quorum thresholds 0, 1, 0.5, and 0.1 using up to 4 threads:
```shell
RUST_LOG=info panacus histgrowth ecoli50.gfa -c bp -q 0,1,0.5,0.1 -t 4 > ecoli50.gfa.histgrowth.tsv
```

3. Visualize coverage histogram and pangenome growth curve with estimation of growth parameters. Place the legend in the upper left:
```shell
panacus-visualize -e -l "upper left" ecoli50.gfa.histgrowth.tsv > ecoli50.gfa.histgrowth.tsv.pdf
```

![coverage histogram and pangenome growth of bps in ecoli50.gfa](/docs/ecoli50.gfa.histgrowth.png?raw=true "coverage and pangenome growth statistics on the Ecoli50 graph")
