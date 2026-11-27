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
    add_remove::ArchetypeAddRemove,
    agents::ArchetypeAgents,
    bytes_add_remove::{BytesCastAddRemove, BytesCopyAddRemove},
    bytes_frag_iter::{BytesCastFragIter, BytesCopyFragIter},
    bytes_simple_iter::{BytesCastSimpleIter, BytesCopySimpleIter},
    entity_churn::ArchetypeEntityChurn,
    frag_iter::ArchetypeFragIter,
    heavy_compute::ArchetypeHeavyCompute,
    many_queries::ArchetypeManyQueries,
    random_access::ArchetypeRandomAccess,
    simple_iter::ArchetypeSimpleIter,
    sprites::ArchetypeSprites,
    wide_entities::ArchetypeWideEntities,
};
use spike_ecs_bevy::{
    add_remove::BevyAddRemove, add_remove::BevySparseAddRemove, agents::BevyAgents,
    agents::BevySparseAgents, entity_churn::BevyEntityChurn, frag_iter::BevyFragIter,
    heavy_compute::BevyHeavyCompute, many_queries::BevyManyQueries,
    random_access::BevyRandomAccess, simple_iter::BevySimpleIter, sprites::BevySprites,
    wide_entities::BevyWideEntities,
};
use spike_ecs_dense::{
    add_remove::DenseAddRemove, agents::DenseAgents, entity_churn::DenseEntityChurn,
    frag_iter::DenseFragIter, heavy_compute::DenseHeavyCompute, many_queries::DenseManyQueries,
    random_access::DenseRandomAccess, simple_iter::DenseSimpleIter, sprites::DenseSprites,
    wide_entities::DenseWideEntities,
};
use spike_ecs_harness::{Params, Spec, Workload};
use spike_ecs_hecs::{
    add_remove::HecsAddRemove, agents::HecsAgents, entity_churn::HecsEntityChurn,
    frag_iter::HecsFragIter, heavy_compute::HecsHeavyCompute, many_queries::HecsManyQueries,
    random_access::HecsRandomAccess, simple_iter::HecsSimpleIter, sprites::HecsSprites,
    wide_entities::HecsWideEntities,
};
use spike_ecs_hybrid::{
    add_remove::HybridAddRemove, agents::HybridAgents, entity_churn::HybridEntityChurn,
    frag_iter::HybridFragIter, heavy_compute::HybridHeavyCompute, many_queries::HybridManyQueries,
    random_access::HybridRandomAccess, simple_iter::HybridSimpleIter, sprites::HybridSprites,
    wide_entities::HybridWideEntities,
};
use spike_ecs_sparse_set::{
    add_remove::{GroupedAddRemove, SparseSetAddRemove},
    agents::{GroupedAgents, SparseSetAgents},
    entity_churn::{GroupedEntityChurn, SparseSetEntityChurn},
    frag_iter::SparseSetFragIter,
    heavy_compute::{GroupedHeavyCompute, SparseSetHeavyCompute},
    many_queries::SparseSetManyQueries,
    random_access::SparseSetRandomAccess,
    simple_iter::{GroupedSimpleIter, SparseSetSimpleIter},
    sprites::{GroupedSprites, SparseSetSprites},
    wide_entities::SparseSetWideEntities,
};
use spike_ecs_vec::agents::{SoaAgents, VecAgents};
use spike_ecs_vec::many_queries::VecManyQueries;
use spike_ecs_vec::random_access::VecRandomAccess;
use spike_ecs_vec::{
    add_remove::VecAddRemove, entity_churn::VecEntityChurn, frag_iter::VecFragIter,
    heavy_compute::VecHeavyCompute, simple_iter::VecSimpleIter, sprites::VecSprites,
    wide_entities::VecWideEntities,
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
    bench::<VecRandomAccess>(c);
    bench::<VecManyQueries>(c);
    bench::<VecWideEntities>(c);
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
    bench::<SparseSetRandomAccess>(c);
    bench::<SparseSetManyQueries>(c);
    bench::<SparseSetWideEntities>(c);
    bench::<ArchetypeSimpleIter>(c);
    bench::<ArchetypeFragIter>(c);
    bench::<ArchetypeHeavyCompute>(c);
    bench::<ArchetypeAddRemove>(c);
    bench::<ArchetypeEntityChurn>(c);
    bench::<ArchetypeSprites>(c);
    bench::<ArchetypeAgents>(c);
    bench::<ArchetypeRandomAccess>(c);
    bench::<ArchetypeManyQueries>(c);
    bench::<ArchetypeWideEntities>(c);
    bench::<BytesCopySimpleIter>(c);
    bench::<BytesCastSimpleIter>(c);
    bench::<BytesCopyFragIter>(c);
    bench::<BytesCastFragIter>(c);
    bench::<BytesCopyAddRemove>(c);
    bench::<BytesCastAddRemove>(c);
    bench::<DenseSimpleIter>(c);
    bench::<DenseFragIter>(c);
    bench::<DenseHeavyCompute>(c);
    bench::<DenseAddRemove>(c);
    bench::<DenseEntityChurn>(c);
    bench::<DenseSprites>(c);
    bench::<DenseAgents>(c);
    bench::<DenseRandomAccess>(c);
    bench::<DenseManyQueries>(c);
    bench::<DenseWideEntities>(c);
    bench::<HybridSimpleIter>(c);
    bench::<HybridFragIter>(c);
    bench::<HybridHeavyCompute>(c);
    bench::<HybridAddRemove>(c);
    bench::<HybridEntityChurn>(c);
    bench::<HybridSprites>(c);
    bench::<HybridAgents>(c);
    bench::<HybridRandomAccess>(c);
    bench::<HybridManyQueries>(c);
    bench::<HybridWideEntities>(c);
    bench::<BevySimpleIter>(c);
    bench::<BevyFragIter>(c);
    bench::<BevyHeavyCompute>(c);
    bench::<BevyAddRemove>(c);
    bench::<BevyEntityChurn>(c);
    bench::<BevySprites>(c);
    bench::<BevyAgents>(c);
    bench::<BevySparseAddRemove>(c);
    bench::<BevySparseAgents>(c);
    bench::<BevyRandomAccess>(c);
    bench::<BevyManyQueries>(c);
    bench::<BevyWideEntities>(c);
    bench::<HecsSimpleIter>(c);
    bench::<HecsFragIter>(c);
    bench::<HecsHeavyCompute>(c);
    bench::<HecsAddRemove>(c);
    bench::<HecsEntityChurn>(c);
    bench::<HecsSprites>(c);
    bench::<HecsAgents>(c);
    bench::<HecsRandomAccess>(c);
    bench::<HecsManyQueries>(c);
    bench::<HecsWideEntities>(c);
}

criterion_group!(benches, compare);
criterion_main!(benches);
