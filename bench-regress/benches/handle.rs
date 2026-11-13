// benchmark harness, not simulation code
#![forbid(unsafe_code)]
#![allow(
    missing_docs,
    clippy::disallowed_types,
    clippy::disallowed_methods,
    clippy::unwrap_used,
    clippy::cast_possible_truncation
)]
//! `engine_handle` pool and allocator hot paths, counted in instructions.
//!
//! Every input comes from a fixed seed, so the same commit always measures the same work.
//! Each benchmark builds its input in a `setup` function, which Gungraun does not count, and
//! runs the operation in a separate `*_work` function that is never inlined.
//!
//! Why the separate function: Callgrind counts a benchmark between the entry and the return of
//! Gungraun's wrapper function, and the toggle matches every symbol containing
//! `__gungraun_wrapper_mod::`. A generic helper or closure defined inside the benchmark function
//! carries that path in its symbol, which turns collection off while it runs (measured: 1
//! instruction), and a loop inlined into the wrapper was also cut short (measured: 14
//! instructions for 1,000 allocations). A plain function outside the benchmark function is
//! counted in full, as a callee.

use core::hint::black_box;

use engine_handle::{DenseHandleAllocator, Handle, HandleAllocator, HandlePool};
use engine_random::Prng;
use gungraun::{library_benchmark, library_benchmark_group, main};

const N: usize = 1_000;
const SEED: u64 = 0x5EED;

/// Shuffles with the fixed seed, so every benchmark sees the same order.
fn shuffled<T>(mut items: Vec<T>) -> Vec<T> {
    Prng::from_seed(SEED).shuffle(&mut items);
    items
}

type Pool = HandlePool<u64>;
type Alloc = HandleAllocator<u64>;
type Dense = DenseHandleAllocator<u64>;

fn empty_pool() -> Pool {
    HandlePool::with_capacity(N)
}

fn filled_pool() -> (Pool, Vec<Handle<u64>>) {
    let mut pool = empty_pool();
    let handles = (0..N as u64).map(|i| pool.insert(i)).collect();
    (pool, handles)
}

/// 10,000 values: the sum over 1,000 is vectorized and falls under the 10,000-instruction floor.
fn filled_pool_10k() -> Pool {
    let mut pool = HandlePool::with_capacity(10 * N);
    for i in 0..10 * N as u64 {
        black_box(pool.insert(i));
    }
    pool
}

fn filled_pool_shuffled() -> (Pool, Vec<Handle<u64>>) {
    let (pool, handles) = filled_pool();
    (pool, shuffled(handles))
}

fn empty_allocator() -> Alloc {
    HandleAllocator::with_capacity(N)
}

fn filled_allocator() -> (Alloc, Vec<Handle<u64>>) {
    let mut allocator = empty_allocator();
    let handles = (0..N).map(|_| allocator.allocate()).collect();
    (allocator, handles)
}

fn filled_allocator_shuffled() -> (Alloc, Vec<Handle<u64>>) {
    let (allocator, handles) = filled_allocator();
    (allocator, shuffled(handles))
}

/// An allocator and 1,000 handles in shuffled order, half of them stale (already freed).
fn allocator_with_stale_half() -> (Alloc, Vec<Handle<u64>>) {
    let (mut allocator, handles) = filled_allocator();
    for handle in &handles[..N / 2] {
        allocator.free(*handle);
    }
    (allocator, shuffled(handles))
}

fn empty_dense() -> Dense {
    DenseHandleAllocator::with_capacity(N)
}

fn filled_dense_shuffled() -> (Dense, Vec<Handle<u64>>) {
    let mut allocator = empty_dense();
    let handles = (0..N).map(|_| allocator.allocate()).collect();
    (allocator, shuffled(handles))
}

#[inline(never)]
fn pool_insert_1k_work(mut pool: Pool) -> Pool {
    for i in 0..N as u64 {
        black_box(pool.insert(i));
    }
    pool
}

#[library_benchmark]
#[bench::fixed(setup = empty_pool)]
fn pool_insert_1k(pool: Pool) -> Pool {
    black_box(pool_insert_1k_work(black_box(pool)))
}

#[inline(never)]
fn pool_get_seq_1k_work((pool, handles): (Pool, Vec<Handle<u64>>)) -> u64 {
    let mut sum = 0_u64;
    for handle in &handles {
        sum = sum.wrapping_add(*pool.get(*handle).unwrap());
    }
    sum
}

#[library_benchmark]
#[bench::fixed(setup = filled_pool)]
fn pool_get_seq_1k(input: (Pool, Vec<Handle<u64>>)) -> u64 {
    black_box(pool_get_seq_1k_work(black_box(input)))
}

#[inline(never)]
fn pool_get_random_1k_work((pool, handles): (Pool, Vec<Handle<u64>>)) -> u64 {
    let mut sum = 0_u64;
    for handle in &handles {
        sum = sum.wrapping_add(*pool.get(*handle).unwrap());
    }
    sum
}

#[library_benchmark]
#[bench::fixed(setup = filled_pool_shuffled)]
fn pool_get_random_1k(input: (Pool, Vec<Handle<u64>>)) -> u64 {
    black_box(pool_get_random_1k_work(black_box(input)))
}

#[inline(never)]
fn pool_remove_1k_work((mut pool, handles): (Pool, Vec<Handle<u64>>)) -> Pool {
    for handle in handles {
        black_box(pool.remove(handle));
    }
    pool
}

#[library_benchmark]
#[bench::fixed(setup = filled_pool_shuffled)]
fn pool_remove_1k(input: (Pool, Vec<Handle<u64>>)) -> Pool {
    black_box(pool_remove_1k_work(black_box(input)))
}

#[inline(never)]
fn pool_iter_10k_work(pool: &Pool) -> u64 {
    let mut sum = 0_u64;
    for (_, value) in pool {
        sum = sum.wrapping_add(*value);
    }
    sum
}

#[library_benchmark]
#[bench::fixed(setup = filled_pool_10k)]
fn pool_iter_10k(pool: Pool) -> u64 {
    black_box(pool_iter_10k_work(black_box(&pool)))
}

#[inline(never)]
fn pool_churn_1k_work((mut pool, handles): (Pool, Vec<Handle<u64>>)) -> Pool {
    for handle in &handles[..N / 2] {
        black_box(pool.remove(*handle));
    }
    for i in 0..(N / 2) as u64 {
        black_box(pool.insert(i));
    }
    pool
}

#[library_benchmark]
#[bench::fixed(setup = filled_pool_shuffled)]
fn pool_churn_1k(input: (Pool, Vec<Handle<u64>>)) -> Pool {
    black_box(pool_churn_1k_work(black_box(input)))
}

#[inline(never)]
fn allocator_allocate_1k_work(mut allocator: Alloc) -> Alloc {
    for _ in 0..N {
        black_box(allocator.allocate());
    }
    allocator
}

#[library_benchmark]
#[bench::fixed(setup = empty_allocator)]
fn allocator_allocate_1k(allocator: Alloc) -> Alloc {
    black_box(allocator_allocate_1k_work(black_box(allocator)))
}

#[inline(never)]
fn allocator_free_realloc_1k_work((mut allocator, handles): (Alloc, Vec<Handle<u64>>)) -> Alloc {
    for handle in &handles[..N / 2] {
        black_box(allocator.free(*handle));
    }
    for _ in 0..N / 2 {
        black_box(allocator.allocate());
    }
    allocator
}

#[library_benchmark]
#[bench::fixed(setup = filled_allocator_shuffled)]
fn allocator_free_realloc_1k(input: (Alloc, Vec<Handle<u64>>)) -> Alloc {
    black_box(allocator_free_realloc_1k_work(black_box(input)))
}

#[inline(never)]
fn allocator_is_alive_1k_work((allocator, handles): (Alloc, Vec<Handle<u64>>)) -> usize {
    let mut alive = 0_usize;
    for handle in &handles {
        alive += usize::from(allocator.is_alive(*handle));
    }
    alive
}

#[library_benchmark]
#[bench::fixed(setup = allocator_with_stale_half)]
fn allocator_is_alive_1k(input: (Alloc, Vec<Handle<u64>>)) -> usize {
    black_box(allocator_is_alive_1k_work(black_box(input)))
}

#[inline(never)]
fn dense_allocate_1k_work(mut allocator: Dense) -> Dense {
    for _ in 0..N {
        black_box(allocator.allocate());
    }
    allocator
}

#[library_benchmark]
#[bench::fixed(setup = empty_dense)]
fn dense_allocate_1k(allocator: Dense) -> Dense {
    black_box(dense_allocate_1k_work(black_box(allocator)))
}

#[inline(never)]
fn dense_free_1k_work((mut allocator, handles): (Dense, Vec<Handle<u64>>)) -> Dense {
    for handle in handles {
        black_box(allocator.free(handle));
    }
    allocator
}

#[library_benchmark]
#[bench::fixed(setup = filled_dense_shuffled)]
fn dense_free_1k(input: (Dense, Vec<Handle<u64>>)) -> Dense {
    black_box(dense_free_1k_work(black_box(input)))
}

library_benchmark_group!(
    name = pool;
    benchmarks =
        pool_insert_1k,
        pool_get_seq_1k,
        pool_get_random_1k,
        pool_remove_1k,
        pool_iter_10k,
        pool_churn_1k
);

library_benchmark_group!(
    name = allocator;
    benchmarks = allocator_allocate_1k, allocator_free_realloc_1k, allocator_is_alive_1k
);

library_benchmark_group!(
    name = dense;
    benchmarks = dense_allocate_1k, dense_free_1k
);

main!(library_benchmark_groups = pool, allocator, dense);
