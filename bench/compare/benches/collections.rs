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
//!
//! Hierarchical bitsets: `engine::collections::HierarchicalBitSet` vs `engine::collections::BitSet`,
//! `hibitset::BitSet` and `hi_sparse_bitset::BitSet<_128bit>`, on a 1,048,576-index range (see the
//! `hierarchical` section below).

use std::collections::hash_map::DefaultHasher;
use std::collections::{BTreeMap, BTreeSet};
use std::hash::BuildHasherDefault;
use std::hint::black_box;

use criterion::{BatchSize, BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use engine::collections::{BitSet, HierarchicalBitSet, IndexMap as Engine, Join};
use engine::random::Prng;
use fixedbitset::FixedBitSet;
use foldhash::fast::FixedState;
use hi_sparse_bitset::config::_128bit;
use hi_sparse_bitset::ops::{And, Sub};
use hi_sparse_bitset::{apply, reduce};
use hibitset::BitSetLike as _;

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

/// `percent`% of the indices below `BITS`, picked by a fixed shuffle (`pick` selects which shuffle).
fn members(percent: usize, pick: u64) -> Vec<usize> {
    let mut indices: Vec<usize> = (0..BITS).collect();
    Prng::from_seed(0x5EED ^ pick).shuffle(&mut indices);
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

// Hierarchical bitsets.
//
// Every case is a 1,048,576-index range at 0.1%, 1%, 50% or 99% occupancy, spread `uniform` (a
// seeded shuffle of the range) or `clustered` (runs of 256 consecutive indices, at seeded random
// run positions; the run count is the occupancy rounded to a whole number of runs, so 0.1% is
// 4 runs, 0.098%). Insert order is the generated order: shuffled for `uniform`, shuffled runs
// each ascending for `clustered`. Input sets for `join` use different seeds, so at low
// occupancy the intersection is empty and the join measures how fast a structure skips.
//
// How each subject computes "three required sets, one excluded":
// - `engine::HierarchicalBitSet`: `Join::new(&[a, b, c], &[d])`, iterated, no allocation.
// - `engine::BitSet` has no multi-way join, so (as labelled) it clones `a` into a scratch set, then
//   `intersect_with(b)`, `intersect_with(c)`, `difference_with(d)`, then iterates the scratch.
// - `hibitset::BitSet`: its lazy combinators, as its docs describe:
//   `BitSetAnd(BitSetAnd(BitSetAnd(&a, &b), &c), BitSetNot(&d))`, iterated.
// - `hi_sparse_bitset::BitSet<_128bit>`: its lazy operations, as its docs describe:
//   `apply(Sub, reduce(And, [&a, &b, &c].into_iter()), &d)`, iterated.
// Insert for `hibitset` is `add`; `hibitset` indices are `u32`.

const RANGE: usize = 1_048_576;
const RUN: usize = 256;
/// Occupancy in parts per thousand, with its label.
const DENSITIES: [(usize, &str); 4] = [(1, "0.1pct"), (10, "1pct"), (500, "50pct"), (990, "99pct")];
const PROBES: usize = 4_096;

#[derive(Clone, Copy)]
enum Spread {
    Uniform,
    Clustered,
}

impl Spread {
    const ALL: [Self; 2] = [Self::Uniform, Self::Clustered];

    fn name(self) -> &'static str {
        match self {
            Self::Uniform => "uniform",
            Self::Clustered => "clustered",
        }
    }
}

/// Members for a case, in insertion order. `pick` selects which seeded choice.
fn spread_members(spread: Spread, permille: usize, pick: u64) -> Vec<usize> {
    let target = RANGE * permille / 1_000;
    let mut prng = Prng::from_seed(0x5EED ^ pick);
    match spread {
        Spread::Uniform => {
            let mut indices: Vec<usize> = (0..RANGE).collect();
            prng.shuffle(&mut indices);
            indices.truncate(target);
            indices
        }
        Spread::Clustered => {
            let mut runs: Vec<usize> = (0..RANGE / RUN).collect();
            prng.shuffle(&mut runs);
            runs.truncate(((target + RUN / 2) / RUN).max(1));
            runs.into_iter()
                .flat_map(|run| run * RUN..(run + 1) * RUN)
                .collect()
        }
    }
}

/// Indices to probe `contains` with: a fixed random sample of the range, hits and misses alike.
fn probes() -> Vec<usize> {
    let mut indices: Vec<usize> = (0..RANGE).collect();
    Prng::from_seed(0x5EED ^ 0x63).shuffle(&mut indices);
    indices.truncate(PROBES);
    indices
}

trait Hier: Sized {
    const NAME: &'static str;
    /// The label of the join, where it is not the plain structure (`BitSet`).
    const JOIN_NAME: &'static str = Self::NAME;
    fn presized() -> Self;
    fn insert(&mut self, i: usize);
    fn contains(&self, i: usize) -> bool;
    fn sum(&self) -> usize;
    fn join_sum(required: [&Self; 3], excluded: &Self) -> usize;
}

impl Hier for HierarchicalBitSet {
    const NAME: &'static str = "engine::HierarchicalBitSet";
    fn presized() -> Self {
        HierarchicalBitSet::with_capacity(RANGE)
    }
    fn insert(&mut self, i: usize) {
        HierarchicalBitSet::insert(self, i);
    }
    fn contains(&self, i: usize) -> bool {
        HierarchicalBitSet::contains(self, i)
    }
    fn sum(&self) -> usize {
        self.iter().fold(0, usize::wrapping_add)
    }
    fn join_sum([a, b, c]: [&Self; 3], excluded: &Self) -> usize {
        Join::new(&[a, b, c], &[excluded]).fold(0, usize::wrapping_add)
    }
}

impl Hier for BitSet {
    const NAME: &'static str = "engine::BitSet";
    const JOIN_NAME: &'static str = "engine::BitSet (clone+intersect_with+difference_with)";
    fn presized() -> Self {
        BitSet::with_capacity(RANGE)
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
    fn join_sum([a, b, c]: [&Self; 3], excluded: &Self) -> usize {
        let mut scratch = a.clone();
        scratch.intersect_with(b);
        scratch.intersect_with(c);
        scratch.difference_with(excluded);
        scratch.iter().fold(0, usize::wrapping_add)
    }
}

impl Hier for hibitset::BitSet {
    const NAME: &'static str = "hibitset::BitSet";
    fn presized() -> Self {
        hibitset::BitSet::with_capacity(RANGE as u32)
    }
    fn insert(&mut self, i: usize) {
        self.add(i as u32);
    }
    fn contains(&self, i: usize) -> bool {
        hibitset::BitSet::contains(self, i as u32)
    }
    fn sum(&self) -> usize {
        self.iter().fold(0, |a, i| a.wrapping_add(i as usize))
    }
    fn join_sum([a, b, c]: [&Self; 3], excluded: &Self) -> usize {
        use hibitset::{BitSetAnd, BitSetNot};
        BitSetAnd(BitSetAnd(BitSetAnd(a, b), c), BitSetNot(excluded))
            .iter()
            .fold(0, |sum, i| sum.wrapping_add(i as usize))
    }
}

type Sparse = hi_sparse_bitset::BitSet<_128bit>;

impl Hier for Sparse {
    const NAME: &'static str = "hi_sparse_bitset::BitSet<_128bit>";
    fn presized() -> Self {
        // It allocates blocks as indices arrive and has no capacity to reserve.
        Sparse::new()
    }
    fn insert(&mut self, i: usize) {
        Sparse::insert(self, i);
    }
    fn contains(&self, i: usize) -> bool {
        hi_sparse_bitset::BitSetInterface::contains(&self, i)
    }
    fn sum(&self) -> usize {
        self.iter().fold(0, usize::wrapping_add)
    }
    fn join_sum([a, b, c]: [&Self; 3], excluded: &Self) -> usize {
        let required = reduce(And, [a, b, c].into_iter()).expect("three sets were given");
        apply(Sub, required, excluded)
            .iter()
            .fold(0, usize::wrapping_add)
    }
}

fn hier_of<H: Hier>(members: &[usize]) -> H {
    let mut set = H::presized();
    for &i in members {
        set.insert(i);
    }
    set
}

fn hier_group<'a>(
    c: &'a mut Criterion,
    name: &str,
    elements: usize,
) -> criterion::BenchmarkGroup<'a, criterion::measurement::WallTime> {
    let mut g = c.benchmark_group(name);
    g.throughput(Throughput::Elements(elements as u64));
    g
}

fn bench_hier_insert<H: Hier>(c: &mut Criterion) {
    for spread in Spread::ALL {
        for (permille, label) in DENSITIES {
            let members = spread_members(spread, permille, 0);
            let id = BenchmarkId::new(H::NAME, format!("{}_{label}", spread.name()));
            hier_group(c, "hierarchical_insert", members.len()).bench_function(id, |b| {
                b.iter_batched(
                    H::presized,
                    |mut set| {
                        for &i in &members {
                            set.insert(i);
                        }
                        black_box(set)
                    },
                    BatchSize::LargeInput,
                );
            });
        }
    }
}

fn bench_hier_contains<H: Hier>(c: &mut Criterion) {
    let probes = probes();
    for spread in Spread::ALL {
        for (permille, label) in DENSITIES {
            let set = hier_of::<H>(&spread_members(spread, permille, 0));
            let id = BenchmarkId::new(H::NAME, format!("{}_{label}", spread.name()));
            hier_group(c, "hierarchical_contains", PROBES).bench_function(id, |b| {
                b.iter(|| probes.iter().filter(|&&i| set.contains(i)).count());
            });
        }
    }
}

fn bench_hier_iter<H: Hier>(c: &mut Criterion) {
    for spread in Spread::ALL {
        for (permille, label) in DENSITIES {
            let members = spread_members(spread, permille, 0);
            let set = hier_of::<H>(&members);
            let id = BenchmarkId::new(H::NAME, format!("{}_{label}", spread.name()));
            hier_group(c, "hierarchical_iter", members.len())
                .bench_function(id, |b| b.iter(|| set.sum()));
        }
    }
}

fn bench_hier_join<H: Hier>(c: &mut Criterion) {
    for spread in Spread::ALL {
        for (permille, label) in DENSITIES {
            let [a, b, c3, d] =
                [0, 1, 2, 3].map(|pick| hier_of::<H>(&spread_members(spread, permille, pick)));
            let id = BenchmarkId::new(H::JOIN_NAME, format!("{}_{label}", spread.name()));
            // Elements: the members of one input set.
            hier_group(c, "hierarchical_join", RANGE * permille / 1_000).bench_function(
                id,
                |bench| {
                    bench.iter(|| H::join_sum([&a, &b, &c3], &d));
                },
            );
        }
    }
}

macro_rules! all_hier {
    ($c:expr, $f:ident) => {
        $f::<HierarchicalBitSet>($c);
        $f::<BitSet>($c);
        $f::<hibitset::BitSet>($c);
        $f::<Sparse>($c);
    };
}

fn hierarchical_benches(c: &mut Criterion) {
    all_hier!(c, bench_hier_insert);
    all_hier!(c, bench_hier_contains);
    all_hier!(c, bench_hier_iter);
    all_hier!(c, bench_hier_join);
}

criterion_group!(benches, map_benches, bit_set_benches, hierarchical_benches);
criterion_main!(benches);
