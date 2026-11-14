// benchmark harness, not simulation code
#![forbid(unsafe_code)]
#![allow(
    missing_docs,
    clippy::disallowed_types,
    clippy::disallowed_methods,
    clippy::unwrap_used
)]
//! `engine_random::Prng` hot paths, counted in instructions.
//!
//! Every input comes from a fixed seed, so the same commit always measures the same work. The
//! operation runs in a separate `*_work` function that is never inlined, for the reason given in
//! `handle.rs`.

use core::hint::black_box;

use engine_random::Prng;
use gungraun::{library_benchmark, library_benchmark_group, main};

const SEED: u64 = 0x5EED;

fn seeded() -> Prng {
    Prng::from_seed(SEED)
}

fn seeded_with_slice() -> (Prng, Vec<u32>) {
    (seeded(), (0..1_000).collect())
}

fn seeds() -> Vec<u64> {
    let mut prng = seeded();
    (0..1_000).map(|_| prng.next_u64()).collect()
}

#[inline(never)]
fn next_u64_10k_work(mut prng: Prng) -> u64 {
    let mut sum = 0_u64;
    for _ in 0..10_000 {
        sum = sum.wrapping_add(prng.next_u64());
    }
    sum
}

#[library_benchmark]
#[bench::fixed(setup = seeded)]
fn next_u64_10k(prng: Prng) -> u64 {
    black_box(next_u64_10k_work(black_box(prng)))
}

#[inline(never)]
fn range_u32_10k_work(mut prng: Prng) -> u32 {
    let mut sum = 0_u32;
    for _ in 0..10_000 {
        sum = sum.wrapping_add(prng.range(0..1000_u32));
    }
    sum
}

#[library_benchmark]
#[bench::fixed(setup = seeded)]
fn range_u32_10k(prng: Prng) -> u32 {
    black_box(range_u32_10k_work(black_box(prng)))
}

#[inline(never)]
fn range_f64_10k_work(mut prng: Prng) -> f64 {
    let mut sum = 0.0_f64;
    for _ in 0..10_000 {
        sum += prng.range(0.0..1.0_f64);
    }
    sum
}

#[library_benchmark]
#[bench::fixed(setup = seeded)]
fn range_f64_10k(prng: Prng) -> f64 {
    black_box(range_f64_10k_work(black_box(prng)))
}

#[inline(never)]
fn shuffle_1k_work((mut prng, mut items): (Prng, Vec<u32>)) -> Vec<u32> {
    prng.shuffle(&mut items);
    items
}

#[library_benchmark]
#[bench::fixed(setup = seeded_with_slice)]
fn shuffle_1k(input: (Prng, Vec<u32>)) -> Vec<u32> {
    black_box(shuffle_1k_work(black_box(input)))
}

#[inline(never)]
fn from_seed_1k_work(seeds: Vec<u64>) -> u64 {
    let mut sum = 0_u64;
    for seed in seeds {
        sum = sum.wrapping_add(Prng::from_seed(seed).next_u64());
    }
    sum
}

#[library_benchmark]
#[bench::fixed(setup = seeds)]
fn from_seed_1k(seeds: Vec<u64>) -> u64 {
    black_box(from_seed_1k_work(black_box(seeds)))
}

library_benchmark_group!(
    name = prng;
    benchmarks = next_u64_10k, range_u32_10k, range_f64_10k, shuffle_1k, from_seed_1k
);

main!(library_benchmark_groups = prng);
