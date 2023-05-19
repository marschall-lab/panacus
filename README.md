# A Counting Tool for Pangenome Graphs

`panacus` is a tool for computing counting statistics of [GFA](https://github.com/GFA-spec/GFA-spec/blob/master/GFA1.md) files. It supports `P` and
`W` lines, but requires that the graph is `blunt`, i.e., nodes do not overlap and consequently, each link (`L`) points from the end of one segment
(`S`) to the start of another.

`panacus` supports the following calculations:

- coverage histogram
- pangenome growth statistics
- path-/group-resolved coverage table

## Dependencies

`panacus` is written in (RUST)[https://www.rust-lang.org/] and requires a working RUST build system for installation. See [here](https://www.rust-lang.org/tools/install) for more details.

- clap
- itertools
- quick-csv
- rand
- rayon
- regex
- rustc-hash
- strum, strum_macros

`panacus` provides a Python script for visualizing the calculated counting statistics and requires the following Python libraries:

- matplotlib
- numpy
- pandas
- scikit-learn
- scipy
- seaborn

## Get `panacus`

```shell
git clone git@github.com:marschall-lab/panacus.git
```

## Build

```shell
cd panacus
cargo build --release
```

The compiled binary can be found in `target/release/` and is called `panacus`.

## Run

```console
$ ./target/release/panacus
Calculate count statistics for pangenomic data

Usage: panacus <COMMAND>

Commands:
  histgrowth          Run in default mode, i.e., run hist and growth successively and output the results of the latter
  hist                Calculate coverage histogram from GFA file
  growth              Construct growth table from coverage histogram
  ordered-histgrowth  Compute growth table for order specified in grouping file (or, if non specified, the order of paths in the GFA file)
  table               Compute coverage table for count items
  help                Print this message or the help of the given subcommand(s)

Options:
  -h, --help     Print help
  -V, --version  Print version
```

## Pangenome Growth Statistics

Here's a quick example for computing pangenome growth statistics on the HPRC v.1.0 pggb, chr 22: 

1. Download and unpack the graph:
```shell
wget https://s3-us-west-2.amazonaws.com/human-pangenomics/pangenomes/freeze/freeze1/pggb/chroms/chr22.hprc-v1.0-pggb.gfa.gz
gunzip chr22.hprc-v1.0-pggb.gfa.gz
```
2. Prepare file to group paths by sample:
```shell
grep '^P' chr22.hprc-v1.0-pggb.gfa | cut -f2 > chr22.hprc-v1.0-pggb.paths.txt
cut -f1 -d\# chr22.hprc-v1.0-pggb.paths.txt > chr22.hprc-v1.0-pggb.groupnames.txt
paste chr22.hprc-v1.0-pggb.paths.txt chr22.hprc-v1.0-pggb.groupnames.txt > chr22.hprc-v1.0-pggb.groups.txt
```
3. Prepare file to select subset of paths corresponding to haplotypes:
```shell
grep -ve 'grch38\|chm13' chr22.hprc-v1.0-pggb.paths.txt > chr22.hprc
-v1.0-pggb.paths.haplotypes.txt
```
4. Run `panacus histgrowth` to calculate pangenome growth for nodes (default) with quorum tresholds 0, 1, 0.5, and 0.1 using up to 4 threads:
```shell
RUST_LOG=info ./target/release/panacus histgrowth chr22.hprc-v1.0-pggb.gfa -t4 -q 0,1,0.5,0.1 -g chr22.hprc-v1.0-pggb.groups.txt -s chr22.hprc-v1.0-pggb.paths.haplotypes.txt chr22.hprc-v1.0-pggb.gfa > chr22.hprc-v1.0-pggb.histgrowth.node.txt
```
5. Visualize growth curve and estimate growth parameters :
```shell
./scripts/panacus-visualize.py -e pggb/chr22.hprc-v1.0-pggb.histgrowth.node.txt > pggb/chr22.hprc-v1.0-pggb.histgrowth.node.pdf
```

![ nodes in hprc-v1.0-pggb.gfa](docs/chr22.hprc-v1.0-pggb.histgrowth.node.png?raw=true "pangenome growth statistics on the HPRC v.1.0 pggb, chr 22")

