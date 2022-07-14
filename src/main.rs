/* standard use */
use std::fs;
use std::io::prelude::*;

/* crate use */
use clap::Parser;
use rustc_hash::FxHashMap;

/* private use */
mod core;
mod io;

#[derive(clap::Parser, Debug)]
#[clap(
    version = "0.2",
    author = "Daniel Doerr <daniel.doerr@hhu.de>",
    about = "Calculate growth statistics for pangenome graphs"
)]
pub struct Command {
    #[clap(index = 1, help = "graph in GFA1 format", required = true)]
    pub graph: String,

    #[clap(
        short = 't',
        long = "type",
        help = "type: node or edge count",
        default_value = "nodes",
        possible_values = &["nodes", "edges", "bp"],
    )]
    pub count_type: String,

    #[clap(
        short = 's',
        long = "subset",
        help = "produce counts by subsetting the graph to a given list of paths or path coordinates (3-column tab-separated file)",
        default_value = ""
    )]
    pub positive_list: String,

    #[clap(
        short = 'e',
        long = "exclude",
        help = "exclude bps/nodes/edges in growth count that intersect with paths (or path coordinates) provided by the given file",
        default_value = ""
    )]
    pub negative_list: String,

    #[clap(
        short = 'g',
        long = "groupby",
        help = "merge counts from paths by path-group mapping from given tab-separated file",
        default_value = ""
    )]
    pub groups: String,

    #[clap(
        short = 'c',
        long = "coverage_thresholds",
        help = "list of named coverage thresholds of the form <name1>=<threshold1>,<name2>=<threshold2> or a file that provides a name-threshold pairs in a tab-separated file",
        default_value = "0.5"
    )]
    pub thresholds: String,

    #[clap(
        short = 'a',
        long = "apriori",
        help = "identify coverage threshold groups a priori rather than during the cumulative counting"
    )]
    pub apriori: bool,

    #[clap(
        short = 'o',
        long = "ordered",
        help = "rather than computing growth across all permutations of the input, produce counts in the order of the paths in the GFA file, or, if a grouping file is specified, in the order of the provided groups"
    )]
    pub ordered: bool,
}

fn some_function<T: core::Countable>(map: FxHashMap<T, usize>) {
    let bla = "nothing";
}

fn main() -> Result<(), std::io::Error> {
    env_logger::init();

    log::debug!(
        "node ID has {} bits and mask is {:b}",
        core::BITS_NODEID,
        core::MASK_LEN
    );

    // print output to stdout
    let mut out = std::io::BufWriter::new(std::io::stdout());

    // initialize command line parser & parse command line arguments
    let params = Command::parse();

    let data = std::io::BufReader::new(fs::File::open(&params.graph)?);
    log::info!("loading graph from {}", params.graph);

    let paths = io::parse_gfa(data, false, &":".to_string());
    log::info!(
        "identified a total of {} paths in {} samples",
        paths.values().map(|x| x.len()).sum::<usize>(),
        paths.len()
    );

    let v = core::Node::new(23, 100);
    log::info!(
        "node 23, hash: {:b}, id: {}, len: {}",
        v.hash(),
        v.id(),
        v.len()
    );

    let e = core::Edge::new(23, false, 24, true);
    log::info!(
        "edge >23<24, hash: {:b}, id1: {}, id2: {}, is_reverse1: {}, is_reverse2: {}",
        e.hash(),
        e.id1(),
        e.id2(),
        e.is_reverse1(),
        e.is_reverse2()
    );

    let test: FxHashMap<core::Node, usize> = FxHashMap::default();
    some_function(test);

    out.flush()?;
    log::info!("done");
    Ok(())
}
