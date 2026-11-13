// benchmark harness, not simulation code
#![forbid(unsafe_code)]
#![allow(
    missing_docs,
    clippy::disallowed_types,
    clippy::disallowed_methods,
    clippy::unwrap_used,
    clippy::cast_possible_truncation
)]
//! Map comparisons: `engine_collections::IndexMap` vs `indexmap` with the same hasher, `indexmap`
//! with the std `SipHash` hasher, and `BTreeMap`.
//!
//! The wrapper should be within noise of raw `indexmap` with the same hasher. Keys are `u64`,
//! shuffled with a fixed seed from `engine_random::Prng`, so every implementation sees the same
//! order. This file reports numbers; it gates nothing.

use std::collections::BTreeMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::BuildHasherDefault;
use std::hint::black_box;

use criterion::{BatchSize, BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use engine_collections::IndexMap as Engine;
use engine_random::Prng;
use foldhash::fast::FixedState;

const SIZES: [usize; 3] = [64, 1_024, 65_536];

trait Map {
    const NAME: &'static str;
    fn with_capacity(n: usize) -> Self;
    fn insert(&mut self, k: u64, v: u64);
    fn get(&self, k: u64) -> Option<u64>;
    fn sum(&self) -> u64;
}

impl Map for Engine<u64, u64> {
    const NAME: &'static str = "engine_collections";
    fn with_capacity(n: usize) -> Self {
        Engine::with_capacity(n)
    }
    fn insert(&mut self, k: u64, v: u64) {
        Engine::insert(self, k, v);
    }
    fn get(&self, k: u64) -> Option<u64> {
        Engine::get(self, &k).copied()
    }
    fn sum(&self) -> u64 {
        self.values().fold(0, |a, v| a.wrapping_add(*v))
    }
}

impl Map for indexmap::IndexMap<u64, u64, FixedState> {
    const NAME: &'static str = "indexmap_foldhash";
    fn with_capacity(n: usize) -> Self {
        indexmap::IndexMap::with_capacity_and_hasher(n, FixedState::with_seed(0))
    }
    fn insert(&mut self, k: u64, v: u64) {
        indexmap::IndexMap::insert(self, k, v);
    }
    fn get(&self, k: u64) -> Option<u64> {
        indexmap::IndexMap::get(self, &k).copied()
    }
    fn sum(&self) -> u64 {
        self.values().fold(0, |a, v| a.wrapping_add(*v))
    }
}

impl Map for indexmap::IndexMap<u64, u64, BuildHasherDefault<DefaultHasher>> {
    const NAME: &'static str = "indexmap_sip";
    fn with_capacity(n: usize) -> Self {
        indexmap::IndexMap::with_capacity_and_hasher(n, BuildHasherDefault::default())
    }
    fn insert(&mut self, k: u64, v: u64) {
        indexmap::IndexMap::insert(self, k, v);
    }
    fn get(&self, k: u64) -> Option<u64> {
        indexmap::IndexMap::get(self, &k).copied()
    }
    fn sum(&self) -> u64 {
        self.values().fold(0, |a, v| a.wrapping_add(*v))
    }
}

impl Map for BTreeMap<u64, u64> {
    const NAME: &'static str = "btreemap";
    fn with_capacity(_: usize) -> Self {
        BTreeMap::new()
    }
    fn insert(&mut self, k: u64, v: u64) {
        BTreeMap::insert(self, k, v);
    }
    fn get(&self, k: u64) -> Option<u64> {
        BTreeMap::get(self, &k).copied()
    }
    fn sum(&self) -> u64 {
        self.values().fold(0, |a, v| a.wrapping_add(*v))
    }
}

type Foldhash = indexmap::IndexMap<u64, u64, FixedState>;
type Sip = indexmap::IndexMap<u64, u64, BuildHasherDefault<DefaultHasher>>;

/// `n` distinct keys in a fixed shuffled order.
fn keys(n: usize, start: u64) -> Vec<u64> {
    let mut keys: Vec<u64> = (start..start + n as u64).collect();
    Prng::from_seed(0x5EED).shuffle(&mut keys);
    keys
}

fn filled<M: Map>(n: usize) -> M {
    let mut map = M::with_capacity(n);
    for k in keys(n, 0) {
        map.insert(k, k);
    }
    map
}

fn group<'a>(
    c: &'a mut Criterion,
    name: &str,
    n: usize,
) -> criterion::BenchmarkGroup<'a, criterion::measurement::WallTime> {
    let mut g = c.benchmark_group(name);
    g.throughput(Throughput::Elements(n as u64));
    g
}

fn bench_insert<M: Map>(c: &mut Criterion) {
    for n in SIZES {
        let keys = keys(n, 0);
        group(c, "map_insert", n).bench_function(BenchmarkId::new(M::NAME, n), |b| {
            b.iter_batched(
                || M::with_capacity(n),
                |mut map| {
                    for &k in &keys {
                        map.insert(k, k);
                    }
                    black_box(map)
                },
                BatchSize::LargeInput,
            );
        });
    }
}

fn bench_get<M: Map>(c: &mut Criterion, name: &str, start: u64) {
    for n in SIZES {
        let map = filled::<M>(n);
        let probes = keys(n, start);
        group(c, name, n).bench_function(BenchmarkId::new(M::NAME, n), |b| {
            b.iter(|| {
                probes
                    .iter()
                    .map(|&k| map.get(k).unwrap_or(0))
                    .fold(0_u64, u64::wrapping_add)
            });
        });
    }
}

fn bench_iter<M: Map>(c: &mut Criterion) {
    for n in SIZES {
        let map = filled::<M>(n);
        group(c, "map_iter", n)
            .bench_function(BenchmarkId::new(M::NAME, n), |b| b.iter(|| map.sum()));
    }
}

macro_rules! all_maps {
    ($c:expr, $f:ident $(, $arg:expr)*) => {
        $f::<Engine<u64, u64>>($c $(, $arg)*);
        $f::<Foldhash>($c $(, $arg)*);
        $f::<Sip>($c $(, $arg)*);
        $f::<BTreeMap<u64, u64>>($c $(, $arg)*);
    };
}

fn map_benches(c: &mut Criterion) {
    all_maps!(c, bench_insert);
    all_maps!(c, bench_get, "map_get_hit", 0);
    // Keys that are never in the map: `start` is past every inserted key.
    all_maps!(c, bench_get, "map_get_miss", 1 << 40);
    all_maps!(c, bench_iter);
}

criterion_group!(benches, map_benches);
criterion_main!(benches);
