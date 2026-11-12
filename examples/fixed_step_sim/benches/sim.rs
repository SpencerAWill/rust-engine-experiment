// benchmark harness, not simulation code
#![forbid(unsafe_code)]
#![allow(
    missing_docs,
    clippy::disallowed_types,
    clippy::disallowed_methods,
    clippy::unwrap_used
)]
//! The full default configuration: 10,000 particles for 600 ticks, jittery frame times and all.

use std::hint::black_box;

use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use fixed_step_sim::{SimConfig, run};

fn bench_sim(c: &mut Criterion) {
    let config = SimConfig::default();
    let mut group = c.benchmark_group("fixed_step_sim");
    group.throughput(Throughput::Elements(config.ticks * config.particles as u64));
    group.sample_size(20);
    group.bench_function("default_config", |b| b.iter(|| black_box(run(&config))));
}

criterion_group!(benches, bench_sim);
criterion_main!(benches);
