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
use spike_ecs_archetype::{
    add_remove::ArchetypeAddRemove, agents::ArchetypeAgents, entity_churn::ArchetypeEntityChurn,
    frag_iter::ArchetypeFragIter, heavy_compute::ArchetypeHeavyCompute,
    simple_iter::ArchetypeSimpleIter, sprites::ArchetypeSprites,
};
use spike_ecs_dense::{
    add_remove::DenseAddRemove, agents::DenseAgents, entity_churn::DenseEntityChurn,
    frag_iter::DenseFragIter, heavy_compute::DenseHeavyCompute, simple_iter::DenseSimpleIter,
    sprites::DenseSprites,
};
use spike_ecs_harness::{Params, Spec, Workload};
use spike_ecs_sparse_set::{
    add_remove::{GroupedAddRemove, SparseSetAddRemove},
    agents::{GroupedAgents, SparseSetAgents},
    entity_churn::{GroupedEntityChurn, SparseSetEntityChurn},
    frag_iter::SparseSetFragIter,
    heavy_compute::{GroupedHeavyCompute, SparseSetHeavyCompute},
    simple_iter::{GroupedSimpleIter, SparseSetSimpleIter},
    sprites::{GroupedSprites, SparseSetSprites},
};
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
    bench::<SparseSetSimpleIter>(c);
    bench::<GroupedSimpleIter>(c);
    bench::<SparseSetFragIter>(c);
    bench::<SparseSetHeavyCompute>(c);
    bench::<GroupedHeavyCompute>(c);
    bench::<SparseSetAddRemove>(c);
    bench::<GroupedAddRemove>(c);
    bench::<SparseSetEntityChurn>(c);
    bench::<GroupedEntityChurn>(c);
    bench::<SparseSetSprites>(c);
    bench::<GroupedSprites>(c);
    bench::<SparseSetAgents>(c);
    bench::<GroupedAgents>(c);
    bench::<ArchetypeSimpleIter>(c);
    bench::<ArchetypeFragIter>(c);
    bench::<ArchetypeHeavyCompute>(c);
    bench::<ArchetypeAddRemove>(c);
    bench::<ArchetypeEntityChurn>(c);
    bench::<ArchetypeSprites>(c);
    bench::<ArchetypeAgents>(c);
    bench::<DenseSimpleIter>(c);
    bench::<DenseFragIter>(c);
    bench::<DenseHeavyCompute>(c);
    bench::<DenseAddRemove>(c);
    bench::<DenseEntityChurn>(c);
    bench::<DenseSprites>(c);
    bench::<DenseAgents>(c);
}

criterion_group!(benches, compare);
criterion_main!(benches);
