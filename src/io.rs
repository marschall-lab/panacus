/* standard use */
use std::io::{BufRead, BufReader, BufWriter, Read, Write};
use std::io::{Error, ErrorKind};
use std::path::Path;
use std::str::{self, FromStr};

/* external use */
use flate2::read::GzDecoder;
use quick_csv::Csv;
use rayon::prelude::*;
use strum_macros::{EnumString, EnumVariantNames};

/* internal use */
use crate::abacus::*;
use crate::graph::*;
use crate::path::*;
use crate::path_parser::*;
use crate::hist::*;
use crate::html::*;
use crate::util::*;

#[derive(Debug, Clone, Copy, PartialEq, EnumString, EnumVariantNames)]
#[strum(serialize_all = "lowercase")]
pub enum OutputFormat {
    Table,
    Html,
}

pub fn bufreader_from_compressed_gfa(gfa_file: &str) -> BufReader<Box<dyn Read>> {
    log::info!("loading graph from {}", &gfa_file);
    let f = std::fs::File::open(&gfa_file).expect(&format!("Error opening gfa file {}", &gfa_file));
    let reader: Box<dyn Read> = if gfa_file.ends_with(".gz") {
        log::info!("assuming that {} is gzip compressed..", &gfa_file);
        Box::new(GzDecoder::new(f))
    } else {
        Box::new(f)
    };
    BufReader::new(reader)
}

pub fn parse_bed<R: Read>(data: &mut BufReader<R>) -> Vec<PathSegment> {
    // based on https://en.wikipedia.org/wiki/BED_(file_format)
    let mut res = Vec::new();

    let reader = Csv::from_reader(data)
        .delimiter(b'\t')
        .flexible(true)
        .has_header(false);
    let mut is_header = true;
    let mut is_full_bed = false;
    for (i, row) in reader.enumerate() {
        let row = row.unwrap();
        let mut row_it = row.bytes_columns();
        let path_name = str::from_utf8(row_it.next().unwrap()).unwrap().to_string();
        // recognize BED header
        if is_header
            && (path_name.starts_with("browser ")
                || path_name.starts_with("track ")
                || path_name.starts_with("#"))
        {
            continue;
        }
        is_header = false;
        let mut path_seg = PathSegment::from_str(&path_name);
        if let Some(start) = row_it.next() {
            if let Some(end) = row_it.next() {
                path_seg.start = usize::from_str(str::from_utf8(start).unwrap()).ok();
                path_seg.end = usize::from_str(str::from_utf8(end).unwrap()).ok();
            } else {
                panic!(
                    "erroneous input in line {}: row must have either 1, 3, or 12 columns, but has 2",
                    i
                );
            }
            if let Some(block_count_raw) = row_it.nth(6) {
                if !is_full_bed {
                    log::debug!("assuming from now (line {}) on that file is in full bed (12 columns) format", i);
                }
                let block_count =
                    usize::from_str(str::from_utf8(block_count_raw).unwrap()).unwrap();
                is_full_bed = true;
                let mut block_sizes = str::from_utf8(row_it.next().unwrap()).unwrap().split(',');
                let mut block_starts = str::from_utf8(row_it.next().unwrap()).unwrap().split(',');
                for _ in 0..block_count {
                    let size = usize::from_str(block_sizes.next().unwrap().trim()).unwrap();
                    let start = usize::from_str(block_starts.next().unwrap().trim()).unwrap();

                    let mut tmp = path_seg.clone();
                    if tmp.start.is_some() {
                        tmp.start = Some(tmp.start.unwrap() + start);
                    } else {
                        tmp.start = Some(start);
                    }
                    tmp.end = Some(start + size);
                    res.push(tmp);
                }
            }
        }
        if !is_full_bed {
            res.push(path_seg);
        }
    }

    res
}

pub fn parse_tsv<R: Read>(
    data: &mut BufReader<R>,
) -> Result<(Vec<Vec<u8>>, Vec<Vec<Vec<u8>>>), Error> {
    let mut comments = Vec::new();
    let mut table = Vec::new();
    let reader = Csv::from_reader(data)
        .delimiter(b'\t')
        .flexible(true)
        .has_header(false);

    let mut is_header = true;
    for (i, row) in reader.enumerate() {
        let row: Vec<Vec<u8>> = row
            .map_err(|_| {
                let msg = format!("unable to parse row {}", i);
                log::error!("{}", &msg);
                Error::new(ErrorKind::Other, msg)
            })?
            .bytes_columns()
            .map(|x| x.to_vec())
            .collect();
        if is_header && (b'#' == row[0][0]) {
            let mut c = row[0].to_vec();
            for e in &row[1..] {
                c.push(b'\t');
                c.extend(e);
            }
            comments.push(c);
        } else {
            is_header = false;
            table.push(row);
        }
    }
    Ok((comments, table))
}

fn transpose_table<'a>(table: &'a Vec<Vec<Vec<u8>>>) -> Vec<Vec<&'a [u8]>> {
    let n = table.first().unwrap_or(&Vec::new()).len();

    let mut res = vec![vec![&table[0][0][..]; table.len()]; n];

    for j in 0..n {
        for i in 0..table.len() {
            res[j][i] = &table[i][j][..];
        }
    }

    res
}

fn parse_column(col: &Vec<&[u8]>, offset: usize) -> Result<Vec<usize>, Error> {
    let skip_lines = 2;
    let mut res = vec![0; col.len() - skip_lines];

    for (i, e) in col[skip_lines..].iter().enumerate() {
        if let Ok(val) = usize::from_str(&str::from_utf8(e).unwrap()) {
            res[i] = val;
        } else {
            let msg = format!(
                "error in line {}: value must be integer, but is '{}'",
                i + 3 + offset,
                &str::from_utf8(e).unwrap()
            );
            log::error!("{}", &msg);
            Err(Error::new(ErrorKind::InvalidData, msg))?
        }
    }

    Ok(res)
}

pub fn parse_hists<R: Read>(
    data: &mut BufReader<R>,
) -> Result<(Vec<(CountType, Vec<usize>)>, Vec<Vec<u8>>), Error> {
    log::info!("loading coverage histogram from");
    let (comments, raw_table) = parse_tsv(data)?;
    let raw_table = transpose_table(&raw_table);
    if raw_table.len() < 4 && b"panacus" != &raw_table[0][0][..] {
        let msg = format!(
            "error in line {}: table appears not to be generated by panacus",
            comments.len()
        );
        log::error!("{}", &msg);
        return Err(Error::new(ErrorKind::InvalidData, msg));
    }

    let mut res = Vec::new();

    let index = parse_column(&raw_table[0], comments.len())?;
    let mx = index.iter().max().unwrap();
    for col in &raw_table[1..] {
        if b"hist" == &col[0] {
            let count = CountType::from_str(&str::from_utf8(&col[1]).unwrap()).or_else(|_| {
                let msg = format!(
                    "error in line {}: expected count type declaration, but got '{}'",
                    2 + comments.len(),
                    &str::from_utf8(&col[1]).unwrap()
                );
                log::error!("{}", &msg);
                Err(Error::new(ErrorKind::InvalidData, msg))
            })?;
            let mut cov = vec![0; mx + 1];
            for (i, c) in index.iter().zip(parse_column(&col, comments.len())?) {
                cov[*i] = c;
            }

            res.push((count, cov));
        }
    }

    if res.is_empty() {
        let msg = "table does not contain hist columns";
        log::error!("{}", msg);
        Err(Error::new(ErrorKind::InvalidData, msg))
    } else {
        Ok((res, comments))
    }
}

#[allow(dead_code)]
pub fn parse_threshold_file<R: Read>(data: &mut BufReader<R>) -> Result<Vec<Threshold>, Error> {
    let mut res = Vec::new();

    let reader = Csv::from_reader(data)
        .delimiter(b'\t')
        .flexible(true)
        .has_header(false);
    for (i, row) in reader.enumerate() {
        let row = row.unwrap();
        let mut row_it = row.bytes_columns();
        if let Some(col) = row_it.next() {
            let threshold_str = str::from_utf8(col).unwrap();
            if let Ok(t) = usize::from_str(threshold_str) {
                res.push(Threshold::Absolute(t));
            } else if let Ok(t) = f64::from_str(threshold_str) {
                res.push(Threshold::Relative(t));
            } else {
                return Err(Error::new(
                    ErrorKind::InvalidData,
                    &format!(
                        "threshold \"{}\" (line {}) is neither an integer nor a float",
                        &threshold_str,
                        i + 1
                    )[..],
                ));
            }
        }
    }

    Ok(res)
}

pub fn subset_path_gfa<R: Read>(
    data: &mut BufReader<R>,
    abacus: &AbacusByTotal,
    graph_aux: &GraphAuxilliary,
    flt_quorum_min: u32,
    flt_quorum_max: u32,
    flt_length_min: u32,
    flt_length_max: u32,
) {
    let mut buf = vec![];
    while data.read_until(b'\n', &mut buf).unwrap_or(0) > 0 {
        if buf[0] == b'P' {
            let mut comma: bool = false;
            let (path_seg, buf_path_seg) = parse_path_identifier(&buf);
            let sids = parse_path_seq_to_item_vec(&buf_path_seg, &graph_aux);
            //parse_path_seq_update_tables
            for i in 0..sids.len() {
                let sid = sids[i].0 as usize;
                let ori = sids[i].1;
                let counts = abacus.countable[sid];
                if counts >= flt_quorum_min
                    && counts <= flt_quorum_max
                    && graph_aux.node_lens[sid] >= flt_length_min
                    && graph_aux.node_lens[sid] <= flt_length_max
                {
                    if comma {
                        print!(",");
                    } else {
                        print!("P\t{}\t", path_seg);
                    }
                    comma = true;
                    print!("{}{}", sid, ori.to_pm() as char);
                }
            }
            if comma {
                println!("\t*");
            }
        }
        //NOT-TESTED
        //if buf[0] == b'W' {
        //    let (path_seg, buf_path_seg) = parse_walk_identifier(&buf);
        //    let sids = parse_walk_seq_to_item_vec(&buf_path_seg, &graph_aux);
        //    for sid in sids.iter() {
        //        println!("{}{}",sid.0,sid.1);
        //    }
        //}
        buf.clear();
    }
}

pub fn write_table<W: Write>(
    headers: &Vec<Vec<String>>,
    columns: &Vec<Vec<f64>>,
    out: &mut BufWriter<W>,
) -> Result<(), Error> {
    let n = headers.first().unwrap_or(&Vec::new()).len();

    for i in 0..n {
        for j in 0..headers.len() {
            if j > 0 {
                write!(out, "\t")?;
            }
            write!(out, "{:0}", headers[j][i])?;
        }
        writeln!(out, "")?;
    }
    let n = columns.first().unwrap_or(&Vec::new()).len();
    for i in 0..n {
        write!(out, "{}", i)?;
        for j in 0..columns.len() {
            write!(out, "\t{:0}", columns[j][i].floor())?;
        }
        writeln!(out, "")?;
    }

    Ok(())
}

pub fn write_ordered_table<W: Write>(
    headers: &Vec<Vec<String>>,
    columns: &Vec<Vec<f64>>,
    index: &Vec<String>,
    out: &mut BufWriter<W>,
) -> Result<(), std::io::Error> {
    let n = headers.first().unwrap_or(&Vec::new()).len();

    for i in 0..n {
        for j in 0..headers.len() {
            if j > 0 {
                write!(out, "\t")?;
            }
            write!(out, "{:0}", headers[j][i])?;
        }
        writeln!(out, "")?;
    }
    let n = columns.first().unwrap_or(&Vec::new()).len();
    for i in 1..n {
        write!(out, "{}", index[i - 1])?;
        for j in 0..columns.len() {
            write!(out, "\t{:0}", columns[j][i].floor())?;
        }
        writeln!(out, "")?;
    }

    Ok(())
}

pub fn write_hist_table<W: Write>(hists: &Vec<Hist>, out: &mut BufWriter<W>) -> Result<(), Error> {
    log::info!("reporting hist table");
    writeln!(
        out,
        "# {}",
        std::env::args().collect::<Vec<String>>().join(" ")
    )?;

    let mut header_cols = vec![vec![
        "panacus".to_string(),
        "count".to_string(),
        String::new(),
        String::new(),
    ]];
    let mut output_columns = Vec::new();
    for h in hists.iter() {
        output_columns.push(h.coverage.iter().map(|x| *x as f64).collect());
        header_cols.push(vec![
            "hist".to_string(),
            h.count.to_string(),
            String::new(),
            String::new(),
        ])
    }
    write_table(&header_cols, &output_columns, out)
}

pub fn write_histgrowth_table<W: Write>(
    hists: &Vec<Hist>,
    growths: &Vec<(CountType, Vec<Vec<f64>>)>,
    hist_aux: &HistAuxilliary,
    out: &mut BufWriter<W>,
) -> Result<(), Error> {
    writeln!(
        out,
        "# {}",
        std::env::args().collect::<Vec<String>>().join(" ")
    )?;

    let mut header_cols = vec![vec![
        "panacus".to_string(),
        "count".to_string(),
        "coverage".to_string(),
        "quorum".to_string(),
    ]];
    let mut output_columns: Vec<Vec<f64>> = Vec::new();

    for h in hists.iter() {
        output_columns.push(h.coverage.iter().map(|x| *x as f64).collect());
        header_cols.push(vec![
            "hist".to_string(),
            h.count.to_string(),
            String::new(),
            String::new(),
        ])
    }

    for (count, g) in growths {
        output_columns.extend(g.clone());
        let m = hist_aux.coverage.len();
        header_cols.extend(
            std::iter::repeat("growth")
                .take(m)
                .zip(std::iter::repeat(count).take(m))
                .zip(hist_aux.coverage.iter())
                .zip(&hist_aux.quorum)
                .map(|(((p, t), c), q)| {
                    vec![p.to_string(), t.to_string(), c.to_string(), q.to_string()]
                }),
        );
    }
    write_table(&header_cols, &output_columns, out)
}

pub fn write_stats<W: Write>(stats: Stats, out: &mut BufWriter<W>) -> Result<(), Error> {
    log::info!("reporting graph stats table");
    writeln!(
        out,
        "# {}",
        std::env::args().collect::<Vec<String>>().join(" ")
    )?;
    writeln!(out, "{}", stats)
}

pub fn write_ordered_histgrowth_table<W: Write>(
    abacus_group: &AbacusByGroup,
    hist_aux: &HistAuxilliary,
    out: &mut BufWriter<W>,
) -> Result<(), Error> {
    log::info!("reporting ordered-growth table");
    writeln!(
        out,
        "# {}",
        std::env::args().collect::<Vec<String>>().join(" ")
    )?;

    let mut output_columns: Vec<Vec<f64>> = hist_aux
        .coverage
        .par_iter()
        .zip(&hist_aux.quorum)
        .map(|(c, q)| {
            log::info!(
                "calculating ordered growth for coverage >= {} and quorum >= {}",
                &c,
                &q
            );
            abacus_group.calc_growth(&c, &q)
        })
        .collect();

    // insert empty row for 0 element
    for c in &mut output_columns {
        c.insert(0, std::f64::NAN);
    }
    let m = hist_aux.coverage.len();
    let mut header_cols = vec![vec![
        "panacus".to_string(),
        "count".to_string(),
        "coverage".to_string(),
        "quorum".to_string(),
    ]];
    header_cols.extend(
        std::iter::repeat("ordered-growth")
            .take(m)
            .zip(std::iter::repeat(abacus_group.count).take(m))
            .zip(hist_aux.coverage.iter())
            .zip(&hist_aux.quorum)
            .map(|(((p, t), c), q)| {
                vec![p.to_string(), t.to_string(), c.to_string(), q.to_string()]
            })
            .collect::<Vec<Vec<String>>>(),
    );
    write_ordered_table(&header_cols, &output_columns, &abacus_group.groups, out)
}

pub fn write_ordered_histgrowth_html<W: Write>(
    abacus_group: &AbacusByGroup,
    hist_aux: &HistAuxilliary,
    gfa_file: &str,
    count: CountType,
    stats: Option<Stats>,
    out: &mut BufWriter<W>,
) -> Result<(), Error> {
    let mut growths: Vec<Vec<f64>> = hist_aux
        .coverage
        .par_iter()
        .zip(&hist_aux.quorum)
        .map(|(c, q)| {
            log::info!(
                "calculating ordered growth for coverage >= {} and quorum >= {}",
                &c,
                &q
            );
            abacus_group.calc_growth(&c, &q)
        })
        .collect();
    // insert empty row for 0 element
    for c in &mut growths {
        c.insert(0, std::f64::NAN);
    }
    log::info!("reporting (hist-)growth table");

    Ok(write_histgrowth_html(
        &None,
        &vec![(count, growths)],
        &hist_aux,
        &Path::new(gfa_file)
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .to_string(),
        Some(&abacus_group.groups),
        stats,
        out,
    )?)
}
