// benchmark harness, not simulation code
#![forbid(unsafe_code)]
#![allow(
    missing_docs,
    clippy::disallowed_types,
    clippy::disallowed_methods,
    clippy::unwrap_used,
    clippy::cast_possible_truncation
)]
//! Map and bitset comparisons.
//!
//! Maps: `engine::collections::IndexMap` vs `indexmap` with the same hasher, `indexmap`
//! with the std `SipHash` hasher, and `BTreeMap`.
//!
//! The wrapper should be within noise of raw `indexmap` with the same hasher. Keys are `u64`,
//! shuffled with a fixed seed from `engine::random::Prng`, so every implementation sees the same
//! order. This file reports numbers; it gates nothing.
//!
//! Bitsets: `engine::collections::BitSet` vs `fixedbitset::FixedBitSet`, `BTreeSet<usize>` and
//! `Vec<bool>`, on 4,096 bits at 1%, 50% and 99% occupancy.

use std::collections::hash_map::DefaultHasher;
use std::collections::{BTreeMap, BTreeSet};
use std::hash::BuildHasherDefault;
use std::hint::black_box;

use criterion::{BatchSize, BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use engine::collections::{BitSet, IndexMap as Engine};
use engine::random::Prng;
use fixedbitset::FixedBitSet;
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
    const NAME: &'static str = "engine::IndexMap";
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

const BITS: usize = 4_096;
const OCCUPANCIES: [usize; 3] = [1, 50, 99];

trait Bits: Clone {
    const NAME: &'static str;
    fn with_capacity(bits: usize) -> Self;
    fn insert(&mut self, i: usize);
    fn contains(&self, i: usize) -> bool;
    fn sum(&self) -> usize;
    fn union_with(&mut self, other: &Self);
}

impl Bits for BitSet {
    const NAME: &'static str = "engine::BitSet";
    fn with_capacity(bits: usize) -> Self {
        BitSet::with_capacity(bits)
    }
    fn insert(&mut self, i: usize) {
        BitSet::insert(self, i);
    }
    fn contains(&self, i: usize) -> bool {
        BitSet::contains(self, i)
    }
    fn sum(&self) -> usize {
        self.iter().fold(0, usize::wrapping_add)
    }
    fn union_with(&mut self, other: &Self) {
        BitSet::union_with(self, other);
    }
}

impl Bits for FixedBitSet {
    const NAME: &'static str = "fixedbitset::FixedBitSet";
    fn with_capacity(bits: usize) -> Self {
        // It does not grow on insert, so it is made with room for every index.
        FixedBitSet::with_capacity(bits)
    }
    fn insert(&mut self, i: usize) {
        FixedBitSet::insert(self, i);
    }
    fn contains(&self, i: usize) -> bool {
        FixedBitSet::contains(self, i)
    }
    fn sum(&self) -> usize {
        self.ones().fold(0, usize::wrapping_add)
    }
    fn union_with(&mut self, other: &Self) {
        FixedBitSet::union_with(self, other);
    }
}

impl Bits for BTreeSet<usize> {
    const NAME: &'static str = "BTreeSet";
    fn with_capacity(_: usize) -> Self {
        BTreeSet::new()
    }
    fn insert(&mut self, i: usize) {
        BTreeSet::insert(self, i);
    }
    fn contains(&self, i: usize) -> bool {
        BTreeSet::contains(self, &i)
    }
    fn sum(&self) -> usize {
        self.iter().fold(0, |a, &i| a.wrapping_add(i))
    }
    fn union_with(&mut self, other: &Self) {
        self.extend(other.iter().copied());
    }
}

impl Bits for Vec<bool> {
    const NAME: &'static str = "Vec<bool>";
    fn with_capacity(bits: usize) -> Self {
        vec![false; bits]
    }
    fn insert(&mut self, i: usize) {
        self[i] = true;
    }
    fn contains(&self, i: usize) -> bool {
        self[i]
    }
    fn sum(&self) -> usize {
        self.iter()
            .enumerate()
            .filter(|(_, set)| **set)
            .fold(0, |a, (i, _)| a.wrapping_add(i))
    }
    fn union_with(&mut self, other: &Self) {
        for (a, b) in self.iter_mut().zip(other) {
            *a |= *b;
        }
    }
}

/// `percent`% of the indices below `BITS`, picked by a fixed shuffle (`salt` varies the pick).
fn members(percent: usize, salt: u64) -> Vec<usize> {
    let mut indices: Vec<usize> = (0..BITS).collect();
    Prng::from_seed(0x5EED ^ salt).shuffle(&mut indices);
    indices.truncate(BITS * percent / 100);
    indices
}

fn bits_of<B: Bits>(members: &[usize]) -> B {
    let mut set = B::with_capacity(BITS);
    for &i in members {
        set.insert(i);
    }
    set
}

fn bit_group<'a>(
    c: &'a mut Criterion,
    name: &str,
) -> criterion::BenchmarkGroup<'a, criterion::measurement::WallTime> {
    let mut g = c.benchmark_group(name);
    g.throughput(Throughput::Elements(BITS as u64));
    g
}

fn bench_bit_insert<B: Bits>(c: &mut Criterion) {
    for percent in OCCUPANCIES {
        let picked = members(percent, 0);
        bit_group(c, "bit_set_insert").bench_function(BenchmarkId::new(B::NAME, percent), |b| {
            b.iter_batched(
                || B::with_capacity(BITS),
                |mut set| {
                    for &i in &picked {
                        set.insert(i);
                    }
                    black_box(set)
                },
                BatchSize::LargeInput,
            );
        });
    }
}

fn bench_bit_contains<B: Bits>(c: &mut Criterion) {
    let probes = members(100, 1);
    for percent in OCCUPANCIES {
        let set = bits_of::<B>(&members(percent, 0));
        bit_group(c, "bit_set_contains").bench_function(BenchmarkId::new(B::NAME, percent), |b| {
            b.iter(|| probes.iter().filter(|&&i| set.contains(i)).count());
        });
    }
}

fn bench_bit_iter<B: Bits>(c: &mut Criterion) {
    for percent in OCCUPANCIES {
        let set = bits_of::<B>(&members(percent, 0));
        bit_group(c, "bit_set_iter")
            .bench_function(BenchmarkId::new(B::NAME, percent), |b| b.iter(|| set.sum()));
    }
}

fn bench_bit_union<B: Bits>(c: &mut Criterion) {
    for percent in OCCUPANCIES {
        let left = bits_of::<B>(&members(percent, 0));
        let right = bits_of::<B>(&members(percent, 2));
        bit_group(c, "bit_set_union").bench_function(BenchmarkId::new(B::NAME, percent), |b| {
            b.iter_batched(
                || left.clone(),
                |mut set| {
                    set.union_with(&right);
                    black_box(set)
                },
                BatchSize::LargeInput,
            );
        });
    }
}

macro_rules! all_bit_sets {
    ($c:expr, $f:ident) => {
        $f::<BitSet>($c);
        $f::<FixedBitSet>($c);
        $f::<BTreeSet<usize>>($c);
        $f::<Vec<bool>>($c);
    };
}

fn bit_set_benches(c: &mut Criterion) {
    all_bit_sets!(c, bench_bit_insert);
    all_bit_sets!(c, bench_bit_contains);
    all_bit_sets!(c, bench_bit_iter);
    all_bit_sets!(c, bench_bit_union);
}

criterion_group!(benches, map_benches, bit_set_benches);
criterion_main!(benches);
