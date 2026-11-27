// benchmark harness, not simulation code
#![forbid(unsafe_code)]
#![allow(missing_docs)]
//! Instruction counts and simulated cache misses of every workload, size and backend
//! (M3.2 spec §4.6).
//!
//! Callgrind runs with its cache simulator on, so Gungraun reports instructions, L1 hits, LL hits,
//! RAM hits and estimated cycles for each benchmark. Run it by hand in the devcontainer or on any
//! Linux machine with Valgrind and `gungraun-runner`: `cargo bench -p spike_ecs_bench_cache`. It
//! is not a gate, and `bench-regress` does not look at it.
//!
//! To add a backend, add one `cache_bench!` line per workload it implements and name the function
//! in that workload's group.
//!
//! Each benchmark's `setup` runs the workload's `setup` and one untimed `step`, so buffers have
//! reached their steady size. The measured function is one `step`, behind a function that is never
//! inlined and is defined outside the benchmark function (see `bench/regress/benches/genid.rs`
//! for why).

use core::hint::black_box;

use gungraun::{
    Callgrind, LibraryBenchmarkConfig, library_benchmark, library_benchmark_group, main,
};
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
};
use spike_ecs_bevy::{
    add_remove::BevyAddRemove, add_remove::BevySparseAddRemove, agents::BevyAgents,
    agents::BevySparseAgents, entity_churn::BevyEntityChurn, frag_iter::BevyFragIter,
    heavy_compute::BevyHeavyCompute, many_queries::BevyManyQueries,
    random_access::BevyRandomAccess, simple_iter::BevySimpleIter, sprites::BevySprites,
};
use spike_ecs_dense::{
    add_remove::DenseAddRemove, agents::DenseAgents, entity_churn::DenseEntityChurn,
    frag_iter::DenseFragIter, heavy_compute::DenseHeavyCompute, many_queries::DenseManyQueries,
    random_access::DenseRandomAccess, simple_iter::DenseSimpleIter, sprites::DenseSprites,
};
use spike_ecs_harness::{Params, Spec, Workload};
use spike_ecs_hecs::{
    add_remove::HecsAddRemove, agents::HecsAgents, entity_churn::HecsEntityChurn,
    frag_iter::HecsFragIter, heavy_compute::HecsHeavyCompute, many_queries::HecsManyQueries,
    random_access::HecsRandomAccess, simple_iter::HecsSimpleIter, sprites::HecsSprites,
};
use spike_ecs_hybrid::{
    add_remove::HybridAddRemove, agents::HybridAgents, entity_churn::HybridEntityChurn,
    frag_iter::HybridFragIter, heavy_compute::HybridHeavyCompute, many_queries::HybridManyQueries,
    random_access::HybridRandomAccess, simple_iter::HybridSimpleIter, sprites::HybridSprites,
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
};
use spike_ecs_vec::agents::{SoaAgents, VecAgents};
use spike_ecs_vec::many_queries::VecManyQueries;
use spike_ecs_vec::random_access::VecRandomAccess;
use spike_ecs_vec::{
    add_remove::VecAddRemove, entity_churn::VecEntityChurn, frag_iter::VecFragIter,
    heavy_compute::VecHeavyCompute, simple_iter::VecSimpleIter, sprites::VecSprites,
};

use spike_ecs_archetype::wide_entities::ArchetypeWideEntities;
use spike_ecs_bevy::wide_entities::BevyWideEntities;
use spike_ecs_dense::wide_entities::DenseWideEntities;
use spike_ecs_hecs::wide_entities::HecsWideEntities;
use spike_ecs_hybrid::wide_entities::HybridWideEntities;
use spike_ecs_sparse_set::wide_entities::SparseSetWideEntities;
use spike_ecs_vec::wide_entities::VecWideEntities;

/// The seed of every benchmark.
const SEED: u64 = 42;

/// Sets up `W` with `count` entities and runs one step, so the measured step is a steady one.
///
/// `count` must be one of the workload's `Spec::SIZES`: the sizes are written out in each
/// `cache_bench!` line, because Gungraun names a benchmark after its `args` text and an `iter`
/// expression would make the names unreadable. The assertion keeps the two lists in step.
fn prepared<W: Workload>(count: usize) -> W {
    assert!(
        <W::Spec as Spec>::SIZES.contains(&count),
        "{count} is not a size of {}",
        <W::Spec as Spec>::NAME
    );
    let mut workload = W::setup(Params { count, seed: SEED });
    workload.step();
    workload
}

/// The measured unit.
#[inline(never)]
fn step_once<W: Workload>(workload: &mut W) {
    workload.step();
}

/// One library benchmark for a (workload, backend) type, over the given sizes. It returns the
/// workload so that dropping it is not counted.
macro_rules! cache_bench {
    ($name:ident, $ty:ty, [$($size:literal),+]) => {
        #[library_benchmark]
        #[benches::sizes(args = [$($size),+], setup = prepared::<$ty>)]
        fn $name(mut workload: $ty) -> $ty {
            step_once(black_box(&mut workload));
            workload
        }
    };
}

cache_bench!(simple_iter_vec_aos, VecSimpleIter, [1_000, 10_000, 100_000]);
cache_bench!(frag_iter_vec_aos, VecFragIter, [20]);
cache_bench!(heavy_compute_vec_aos, VecHeavyCompute, [1_000]);
cache_bench!(add_remove_vec_aos, VecAddRemove, [1_000, 10_000, 100_000]);
cache_bench!(
    entity_churn_vec_aos,
    VecEntityChurn,
    [1_000, 10_000, 100_000]
);
cache_bench!(sprites_vec_aos, VecSprites, [100_000]);
cache_bench!(agents_vec_aos, VecAgents, [10_000, 100_000]);
cache_bench!(agents_vec_soa, SoaAgents, [10_000, 100_000]);
cache_bench!(
    random_access_vec_aos,
    VecRandomAccess,
    [10_000, 100_000, 1_000_000]
);
cache_bench!(
    simple_iter_sparse_set,
    SparseSetSimpleIter,
    [1_000, 10_000, 100_000]
);
cache_bench!(
    simple_iter_sparse_set_grouped,
    GroupedSimpleIter,
    [1_000, 10_000, 100_000]
);
cache_bench!(frag_iter_sparse_set, SparseSetFragIter, [20]);
cache_bench!(heavy_compute_sparse_set, SparseSetHeavyCompute, [1_000]);
cache_bench!(
    heavy_compute_sparse_set_grouped,
    GroupedHeavyCompute,
    [1_000]
);
cache_bench!(
    add_remove_sparse_set,
    SparseSetAddRemove,
    [1_000, 10_000, 100_000]
);
cache_bench!(
    add_remove_sparse_set_grouped,
    GroupedAddRemove,
    [1_000, 10_000, 100_000]
);
cache_bench!(
    entity_churn_sparse_set,
    SparseSetEntityChurn,
    [1_000, 10_000, 100_000]
);
cache_bench!(
    entity_churn_sparse_set_grouped,
    GroupedEntityChurn,
    [1_000, 10_000, 100_000]
);
cache_bench!(sprites_sparse_set, SparseSetSprites, [100_000]);
cache_bench!(sprites_sparse_set_grouped, GroupedSprites, [100_000]);
cache_bench!(agents_sparse_set, SparseSetAgents, [10_000, 100_000]);
cache_bench!(agents_sparse_set_grouped, GroupedAgents, [10_000, 100_000]);
cache_bench!(
    random_access_sparse_set,
    SparseSetRandomAccess,
    [10_000, 100_000, 1_000_000]
);
cache_bench!(
    simple_iter_archetype,
    ArchetypeSimpleIter,
    [1_000, 10_000, 100_000]
);
cache_bench!(frag_iter_archetype, ArchetypeFragIter, [20]);
cache_bench!(heavy_compute_archetype, ArchetypeHeavyCompute, [1_000]);
cache_bench!(
    add_remove_archetype,
    ArchetypeAddRemove,
    [1_000, 10_000, 100_000]
);
cache_bench!(
    entity_churn_archetype,
    ArchetypeEntityChurn,
    [1_000, 10_000, 100_000]
);
cache_bench!(sprites_archetype, ArchetypeSprites, [100_000]);
cache_bench!(agents_archetype, ArchetypeAgents, [10_000, 100_000]);
cache_bench!(
    random_access_archetype,
    ArchetypeRandomAccess,
    [10_000, 100_000, 1_000_000]
);
cache_bench!(
    simple_iter_archetype_bytes_copy,
    BytesCopySimpleIter,
    [1_000, 10_000, 100_000]
);
cache_bench!(
    simple_iter_archetype_bytes_cast,
    BytesCastSimpleIter,
    [1_000, 10_000, 100_000]
);
cache_bench!(frag_iter_archetype_bytes_copy, BytesCopyFragIter, [20]);
cache_bench!(frag_iter_archetype_bytes_cast, BytesCastFragIter, [20]);
cache_bench!(
    add_remove_archetype_bytes_copy,
    BytesCopyAddRemove,
    [1_000, 10_000, 100_000]
);
cache_bench!(
    add_remove_archetype_bytes_cast,
    BytesCastAddRemove,
    [1_000, 10_000, 100_000]
);
cache_bench!(simple_iter_dense, DenseSimpleIter, [1_000, 10_000, 100_000]);
cache_bench!(frag_iter_dense, DenseFragIter, [20]);
cache_bench!(heavy_compute_dense, DenseHeavyCompute, [1_000]);
cache_bench!(add_remove_dense, DenseAddRemove, [1_000, 10_000, 100_000]);
cache_bench!(
    entity_churn_dense,
    DenseEntityChurn,
    [1_000, 10_000, 100_000]
);
cache_bench!(sprites_dense, DenseSprites, [100_000]);
cache_bench!(agents_dense, DenseAgents, [10_000, 100_000]);
cache_bench!(
    random_access_dense,
    DenseRandomAccess,
    [10_000, 100_000, 1_000_000]
);
cache_bench!(
    simple_iter_hybrid,
    HybridSimpleIter,
    [1_000, 10_000, 100_000]
);
cache_bench!(frag_iter_hybrid, HybridFragIter, [20]);
cache_bench!(heavy_compute_hybrid, HybridHeavyCompute, [1_000]);
cache_bench!(add_remove_hybrid, HybridAddRemove, [1_000, 10_000, 100_000]);
cache_bench!(
    entity_churn_hybrid,
    HybridEntityChurn,
    [1_000, 10_000, 100_000]
);
cache_bench!(sprites_hybrid, HybridSprites, [100_000]);
cache_bench!(agents_hybrid, HybridAgents, [10_000, 100_000]);
cache_bench!(
    random_access_hybrid,
    HybridRandomAccess,
    [10_000, 100_000, 1_000_000]
);
cache_bench!(
    simple_iter_bevy_ecs,
    BevySimpleIter,
    [1_000, 10_000, 100_000]
);
cache_bench!(frag_iter_bevy_ecs, BevyFragIter, [20]);
cache_bench!(heavy_compute_bevy_ecs, BevyHeavyCompute, [1_000]);
cache_bench!(add_remove_bevy_ecs, BevyAddRemove, [1_000, 10_000, 100_000]);
cache_bench!(
    entity_churn_bevy_ecs,
    BevyEntityChurn,
    [1_000, 10_000, 100_000]
);
cache_bench!(sprites_bevy_ecs, BevySprites, [100_000]);
cache_bench!(agents_bevy_ecs, BevyAgents, [10_000, 100_000]);
cache_bench!(
    add_remove_bevy_ecs_sparse,
    BevySparseAddRemove,
    [1_000, 10_000, 100_000]
);
cache_bench!(agents_bevy_ecs_sparse, BevySparseAgents, [10_000, 100_000]);
cache_bench!(
    random_access_bevy_ecs,
    BevyRandomAccess,
    [10_000, 100_000, 1_000_000]
);
cache_bench!(simple_iter_hecs, HecsSimpleIter, [1_000, 10_000, 100_000]);
cache_bench!(frag_iter_hecs, HecsFragIter, [20]);
cache_bench!(heavy_compute_hecs, HecsHeavyCompute, [1_000]);
cache_bench!(add_remove_hecs, HecsAddRemove, [1_000, 10_000, 100_000]);
cache_bench!(entity_churn_hecs, HecsEntityChurn, [1_000, 10_000, 100_000]);
cache_bench!(sprites_hecs, HecsSprites, [100_000]);
cache_bench!(agents_hecs, HecsAgents, [10_000, 100_000]);
cache_bench!(
    random_access_hecs,
    HecsRandomAccess,
    [10_000, 100_000, 1_000_000]
);
cache_bench!(many_queries_vec_aos, VecManyQueries, [100, 1_000, 10_000]);
cache_bench!(
    many_queries_sparse_set,
    SparseSetManyQueries,
    [100, 1_000, 10_000]
);
cache_bench!(
    many_queries_archetype,
    ArchetypeManyQueries,
    [100, 1_000, 10_000]
);
cache_bench!(many_queries_dense, DenseManyQueries, [100, 1_000, 10_000]);
cache_bench!(many_queries_hybrid, HybridManyQueries, [100, 1_000, 10_000]);
cache_bench!(many_queries_bevy_ecs, BevyManyQueries, [100, 1_000, 10_000]);
cache_bench!(many_queries_hecs, HecsManyQueries, [100, 1_000, 10_000]);
cache_bench!(
    wide_entities_vec_aos,
    VecWideEntities,
    [1_000, 10_000, 100_000]
);
cache_bench!(
    wide_entities_sparse_set,
    SparseSetWideEntities,
    [1_000, 10_000, 100_000]
);
cache_bench!(
    wide_entities_archetype,
    ArchetypeWideEntities,
    [1_000, 10_000, 100_000]
);
cache_bench!(
    wide_entities_dense,
    DenseWideEntities,
    [1_000, 10_000, 100_000]
);
cache_bench!(
    wide_entities_hybrid,
    HybridWideEntities,
    [1_000, 10_000, 100_000]
);
cache_bench!(
    wide_entities_bevy_ecs,
    BevyWideEntities,
    [1_000, 10_000, 100_000]
);
cache_bench!(
    wide_entities_hecs,
    HecsWideEntities,
    [1_000, 10_000, 100_000]
);

library_benchmark_group!(name = simple_iter; benchmarks = simple_iter_vec_aos, simple_iter_sparse_set, simple_iter_sparse_set_grouped, simple_iter_archetype, simple_iter_archetype_bytes_copy, simple_iter_archetype_bytes_cast, simple_iter_dense, simple_iter_hybrid, simple_iter_bevy_ecs, simple_iter_hecs);
library_benchmark_group!(name = frag_iter; benchmarks = frag_iter_vec_aos, frag_iter_sparse_set, frag_iter_archetype, frag_iter_archetype_bytes_copy, frag_iter_archetype_bytes_cast, frag_iter_dense, frag_iter_hybrid, frag_iter_bevy_ecs, frag_iter_hecs);
library_benchmark_group!(name = heavy_compute; benchmarks = heavy_compute_vec_aos, heavy_compute_sparse_set, heavy_compute_sparse_set_grouped, heavy_compute_archetype, heavy_compute_dense, heavy_compute_hybrid, heavy_compute_bevy_ecs, heavy_compute_hecs);
library_benchmark_group!(name = add_remove; benchmarks = add_remove_vec_aos, add_remove_sparse_set, add_remove_sparse_set_grouped, add_remove_archetype, add_remove_archetype_bytes_copy, add_remove_archetype_bytes_cast, add_remove_dense, add_remove_hybrid, add_remove_bevy_ecs, add_remove_bevy_ecs_sparse, add_remove_hecs);
library_benchmark_group!(name = entity_churn; benchmarks = entity_churn_vec_aos, entity_churn_sparse_set, entity_churn_sparse_set_grouped, entity_churn_archetype, entity_churn_dense, entity_churn_hybrid, entity_churn_bevy_ecs, entity_churn_hecs);
library_benchmark_group!(name = sprites; benchmarks = sprites_vec_aos, sprites_sparse_set, sprites_sparse_set_grouped, sprites_archetype, sprites_dense, sprites_hybrid, sprites_bevy_ecs, sprites_hecs);
library_benchmark_group!(name = agents; benchmarks = agents_vec_aos, agents_vec_soa, agents_sparse_set, agents_sparse_set_grouped, agents_archetype, agents_dense, agents_hybrid, agents_bevy_ecs, agents_bevy_ecs_sparse, agents_hecs);
library_benchmark_group!(name = random_access; benchmarks = random_access_vec_aos, random_access_sparse_set, random_access_archetype, random_access_dense, random_access_hybrid, random_access_bevy_ecs, random_access_hecs);
library_benchmark_group!(name = many_queries; benchmarks = many_queries_vec_aos, many_queries_sparse_set, many_queries_archetype, many_queries_dense, many_queries_hybrid, many_queries_bevy_ecs, many_queries_hecs);
library_benchmark_group!(name = wide_entities; benchmarks = wide_entities_vec_aos, wide_entities_sparse_set, wide_entities_archetype, wide_entities_dense, wide_entities_hybrid, wide_entities_bevy_ecs, wide_entities_hecs);

main!(
    config = LibraryBenchmarkConfig::default().tool(Callgrind::with_args(["--cache-sim=yes"]));
    library_benchmark_groups = simple_iter, frag_iter, heavy_compute, add_remove, entity_churn,
        sprites, agents, random_access, many_queries, wide_entities
);
