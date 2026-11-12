// benchmark harness, not simulation code
#![forbid(unsafe_code)]
#![allow(
    missing_docs,
    clippy::disallowed_types,
    clippy::disallowed_methods,
    clippy::unwrap_used
)]
//! `FixedTimestep::accumulate` throughput: a 16 ms frame against a 1/60 s step, draining
//! the returned steps, 1,000,000 frames per iteration.

use std::hint::black_box;
use std::num::NonZeroU32;
use std::time::Duration;

use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use engine_time::FixedTimestep;

const FRAMES: u64 = 1_000_000;

fn bench_accumulate(c: &mut Criterion) {
    let mut group = c.benchmark_group("fixed_timestep");
    group.throughput(Throughput::Elements(FRAMES));
    let frame = Duration::from_millis(16);
    let mut timestep = FixedTimestep::from_hz(NonZeroU32::new(60).unwrap());
    group.bench_function("accumulate_16ms_at_60hz", |b| {
        b.iter(|| {
            let mut ticks = 0_u64;
            for _ in 0..FRAMES {
                for tick in timestep.accumulate(frame) {
                    ticks = ticks.wrapping_add(tick.get());
                }
            }
            black_box(ticks)
        });
    });
}

criterion_group!(benches, bench_accumulate);
criterion_main!(benches);
