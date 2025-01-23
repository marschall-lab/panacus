[![Rust Build](https://github.com/marschall-lab/panacus/actions/workflows/rust_build.yml/badge.svg)](https://github.com/marschall-lab/panacus/actions/workflows/rust_build.yml) [![Anaconda-Server Badge](https://anaconda.org/bioconda/panacus/badges/version.svg)](https://conda.anaconda.org/bioconda) [![Anaconda-Server Badge](https://anaconda.org/bioconda/panacus/badges/platforms.svg)](https://anaconda.org/bioconda/panacus) [![Anaconda-Server Badge](https://anaconda.org/bioconda/panacus/badges/license.svg)](https://anaconda.org/bioconda/panacus)

# A Counting Tool for Pangenome Graphs

![panacus is a counting tool for pangenome graphs](docs/panacus-illustration.png?raw=true "panacus is a counting tool for pangenome graphs")

`panacus` is a tool for calculating statistics for [GFA](https://github.com/GFA-spec/GFA-spec/blob/master/GFA1.md) files. It supports GFA files with `P` and
`W` lines, but requires that the graph is `blunt`, i.e., nodes do not overlap and consequently, each link (`L`) points from the end of one segment
(`S`) to the start of another.

`panacus` supports the following calculations:

- coverage histogram
- pangenome growth statistics
- path-/group-resolved coverage table

### Coverage Histogram
Histogram listing the number of features (nodes, edges, ...) that are visited by a certain number of paths.

### Pangenome Growth statistics
Describes how many features (nodes, edges, ...) one would expect on average if the graph was built from
1...n haplotypes.

To limit the amount of features that are part of the calculation (e.g. for visualizing the core genome) pairs of the coverage/quorum parameters can be used:

- `coverage`: include only features in the calculation that are visited by at least that many paths (can be used e.g. to filter out private nodes, that are part of only 1 haplotype)
- `quorum`: fraction of haplotypes that must share a feature after the haplotype is added to the graph to include it in the output (e.g. a quorum of `1` means only features that are shared by `100%` of the haplotypes ("core genome"))

## Installation
`panacus` is written in [RUST](https://www.rust-lang.org/) and requires a working RUST build system (version >= 1.74.1) for installation. See [here](https://www.rust-lang.org/tools/install) for more details.

`panacus` provides a Python script for visualizing the calculated counting statistics. It requires Python>=3.6 and the following Python libraries:
- matplotlib
- numpy
- pandas
- scikit-learn
- scipy
- seaborn

### From bioconda channel

Make sure you have [conda](https://conda.io)/[mamba](https://anaconda.org/conda-forge/mamba) installed!

```shell
mamba install -c conda-forge -c bioconda panacus
```

### From binary release
#### Linux x86\_64
```shell
wget --no-check-certificate -c https://github.com/marschall-lab/panacus/releases/download/0.2.6/panacus-0.2.6_x86_64-unknown-linux-musl.tar.gz
tar -xzvf panacus-0.2.6_x86_64-unknown-linux-musl.tar.gz

# install the Python libraries necessary for panacus-visualize
pip install --user matplotlib numpy pandas scikit-learn scipy seaborn

# suggestion: add tool to path in your ~/.bashrc
export PATH="$(readlink -f panacus-0.2.6_x86_64-unknown-linux-musl/bin)":$PATH

# you are ready to go!
panacus --help
```

#### Mac OSX arm64
```shell
wget --no-check-certificate -c https://github.com/marschall-lab/panacus/releases/download/0.2.6/panacus-0.2.6_aarch64-apple-darwin.tar.gz
tar -xzvf panacus-0.2.6_aarch64-apple-darwin.tar.gz

# install the Python libraries necessary for panacus-visualize
pip install --user matplotlib numpy pandas scikit-learn scipy seaborn

# suggestion: add tool to path in your ~/.bashrc
export PATH="$(readlink -f panacus-0.2.6_aarch64-apple-darwin/bin)":$PATH

# you are ready to go!
panacus --help
```

### From repository
```shell
git clone git@github.com:marschall-lab/panacus.git

cd panacus
cargo build --release

mkdir bin
ln -s ../target/release/panacus bin/
ln -s ../scripts/panacus-visualize.py bin/panacus-visualize

# install the Python libraries necessary for panacus-visualize
pip install --user matplotlib numpy pandas scikit-learn scipy seaborn

# suggestion: add tool to path in your ~/.bashrc
export PATH="$(readlink -f bin)":$PATH

# you are ready to go!
panacus --help

```

## Run

```console
$ panacus
Calculate count statistics for pangenomic data

Usage: panacus <COMMAND>

Commands:
  info                Return general graph and paths info
  histgrowth          Run hist and growth. Return the growth curve
  hist                Calculate coverage histogram
  growth              Calculate growth curve from coverage histogram
  ordered-histgrowth  Calculate growth curve based on group file order (if order is unspecified, use path order in GFA)
  table               Compute coverage table for count type
  help                Print this message or the help of the given subcommand(s)

Options:
  -h, --help     Print help
  -V, --version  Print version
```

## Quickstart
Generate a simple growth plot from a GFA file:
```shell
RUST_LOG=info panacus histgrowth -t6 -q 0.1,0.5,1 -S <INPUT_GFA> > output.tsv
panacus-visualize -e output.tsv > output.pdf
```

## Examples
Examples can be found in the [examples directory](/examples/).

## Citation
Parmigiani, L., Garrison, E., Stoye, J., Marschall, T. & Doerr, D. Panacus: fast and exact pangenome growth and core size estimation. Bionformatics, https://doi.org/10.1093/bioinformatics/btae720 (2024).
