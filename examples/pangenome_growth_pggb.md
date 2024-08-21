# Pangenome Coverage and Growth Statistics for PGGB

> ![TIP]
> You can execute this file by downloading it and running: ````cat pangenome_growth_pggb.md | sed -n '/```bash/,/```/p' | sed '/```/d'| bash````

Here's a quick example for computing coverage and pangenome growth statistics on the HPRC v.1.0 pggb, chr 22: 

1. Download and unpack the graph:
```shell
wget https://s3-us-west-2.amazonaws.com/human-pangenomics/pangenomes/freeze/freeze1/pggb/chroms/chr22.hprc-v1.0-pggb.gfa.gz
gunzip chr22.hprc-v1.0-pggb.gfa.gz
```
2. Prepare file to select subset of paths corresponding to haplotypes:
```shell
grep '^P' chr22.hprc-v1.0-pggb.gfa | cut -f2 | grep -ve 'grch38\|chm13' > chr22.hprc-v1.0-pggb.paths.haplotypes.txt
```
3. Run `panacus histgrowth` to calculate coverage and pangenome growth for nodes (default) with coverage/quorum thresholds 1/0, 2/0, 1/1, 1/0.5, and 1/0.1 using up to 4 threads:
```shell
RUST_LOG=info panacus histgrowth -t4 -l 1,2,1,1,1 -q 0,0,1,0.5,0.1 -S -a -s chr22.hprc-v1.0-pggb.paths.haplotypes.txt chr22.hprc-v1.0-pggb.gfa > chr22.hprc-v1.0-pggb.histgrowth.node.tsv
```
4. Visualize coverage histogram and pangenome growth curve:
```shell
panacus-visualize chr22.hprc-v1.0-pggb.histgrowth.node.tsv > chr22.hprc-v1.0-pggb.histgrowth.node.pdf
```

![coverage histogram and pangenome growth of nodes in chr22.hprc-v1.0-pggb.gfa](docs/chr22.hprc-v1.0-pggb.histgrowth.node.png?raw=true "coverage and pangenome growth statistics on the HPRC v.1.0 pggb, chr 22")

