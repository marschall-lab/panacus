# Generate Rarefaction Statistics from Pangenome Graphs

## Build

`cargo build --release`

## Run

```
Daniel Doerr <daniel.doerr@hhu.de>
Calculate rarefaction statistics from pangenome graph

USAGE:
    pangenome-rarefaction [OPTIONS] <GRAPH> <SAMPLES>

ARGS:
    <GRAPH>      graph in GFA1 format
    <SAMPLES>    file of samples; their order determines the cumulative count

OPTIONS:
    -f, --fix_first                     only relevant if permuted_repeats > 0; fixes the first
                                        haplotype to be the first haplotype in all permutations
    -h, --help                          Print help information
    -r, --permuted_repeats <PERMUTE>    if larger 0, the haplotypes are not added in given order,
                                        but by a random permutation; the process is repeated a given
                                        number of times [default: 0]
    -t, --type <COUNT_TYPE>             type: node or edge count [default: nodes] [possible values:
                                        nodes, edges]
    -V, --version                       Print version information
```
