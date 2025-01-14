use std::collections::HashMap;
use std::fmt;
use std::time::{Duration, Instant};
use once_cell::sync::Lazy;
use std::sync::Mutex;

pub struct BenchMark {
    durations: Vec<Duration>,
}

impl BenchMark {
    fn new() -> Self {
        Self { durations: Vec::new() }
    }

    fn add_duration(&mut self, duration: Duration) {
        self.durations.push(duration);
    }

    fn mean(&self) -> f64 {
        let sum: Duration = self.durations.iter().sum();
        sum.as_secs_f64() / self.durations.len() as f64
    }

    fn median(&self) -> f64 {
        let mut times: Vec<f64> = self.durations.iter().map(|d| d.as_secs_f64()).collect();
        times.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let mid = times.len() / 2;
        if times.len() % 2 == 0 {
            (times[mid - 1] + times[mid]) / 2.0
        } else {
            times[mid]
        }
    }

    fn standard_deviation(&self) -> f64 {
        let mean = self.mean();
        let variance: f64 = self.durations
            .iter()
            .map(|d| {
                let duration = d.as_secs_f64() - mean;
                duration * duration
            })
            .sum::<f64>() / (self.durations.len() as f64);
        variance.sqrt()
    }

    fn total_duration(&self) -> f64 {
        self.durations.iter().map(|d| d.as_secs_f64()).sum()
    }
}

pub struct Bench {
    marks: HashMap<String, BenchMark>,
    starts: HashMap<String, Instant>,
}

impl Bench {
    fn new() -> Self {
        Self {
            marks: HashMap::new(),
            starts: HashMap::new(),
        }
    }

    pub fn start(name: &str) {
        let mut bench = GLOBAL_BENCH_MANAGER.lock().unwrap();
        let start = Instant::now();
        bench.starts.insert(name.to_string(), start);
    }

    pub fn end(name: &str) {
        let mut bench = GLOBAL_BENCH_MANAGER.lock().unwrap();
        if let Some(start) = bench.starts.remove(name) {
            let duration = start.elapsed();
            let entry = bench.marks.entry(name.to_string()).or_insert_with(BenchMark::new);
            entry.add_duration(duration);
        }
    }

    pub fn report() {
        let bench = GLOBAL_BENCH_MANAGER.lock().unwrap();
        //let total_time: f64 = bench.marks.iter().filter(|&(k, _)| *k != "main").map(|(_, v)| v.total_duration()).sum();
        let main_time = bench.marks.get("main").map_or(0.0, |b| b.total_duration());

        // Collect and sort benchmarks by total duration descending
        let mut benchmarks: Vec<(&String, &BenchMark)> = bench.marks.iter().collect();
        benchmarks.sort_by(|a, b| b.1.total_duration().partial_cmp(&a.1.total_duration()).unwrap());

        println!("{:<20} {:<12} {:<12} {:<12} {:<12} {:<12}", "name", "mean_s", "median_s", "sd_s", "total_s", "main_%");
        
        for (name, benchmark) in benchmarks {
            let mean = benchmark.mean();
            let median = benchmark.median();
            let std_dev = benchmark.standard_deviation();
            let total = benchmark.total_duration();
            let percent_of_total = if name == "main" { 100.0 } else { total / main_time * 100.0 };
            println!("{:<20} {:<12.6} {:<12.6} {:<12.6} {:<12.6} {:<12.2}", name, mean, median, std_dev, total, percent_of_total);
        }

        //println!("\nTotal time (excl. main): {:.6}s", total_time);
        //println!("Total time (main only): {:.6}s", main_time);
    }
}

static GLOBAL_BENCH_MANAGER: Lazy<Mutex<Bench>> = Lazy::new(|| Mutex::new(Bench::new()));
