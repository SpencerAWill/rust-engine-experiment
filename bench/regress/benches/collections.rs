// benchmark harness, not simulation code
#![forbid(unsafe_code)]
#![allow(
    missing_docs,
    clippy::disallowed_types,
    clippy::disallowed_methods,
    clippy::unwrap_used,
    clippy::cast_possible_truncation
)]
//! `engine_collections::IndexMap` hot paths, counted in instructions.
//!
//! Every input comes from a fixed seed, so the same commit always measures the same work. Each
//! benchmark builds its input in a `setup` function, which Gungraun does not count, and runs the
//! operation in a separate `*_work` function that is never inlined (see `handle.rs` for why).

use core::any::TypeId;
use core::hint::black_box;

use engine_collections::IndexMap;
use engine_random::Prng;
use gungraun::{library_benchmark, library_benchmark_group, main};

const N: usize = 1_024;
const SEED: u64 = 0x5EED;

type Map = IndexMap<u64, u64>;

/// Shuffles with the fixed seed, so every benchmark sees the same order.
fn shuffled<T>(mut items: Vec<T>) -> Vec<T> {
    Prng::from_seed(SEED).shuffle(&mut items);
    items
}

/// 1,024 distinct keys in shuffled order.
fn keys() -> Vec<u64> {
    shuffled((0..N as u64).collect())
}

/// 1,024 distinct keys that are never in the map, in shuffled order.
fn missing_keys() -> Vec<u64> {
    shuffled((N as u64..2 * N as u64).collect())
}

fn empty_map() -> (Map, Vec<u64>) {
    (IndexMap::with_capacity(N), keys())
}

fn filled_map() -> (Map, Vec<u64>) {
    let (mut map, keys) = empty_map();
    for &k in &keys {
        map.insert(k, k);
    }
    (map, keys)
}

fn filled_map_with_missing_keys() -> (Map, Vec<u64>) {
    let (map, _) = filled_map();
    (map, missing_keys())
}

#[inline(never)]
fn map_insert_1k_work((mut map, keys): (Map, Vec<u64>)) -> Map {
    for &k in &keys {
        black_box(map.insert(k, k));
    }
    map
}

#[library_benchmark]
#[bench::fixed(setup = empty_map)]
fn map_insert_1k(input: (Map, Vec<u64>)) -> Map {
    black_box(map_insert_1k_work(black_box(input)))
}

#[inline(never)]
fn map_get_hit_1k_work((map, keys): (Map, Vec<u64>)) -> u64 {
    let mut sum = 0_u64;
    for k in &keys {
        sum = sum.wrapping_add(*map.get(k).unwrap());
    }
    sum
}

#[library_benchmark]
#[bench::fixed(setup = filled_map)]
fn map_get_hit_1k(input: (Map, Vec<u64>)) -> u64 {
    black_box(map_get_hit_1k_work(black_box(input)))
}

#[inline(never)]
fn map_get_miss_1k_work((map, keys): (Map, Vec<u64>)) -> usize {
    let mut misses = 0;
    for k in &keys {
        misses += usize::from(map.get(k).is_none());
    }
    misses
}

#[library_benchmark]
#[bench::fixed(setup = filled_map_with_missing_keys)]
fn map_get_miss_1k(input: (Map, Vec<u64>)) -> usize {
    black_box(map_get_miss_1k_work(black_box(input)))
}

/// 10,240 entries: iterating 1,024 is under the 10,000-instruction minimum (M2.4 spec §4.2).
fn filled_map_10k() -> Map {
    let mut map = IndexMap::with_capacity(10 * N);
    for k in shuffled((0..10 * N as u64).collect()) {
        map.insert(k, k);
    }
    map
}

#[inline(never)]
fn map_iter_10k_work(map: &Map) -> u64 {
    let mut sum = 0_u64;
    for (_, v) in map {
        sum = sum.wrapping_add(*v);
    }
    sum
}

#[library_benchmark]
#[bench::fixed(setup = filled_map_10k)]
fn map_iter_10k(map: Map) -> u64 {
    black_box(map_iter_10k_work(black_box(&map)))
}

/// A distinct, zero-sized marker type per `N`, so each has its own `TypeId`.
struct Tag<const N: usize>;

macro_rules! type_ids {
    ($($n:literal)*) => { [$(TypeId::of::<Tag<$n>>()),*] };
}

fn tag_ids() -> [TypeId; 64] {
    type_ids!(
        0 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 19 20 21 22 23 24 25 26 27 28 29 30 31
        32 33 34 35 36 37 38 39 40 41 42 43 44 45 46 47 48 49 50 51 52 53 54 55 56 57 58 59 60
        61 62 63
    )
}

fn typeid_map() -> (IndexMap<TypeId, u64>, Vec<TypeId>) {
    let ids = shuffled(tag_ids().to_vec());
    let mut map = IndexMap::with_capacity(64);
    for (i, id) in ids.iter().enumerate() {
        map.insert(*id, i as u64);
    }
    (map, shuffled(ids))
}

#[inline(never)]
fn map_get_typeid_64_x8_work((map, ids): (IndexMap<TypeId, u64>, Vec<TypeId>)) -> u64 {
    let mut sum = 0_u64;
    // Eight rounds: one pass over 64 lookups is under the 10,000-instruction minimum.
    for _ in 0..8 {
        for id in &ids {
            sum = sum.wrapping_add(*map.get(id).unwrap());
        }
    }
    sum
}

#[library_benchmark]
#[bench::fixed(setup = typeid_map)]
fn map_get_typeid_64_x8(input: (IndexMap<TypeId, u64>, Vec<TypeId>)) -> u64 {
    black_box(map_get_typeid_64_x8_work(black_box(input)))
}

library_benchmark_group!(
    name = map;
    benchmarks =
        map_insert_1k,
        map_get_hit_1k,
        map_get_miss_1k,
        map_iter_10k,
        map_get_typeid_64_x8
);

main!(library_benchmark_groups = map);
