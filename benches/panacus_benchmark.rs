use criterion::{criterion_group, criterion_main, Criterion};
use panacus::{
    analyses::InputRequirement,
    graph_broker::{GraphBroker, GraphMaskParameters},
};
use std::{collections::HashSet, hint::black_box};

fn benchmark_graph_broker_hist(c: &mut Criterion) {
    let mask = GraphMaskParameters::default();
    let gfa_file = "./benches/chrM.pan.fa.6626ff2.4030258.6a1ecc2.smooth.gfa";
    let input_requirements = HashSet::from([
        InputRequirement::Hist,
        InputRequirement::Node,
        InputRequirement::Bp,
        InputRequirement::Edge,
        InputRequirement::PathLens,
    ]);
    c.bench_function("graph_broker_hist", |b| {
        b.iter(|| {
            GraphBroker::from_gfa_with_view(
                black_box(gfa_file),
                black_box(input_requirements.clone()),
                black_box(&mask),
            )
        })
    });
}

criterion_group!(benches, benchmark_graph_broker_hist);
criterion_main!(benches);
