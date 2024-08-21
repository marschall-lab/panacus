# Pangenome Coverage and Growth Statistics for Minigraph-Cactus

*TIP:*
You can execute this file by downloading it and running: 
````bash
cat pangenome_growth_minigraph_cactus.md | sed -n '/```shell/,/```/p' | sed '/```/d' | bash
````

This example shows how to ccompute coverage and pangenome growth statistics for the HPRC v.1.1 mc, chr 22: 

1. Download the graph:
```shell
wget https://s3-us-west-2.amazonaws.com/human-pangenomics/pangenomes/freeze/freeze1/minigraph-cactus/hprc-v1.1-mc-grch38/hprc-v1.1-mc-grch38.chroms/chr22.vg
```
2. Convert to GFA (this graph is provided in VG format and requires conversion into GFA with [vg](https://github.com/vgteam/vg):
```shell
vg view --gfa chr22.vg > chr22.hprc-v1.1-mc-grch38.gfa
```
3. Prepare file to select subset of paths corresponding to haplotypes:
```shell
grep -e '^W' chr22.hprc-v1.1-mc-grch38.gfa | cut -f2-6 | awk '{ print $1 "#" $2 "#" $3 ":" $4 "-" $5 }' > chr22.hprc-v1.1-mc-grch38.paths.txt
grep -ve 'grch38\|chm13' chr22.hprc-v1.1-mc-grch38.paths.txt > chr22.hprc-v1.1-mc-grch38.paths.haplotypes.txt
```
4. Run `panacus histgrowth` to calculate coverage and pangenome growth for nodes (default) with coverage/quorum thresholds 1/0, 2/0, 1/1, 1/0.5, and 1/0.1 using up to 4 threads:
```shell
RUST_LOG=info panacus histgrowth -t4 -l 1,2,1,1,1 -q 0,0,1,0.5,0.1 -S -a -s chr22.hprc-v1.1-mc-grch38.paths.haplotypes.txt chr22.hprc-v1.1-mc-grch38.gfa > chr22.hprc-v1.1-mc-grch38.histgrowth.node.tsv
```
5. Visualize coverage histogram and pangenome growth curve with estimated growth parameters:
```shell
panacus-visualize -e chr22.hprc-v1.1-mc-grch38.histgrowth.node.tsv > chr22.hprc-v1.1-mc-grch38.histgrowth.node.pdf
```

![coverage histogram and pangenome growth of nodes in chr22.hprc-v1.1-mc-grch38.gfa](docs/chr22.hprc-v1.1-mc-grch38.histgrowth.node.png?raw=true "coverage and pangenome growth statistics on the HPRC v.1.1 mc-grch38, chr 22")

