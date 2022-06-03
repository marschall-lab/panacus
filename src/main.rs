/* standard use */
use std::fs;
use std::io;
use std::io::prelude::*;
use std::str::{self, FromStr};

/* crate use */
use clap::Parser;
use gfa::{gfa::GFA, parser::GFAParser};
use handlegraph::handle::Handle;
use itertools::Itertools;
use quick_csv::Csv;
use rand::{seq::SliceRandom, thread_rng};
use rustc_hash::{FxHashMap, FxHashSet};

#[derive(clap::Parser, Debug)]
#[clap(
    version = "0.1",
    author = "Daniel Doerr <daniel.doerr@hhu.de>",
    about = "Calculate rarefaction statistics from pangenome graph"
)]
pub struct Command {
    #[clap(index = 1, help = "graph in GFA1 format", required = true)]
    pub graph: String,

    #[clap(
        index = 2,
        required = true,
        help = "file of samples; their order determines the cumulative count"
    )]
    pub samples: String,

    #[clap(
        short = 't',
        long = "type",
        help = "type: node or edge count",
        default_value = "nodes",
        possible_values = &["nodes", "edges", "bp"],
    )]
    pub count_type: String,

    #[clap(
        short = 'r',
        long = "permuted_repeats",
        help = "if larger 0, the haplotypes are not added in given order, but by a random permutation; the process is repeated a given number of times",
        default_value = "0"
    )]
    pub permute: usize,

    #[clap(
        short = 'f',
        long = "fix_first",
        help = "only relevant if permuted_repeats > 0; fixes the first sample (and its haplotypes) to be the first in all permutations"
    )]
    pub fix_first: bool,

    #[clap(
        short = 'm',
        long = "merge_chromosomes",
        help = "merge haplotype paths within samples whose names start with \'chr\'"
    )]
    pub merge_chr: bool,
}

fn parse_gfa<R: io::Read>(
    mut data: io::BufReader<R>,
    merge_chr: bool,
) -> FxHashMap<String, FxHashMap<String, Vec<Vec<Handle>>>> {
    let mut res: FxHashMap<String, FxHashMap<String, Vec<Vec<Handle>>>> = FxHashMap::default();
    let reader = Csv::from_reader(&mut data)
        .delimiter(b'\t')
        .flexible(true)
        .has_header(false);

    for row in reader.into_iter() {
        let row = row.unwrap();
        let mut row_it = row.bytes_columns();
        let fst_col = row_it.next().unwrap();
        if &[b'W'] == fst_col {
            let sample_id = str::from_utf8(row_it.next().unwrap()).unwrap();
            let mut hap_id = str::from_utf8(row_it.next().unwrap()).unwrap();
            if merge_chr && hap_id.to_lowercase().starts_with("chr") {
                hap_id = "chromosomes";
            }
            let seq_id = str::from_utf8(row_it.next().unwrap()).unwrap();
            let seq_start = str::from_utf8(row_it.next().unwrap()).unwrap();
            let seq_end = str::from_utf8(row_it.next().unwrap()).unwrap();
            let walk_ident = format!(
                "{}#{}#{}:{}-{}",
                sample_id, hap_id, seq_id, seq_start, seq_end
            );
            log::info!("processing walk {}", walk_ident);

            let walk_data = row_it.next().unwrap();
            let walk = parse_walk(walk_data.to_vec())
                .unwrap_or_else(|e| panic!("Unable to parse walk for {}: {}", &walk_ident, e));
            res.entry(sample_id.to_lowercase().to_string())
                .or_insert(FxHashMap::default())
                .entry(hap_id.to_string())
                .or_insert(Vec::new())
                .push(walk);
        } else if &[b'P'] == fst_col {
            let path_name = str::from_utf8(row_it.next().unwrap()).unwrap();
            let segments = path_name.split("#").collect::<Vec<&str>>();
            let sample_id =
                segments[0].to_string().split(".").collect::<Vec<&str>>()[0].to_string();
            let hap_id: String = if segments.len() > 1 {
                if merge_chr && segments[1].to_lowercase().starts_with("chr") {
                    "chromosomes".to_string()
                } else {
                    segments[1].to_string()
                }
            } else {
                "".to_string()
            };
            let path_data = row_it.next().unwrap();

            log::info!("processing path {}", path_name);
            let path = parse_path(path_data.to_vec())
                .unwrap_or_else(|e| panic!("Unable to parse walk for {}: {}", &path_name, e));
            res.entry(sample_id.to_lowercase().to_string())
                .or_insert(FxHashMap::default())
                .entry(hap_id.to_string())
                .or_insert(Vec::new())
                .push(path);
        }
    }

    res
}

fn parse_path(path_data: Vec<u8>) -> Result<Vec<Handle>, String> {
    let mut path: Vec<Handle> = Vec::new();

    let mut cur_el: Vec<u8> = Vec::new();
    for c in path_data {
        if c == b',' {
            let sid =
                usize::from_str(str::from_utf8(&cur_el[..cur_el.len() - 1]).unwrap()).unwrap();
            let is_rev = match cur_el.last().unwrap() {
                b'+' => Ok(false),
                b'-' => Ok(true),
                _ => Err(format!(
                    "unknown orientation '{}' of segment {}",
                    cur_el.last().unwrap(),
                    sid
                )),
            };
            if is_rev.is_ok() {
                path.push(Handle::pack(sid, is_rev.unwrap()));
            } else {
                return Err(is_rev.err().unwrap());
            }
            cur_el.clear();
        } else {
            cur_el.push(c);
        }
    }

    if !cur_el.is_empty() {
        let sid = usize::from_str(str::from_utf8(&cur_el[..cur_el.len() - 1]).unwrap()).unwrap();
        let is_rev = match cur_el.last().unwrap() {
            b'+' => Ok(false),
            b'-' => Ok(true),
            _ => Err(format!(
                "unknown orientation '{}' of segment {}",
                cur_el.last().unwrap(),
                sid
            )),
        };
        if is_rev.is_ok() {
            path.push(Handle::pack(sid, is_rev.unwrap()));
        } else {
            return Err(is_rev.err().unwrap());
        }
    }
    Ok(path)
}

fn parse_walk(walk_data: Vec<u8>) -> Result<Vec<Handle>, String> {
    let mut walk: Vec<Handle> = Vec::new();

    let mut cur_el: Vec<u8> = Vec::new();
    for c in walk_data {
        if (c == b'>' || c == b'<') && !cur_el.is_empty() {
            let sid = usize::from_str(str::from_utf8(&cur_el[1..]).unwrap()).unwrap();
            let is_rev = match cur_el[0] {
                b'>' => Ok(false),
                b'<' => Ok(true),
                _ => Err(format!(
                    "unknown orientation '{}' of segment {}",
                    cur_el[0], sid
                )),
            };
            if is_rev.is_ok() {
                walk.push(Handle::pack(sid, is_rev.unwrap()));
            } else {
                return Err(is_rev.err().unwrap());
            }
            cur_el.clear();
        }
        cur_el.push(c);
    }

    if !cur_el.is_empty() {
        let sid = usize::from_str(str::from_utf8(&cur_el[1..]).unwrap()).unwrap();
        let is_rev = match cur_el[0] {
            b'>' => Ok(false),
            b'<' => Ok(true),
            _ => Err(format!(
                "unknown orientation '{}' of segment {}",
                cur_el[0], sid
            )),
        };
        if is_rev.is_ok() {
            walk.push(Handle::pack(sid, is_rev.unwrap()));
        } else {
            return Err(is_rev.err().unwrap());
        }
    }
    Ok(walk)
}

fn parse_length(gfa_file: &str) -> FxHashMap<Handle, usize> {
    let mut res: FxHashMap<Handle, usize> = FxHashMap::default();

    let parser = GFAParser::new();
    let gfa: GFA<usize, ()> = parser.parse_file(gfa_file).unwrap();
    for s in gfa.segments.iter() {
        res.insert(Handle::pack(s.name, false), s.sequence.len());
    }

    res
}

fn read_samples<R: io::Read>(mut data: io::BufReader<R>) -> Vec<String> {
    let mut res = Vec::new();

    let reader = Csv::from_reader(&mut data)
        .delimiter(b'\t')
        .flexible(true)
        .has_header(false);
    for row in reader.into_iter() {
        let row = row.unwrap();
        let mut row_it = row.bytes_columns();
        res.push(str::from_utf8(row_it.next().unwrap()).unwrap().to_string());
    }

    res
}

fn cumulative_count_edges_one_haplotype(
    haplotype: &Vec<Vec<Handle>>,
    haplotype_id: usize,
    visited: &mut FxHashMap<(Handle, Handle), FxHashSet<usize>>,
) -> (usize, usize, usize) {
    let mut new = 0;

    for seq in haplotype.iter() {
        for (u, v) in seq.iter().tuple_windows() {
            let e = if (u.is_reverse() && v.is_reverse())
                || (u.is_reverse() != v.is_reverse() && u.unpack_number() > v.unpack_number())
            {
                (v.flip(), u.flip())
            } else {
                (*u, *v)
            };

            if visited.contains_key(&e) {
                visited.get_mut(&e).unwrap().insert(haplotype_id);
            } else {
                new += 1;
                let mut x = FxHashSet::default();
                x.insert(haplotype_id);
                visited.insert(e, x);
            }
        }
    }

    let major = visited
        .values()
        .map(|x| {
            if x.len() >= (haplotype_id + 1) / 2 {
                1
            } else {
                0
            }
        })
        .sum();
    let shared = visited
        .values()
        .map(|x| if x.len() == (haplotype_id + 1) { 1 } else { 0 })
        .sum();

    (new, major, shared)
}

fn cumulative_count_nodes_one_haplotype(
    haplotype: &Vec<Vec<Handle>>,
    haplotype_id: usize,
    visited: &mut FxHashMap<u64, FxHashSet<usize>>,
) -> (usize, usize, usize) {
    let mut new = 0;

    for seq in haplotype.iter() {
        for v in seq.iter() {
            let vid = v.unpack_number();
            if visited.contains_key(&vid) {
                visited.get_mut(&vid).unwrap().insert(haplotype_id);
            } else {
                new += 1;
                let mut x = FxHashSet::default();
                x.insert(haplotype_id);
                visited.insert(vid, x);
            }
        }
    }

    let major = visited
        .values()
        .map(|x| {
            if x.len() >= (haplotype_id + 1) / 2 {
                1
            } else {
                0
            }
        })
        .sum();
    let shared = visited
        .values()
        .map(|x| if x.len() == (haplotype_id + 1) { 1 } else { 0 })
        .sum();

    (new, major, shared)
}

fn cumulative_count_bp_one_haplotype(
    haplotype: &Vec<Vec<Handle>>,
    haplotype_id: usize,
    lengths: &FxHashMap<Handle, usize>,
    visited: &mut FxHashMap<u64, FxHashSet<usize>>,
) -> (usize, usize, usize) {
    let mut new = 0;

    for seq in haplotype.iter() {
        for v in seq.iter() {
            let vid = v.unpack_number();
            if visited.contains_key(&vid) {
                visited.get_mut(&vid).unwrap().insert(haplotype_id);
            } else {
                new += lengths.get(&v.forward()).unwrap();
                let mut x = FxHashSet::default();
                x.insert(haplotype_id);
                visited.insert(vid, x);
            }
        }
    }

    let major = visited
        .iter()
        .map(|(id, x)| {
            if x.len() >= (haplotype_id + 1) / 2 {
                *lengths.get(&Handle::pack(*id, false)).unwrap()
            } else {
                0
            }
        })
        .sum();
    let shared = visited
        .iter()
        .map(|(id, x)| {
            if x.len() == (haplotype_id + 1) {
                *lengths.get(&Handle::pack(*id, false)).unwrap()
            } else {
                0
            }
        })
        .sum();

    (new, major, shared)
}

fn cumulative_count_edges(
    samples: &Vec<(String, Option<String>)>,
    paths: &FxHashMap<String, FxHashMap<String, Vec<Vec<Handle>>>>,
) -> Vec<(String, String, usize, usize, usize)> {
    let mut res: Vec<(String, String, usize, usize, usize)> = Vec::new();
    let mut visited: FxHashMap<(Handle, Handle), FxHashSet<usize>> = FxHashMap::default();

    let mut new = 0;
    for (sample_id, hap_id_op) in samples.iter() {
        match paths.get(&sample_id.to_lowercase()) {
            None => {
                log::info!("sample {} not found in GFA!", sample_id);
            }
            Some(l) => {
                match hap_id_op {
                    None => {
                        for (hap_id, hap_seqs) in l.iter().sorted() {
                            log::info!(
                                "cmulative edge count of haplotype {}:{}",
                                sample_id,
                                hap_id
                            );
                            let c = cumulative_count_edges_one_haplotype(
                                &hap_seqs,
                                res.len(),
                                &mut visited,
                            );
                            new += c.0;
                            res.push((sample_id.clone(), hap_id.clone(), new, c.1, c.2));
                        }
                    }
                    Some(hap_id) => {
                        match l.get(hap_id) {
                            None => {
                                log::info!(
                                    "haplotype {} not found in sample {}!",
                                    hap_id,
                                    sample_id
                                );
                            }
                            Some(hap_seqs) => {
                                log::info!(
                                    "cmulative edge count of haplotype {}:{}",
                                    sample_id,
                                    hap_id
                                );
                                let c = cumulative_count_edges_one_haplotype(
                                    &hap_seqs,
                                    res.len(),
                                    &mut visited,
                                );
                                new += c.0;
                                res.push((sample_id.clone(), hap_id.clone(), new, c.1, c.2));
                            }
                        };
                    }
                };
            }
        };
    }

    res
}

fn cumulative_count_nodes(
    samples: &Vec<(String, Option<String>)>,
    paths: &FxHashMap<String, FxHashMap<String, Vec<Vec<Handle>>>>,
) -> Vec<(String, String, usize, usize, usize)> {
    let mut res: Vec<(String, String, usize, usize, usize)> = Vec::new();
    let mut visited: FxHashMap<u64, FxHashSet<usize>> = FxHashMap::default();

    let mut new = 0;
    for (sample_id, hap_id_op) in samples.iter() {
        match paths.get(&sample_id.to_lowercase()) {
            None => {
                log::info!("sample {} not found in GFA!", sample_id);
            }
            Some(l) => {
                match hap_id_op {
                    None => {
                        for (hap_id, hap_seqs) in l.iter().sorted() {
                            log::info!(
                                "cmulative node count of haplotype {}:{}",
                                sample_id,
                                hap_id
                            );
                            let c = cumulative_count_nodes_one_haplotype(
                                &hap_seqs,
                                res.len(),
                                &mut visited,
                            );
                            new += c.0;
                            res.push((sample_id.clone(), hap_id.clone(), new, c.1, c.2));
                        }
                    }
                    Some(hap_id) => {
                        match l.get(hap_id) {
                            None => {
                                log::info!(
                                    "haplotype {} not found in sample {}!",
                                    hap_id,
                                    sample_id
                                );
                            }
                            Some(hap_seqs) => {
                                log::info!(
                                    "cmulative node count of haplotype {}:{}",
                                    sample_id,
                                    hap_id
                                );
                                let c = cumulative_count_nodes_one_haplotype(
                                    &hap_seqs,
                                    res.len(),
                                    &mut visited,
                                );
                                new += c.0;
                                res.push((sample_id.clone(), hap_id.clone(), new, c.1, c.2));
                            }
                        };
                    }
                };
            }
        };
    }

    res
}

fn cumulative_count_bp(
    samples: &Vec<(String, Option<String>)>,
    paths: &FxHashMap<String, FxHashMap<String, Vec<Vec<Handle>>>>,
    lengths: &FxHashMap<Handle, usize>,
) -> Vec<(String, String, usize, usize, usize)> {
    let mut res: Vec<(String, String, usize, usize, usize)> = Vec::new();
    let mut visited: FxHashMap<u64, FxHashSet<usize>> = FxHashMap::default();

    let mut new = 0;
    for (sample_id, hap_id_op) in samples.iter() {
        match paths.get(&sample_id.to_lowercase()) {
            None => {
                log::info!("sample {} not found in GFA!", sample_id);
            }
            Some(l) => {
                match hap_id_op {
                    None => {
                        for (hap_id, hap_seqs) in l.iter().sorted() {
                            log::info!(
                                "cmulative bp count of haplotype {}:{}",
                                sample_id,
                                hap_id
                            );
                            let c = cumulative_count_bp_one_haplotype(
                                &hap_seqs,
                                res.len(),
                                lengths,
                                &mut visited,
                            );
                            new += c.0;
                            res.push((sample_id.clone(), hap_id.clone(), new, c.1, c.2));
                        }
                    }
                    Some(hap_id) => {
                        match l.get(hap_id) {
                            None => {
                                log::info!(
                                    "haplotype {} not found in sample {}!",
                                    hap_id,
                                    sample_id
                                );
                            }
                            Some(hap_seqs) => {
                                log::info!(
                                    "cmulative bp count of haplotype {}:{}",
                                    sample_id,
                                    hap_id
                                );
                                let c = cumulative_count_bp_one_haplotype(
                                    &hap_seqs,
                                    res.len(),
                                    lengths,
                                    &mut visited,
                                );
                                new += c.0;
                                res.push((sample_id.clone(), hap_id.clone(), new, c.1, c.2));
                            }
                        };
                    }
                };
            }
        };
    }

    res
}

fn main() -> Result<(), io::Error> {
    env_logger::init();

    // print output to stdout
    let mut out = io::BufWriter::new(std::io::stdout());

    // initialize command line parser & parse command line arguments
    let params = Command::parse();

    let data = io::BufReader::new(fs::File::open(&params.graph)?);
    log::info!("loading graph from {}", params.graph);

    let paths = parse_gfa(data, params.merge_chr);
    log::info!(
        "identified a total of {} paths in {} samples",
        paths.values().map(|x| x.len()).sum::<usize>(),
        paths.len()
    );

    let data = io::BufReader::new(fs::File::open(&params.samples)?);
    log::info!("loading samples from {}", params.samples);
    let samples = read_samples(data);

    if params.permute > 0 {
        writeln!(
            out,
            "iteration\t{}\t{}\t{}",
            vec!["cumulative_count"; params.permute].join("\t"),
            vec!["major"; params.permute].join("\t"),
            vec!["shared"; params.permute].join("\t"),
        )?;
        writeln!(
            out,
            "\t{}\t{}\t{}",
            (0..params.permute)
                .map(|x| x.to_string())
                .collect::<Vec<String>>()
                .join("\t"),
            (0..params.permute)
                .map(|x| x.to_string())
                .collect::<Vec<String>>()
                .join("\t"),
            (0..params.permute)
                .map(|x| x.to_string())
                .collect::<Vec<String>>()
                .join("\t"),
        )?;

        let mut count: Vec<Vec<(String, String, usize, usize, usize)>> = Vec::new();

        let mut rng = thread_rng();

        for l in 0..params.permute {
            let mut sam_haps: Vec<(String, Option<String>)> = Vec::new();
            if params.fix_first {
                if l == 0 {
                    log::info!(
                        "do cumulative count on {} permutations with sample ({}) being fixed at 1st position", 
                        params.permute, samples[0]);
                    sam_haps.push((samples[0].clone(), None));
                }
                for (sample_id, haps) in paths.iter() {
                    for hap_id in haps.keys() {
                        sam_haps.push((sample_id.clone(), Some(hap_id.clone())));
                    }
                }
                sam_haps[1..].shuffle(&mut rng);
            } else {
                if l == 0 {
                    log::info!("do cumulative count on {} permutations", params.permute);
                }
                for (sample_id, haps) in paths.iter() {
                    for hap_id in haps.keys() {
                        sam_haps.push((sample_id.clone(), Some(hap_id.clone())));
                    }
                }
                sam_haps.shuffle(&mut rng);
            }
            log::info!("iteration {}", l + 1);
            count.push(match &params.count_type[..] {
                "nodes" => cumulative_count_nodes(&sam_haps, &paths),
                "edges" => cumulative_count_edges(&sam_haps, &paths),
                "bp" => cumulative_count_bp(&sam_haps, &paths, &parse_length(&params.graph)),
                _ => panic!("Unknown count type {}", params.count_type),
            });
        }

        for i in 0..count[0].len() {
            let mut sample_id = format!("{}", i);
            if i == 0 && params.fix_first {
                sample_id = format!("{}#{}", count[0][0].0, count[0][0].1);
            }
            writeln!(
                out,
                "{}\t{}\t{}\t{}",
                sample_id,
                count
                    .iter()
                    .map(move |x| format!("{}", x[i].2))
                    .collect::<Vec<String>>()
                    .join("\t"),
                count
                    .iter()
                    .map(move |x| format!("{}", x[i].3))
                    .collect::<Vec<String>>()
                    .join("\t"),
                count
                    .iter()
                    .map(move |x| format!("{}", x[i].4))
                    .collect::<Vec<String>>()
                    .join("\t")
            )?;
        }
    } else {
        log::info!("do cumulative count of samples in the order given by the input file");
        writeln!(out, "sample\thaplotype\tcumulative_count\tmajor\tshared")?;
        let count = match &params.count_type[..] {
            "nodes" => {
                cumulative_count_nodes(&samples.iter().map(|x| (x.clone(), None)).collect(), &paths)
            }
            "edges" => {
                cumulative_count_edges(&samples.iter().map(|x| (x.clone(), None)).collect(), &paths)
            }
            "bp" => cumulative_count_bp(
                &samples.iter().map(|x| (x.clone(), None)).collect(),
                &paths,
                &parse_length(&params.graph),
            ),
            _ => panic!("Unknown count type {}", params.count_type),
        };

        for (sample_id, hap_id, new, major, shared) in count.iter() {
            writeln!(
                out,
                "{}\t{}\t{}\t{}\t{}",
                sample_id, hap_id, new, major, shared
            )?;
        }
    }

    out.flush()?;
    log::info!("done");
    Ok(())
}
