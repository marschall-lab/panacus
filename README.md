# Generate Growth Plots for Pangenome Graphs

![Cumulative, major, and shared number of nodes in hprc-v1.0-pggb.gfa](docs/nodes_ordered.png?raw=true "Cumulative, major, and shared number of nodes in hprc-v1.0-pggb.gfa")


![Cumulative, major, and shared number of edges in hprc-v1.0-pggb.gfa](docs/edges_ordered.png?raw=true "Cumulative, major, and shared number of edges in hprc-v1.0-pggb.gfa")
## Build

`cargo build --release`

## Run

```
pangenome-growth 0.1
Daniel Doerr <daniel.doerr@hhu.de>
Calculate growth statistics for pangenome graphs

USAGE:
    pangenome-growth [OPTIONS] <GRAPH> <SAMPLES>

ARGS:
    <GRAPH>      graph in GFA1 format
    <SAMPLES>    file of samples; their order determines the cumulative count

OPTIONS:
    -d, --minimum_depth <MIN_DEPTH>     minimum depth of a node to be considered in cumulative count
                                        [default: 1]
    -f, --fix_first                     only relevant if permuted_repeats > 0; fixes the first
                                        sample (and its haplotypes) to be the first in all
                                        permutations
    -h, --help                          Print help information
    -m, --merge_chromosomes             merge haplotype paths within samples whose names start with
                                        'chr'
    -r, --permuted_repeats <PERMUTE>    if larger 0, the haplotypes are not added in given order,
                                        but by a random permutation; the process is repeated a given
                                        number of times [default: 0]
    -t, --type <COUNT_TYPE>             type: node or edge count [default: nodes] [possible values:
                                        nodes, edges, bp]
    -V, --version                       Print version information
```
