# Ordered Pangenome Growth Statistics

*TIP:*
You can try this example by downloading this file and running: 
````bash
cat ordered_pangenome_growth_statistics.md | sed -n '/```shell/,/```/p' | sed '/```/d' | bash
````

Sometimes it is interesting to look at the pangenome growth when samples are processed in a specific order rather than considering all all possible
orders. `panacus`' capability to construct such plots is illustrated here by the example of the GRCh38-based HPRC v.1.0 minigraph-cactus graph (all
chromosomes). The example reproduces Figure 3g(left) from the publication [A draft human pangenome
reference](https://doi.org/10.1038/s41586-023-05896-x) that quantifies pangenome growth of the amount of non-reference (GRCh38) sequence of the
minigraph-cactus based human pangenome reference graph.

1. Download and unpack the graph:
```shell
wget https://s3-us-west-2.amazonaws.com/human-pangenomics/pangenomes/freeze/freeze1/minigraph-cactus/hprc-v1.0-mc-grch38.gfa.gz
gunzip hprc-v1.0-mc-grch38.gfa.gz
```
2. Establish order of samples in the growth statistics:
```shell
echo 'HG03492 HG00438 HG00621 HG00673 HG02080 HG00733 HG00735 HG00741 HG01071 HG01106 HG01109 HG01123 HG01175 HG01243 HG01258 HG01358 HG01361 HG01928
HG01952 HG01978 HG02148 HG01891 HG02055 HG02109 HG02145 HG02257 HG02486 HG02559 HG02572 HG02622 HG02630 HG02717 HG02723 HG02818 HG02886 HG03098
HG03453 HG03486 HG03516 HG03540 HG03579 NA18906 NA20129 NA21309' | tr ' ' '\n' > hprc-v1.0-mc-grch38.order.samples.txt
```
3. Exclude paths from reference genome GRCh38
```shell
grep '^P' hprc-v1.0-mc-grch38.gfa | cut -f2 | grep -ie 'grch38' > hprc-v1.0-mc-grch38.exclude.grch38.txt
```
4. Run `panacus ordered-histgrowth` to calculate pangenome growth for basepairs with coverage thresholds 1,2,3, and 42 using up to 4 threads:
```shell
RUST_LOG=info panacus ordered-histgrowth -c bp -t4 -l 1,2,3,42 -S -e hprc-v1.0-mc-grch38.exclude.grch38.txt -O hprc-v1.0-mc.grch38.order.samples.txt hprc-v1.0-mc-grch38.gfa > hprc-v1.0-mc-grch38.ordered-histgrowth.bp.tsv
```
(The log will report some errors regarding missing order information of CHM13 paths. These paths will be ignored in the plot, which is the intended
behavior of this command line call)

5. Visualize growth curve and estimate growth parameters :
```shell
panacus-visualize hprc-v1.0-mc-grch38.ordered-histgrowth.bp.tsv > hprc-v1.0-mc-grch38.ordered-histgrowth.bp.pdf
```

![ordered pangenome growth of bps in hprc-v1.0-mc-grch38.gfa](/docs/hprc-v1.0-mc-grch38.ordered-histgrowth.bp.png?raw=true "pangenome growth of non-reference sequence in the HPRC v.1.0 MC GRCh38 graph")

