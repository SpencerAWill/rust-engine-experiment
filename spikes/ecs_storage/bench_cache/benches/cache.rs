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
    simple_iter_sparse_set,
    SparseSetSimpleIter,
    [1_000, 10_000, 100_000]
);
cache_bench!(
    simple_iter_grouped,
    GroupedSimpleIter,
    [1_000, 10_000, 100_000]
);
cache_bench!(frag_iter_sparse_set, SparseSetFragIter, [20]);
cache_bench!(heavy_compute_sparse_set, SparseSetHeavyCompute, [1_000]);
cache_bench!(heavy_compute_grouped, GroupedHeavyCompute, [1_000]);
cache_bench!(
    add_remove_sparse_set,
    SparseSetAddRemove,
    [1_000, 10_000, 100_000]
);
cache_bench!(
    add_remove_grouped,
    GroupedAddRemove,
    [1_000, 10_000, 100_000]
);
cache_bench!(
    entity_churn_sparse_set,
    SparseSetEntityChurn,
    [1_000, 10_000, 100_000]
);
cache_bench!(
    entity_churn_grouped,
    GroupedEntityChurn,
    [1_000, 10_000, 100_000]
);
cache_bench!(sprites_sparse_set, SparseSetSprites, [100_000]);
cache_bench!(sprites_grouped, GroupedSprites, [100_000]);
cache_bench!(agents_sparse_set, SparseSetAgents, [10_000, 100_000]);
cache_bench!(agents_grouped, GroupedAgents, [10_000, 100_000]);

library_benchmark_group!(name = simple_iter; benchmarks = simple_iter_vec_aos, simple_iter_sparse_set, simple_iter_grouped);
library_benchmark_group!(name = frag_iter; benchmarks = frag_iter_vec_aos, frag_iter_sparse_set);
library_benchmark_group!(name = heavy_compute; benchmarks = heavy_compute_vec_aos, heavy_compute_sparse_set, heavy_compute_grouped);
library_benchmark_group!(name = add_remove; benchmarks = add_remove_vec_aos, add_remove_sparse_set, add_remove_grouped);
library_benchmark_group!(name = entity_churn; benchmarks = entity_churn_vec_aos, entity_churn_sparse_set, entity_churn_grouped);
library_benchmark_group!(name = sprites; benchmarks = sprites_vec_aos, sprites_sparse_set, sprites_grouped);
library_benchmark_group!(name = agents; benchmarks = agents_vec_aos, agents_vec_soa, agents_sparse_set, agents_grouped);

main!(
    config = LibraryBenchmarkConfig::default().tool(Callgrind::with_args(["--cache-sim=yes"]));
    library_benchmark_groups = simple_iter, frag_iter, heavy_compute, add_remove, entity_churn,
        sprites, agents
);
