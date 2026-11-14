// benchmark harness, not simulation code
#![forbid(unsafe_code)]
#![allow(missing_docs)]
//! Wall-clock benchmarks of every workload, size and backend (M3.2 spec §4.6).
//!
//! The ids are `spike/ecs/<workload>/<backend>/<count>`, so backends for the same workload and
//! size sit side by side. To add a backend, add one `bench::<Type>` line per workload it
//! implements to [`compare`].

use core::time::Duration;

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use spike_ecs_harness::{Params, Spec, Workload};
use spike_ecs_vec::agents::{SoaAgents, VecAgents};
use spike_ecs_vec::{
    add_remove::VecAddRemove, entity_churn::VecEntityChurn, frag_iter::VecFragIter,
    heavy_compute::VecHeavyCompute, simple_iter::VecSimpleIter, sprites::VecSprites,
};

/// The seed of every benchmark.
const SEED: u64 = 42;

/// Registers one benchmark per size of `W`'s workload. The workload is set up once, outside the
/// timed region, and the timed routine is one `step`.
fn bench<W: Workload>(c: &mut Criterion) {
    let mut group = c.benchmark_group(format!("spike/ecs/{}", <W::Spec as Spec>::NAME));
    group
        .warm_up_time(Duration::from_secs(1))
        .measurement_time(Duration::from_secs(3));
    for &count in <W::Spec as Spec>::SIZES {
        let mut workload = W::setup(Params { count, seed: SEED });
        group.bench_function(BenchmarkId::new(W::BACKEND, count), |b| {
            b.iter(|| workload.step());
        });
    }
    group.finish();
}

fn compare(c: &mut Criterion) {
    bench::<VecSimpleIter>(c);
    bench::<VecFragIter>(c);
    bench::<VecHeavyCompute>(c);
    bench::<VecAddRemove>(c);
    bench::<VecEntityChurn>(c);
    bench::<VecSprites>(c);
    bench::<VecAgents>(c);
    bench::<SoaAgents>(c);
}

criterion_group!(benches, compare);
criterion_main!(benches);
