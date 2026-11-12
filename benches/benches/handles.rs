// benchmark harness, not simulation code
#![forbid(unsafe_code)]
#![allow(
    missing_docs,
    clippy::disallowed_types,
    clippy::disallowed_methods,
    clippy::unwrap_used,
    clippy::cast_possible_truncation
)]
//! Handle pool and allocator comparisons: `engine_handle` vs `slotmap` vs `thunderdome`.
//!
//! Every group runs over `N` in `{1_000, 100_000, 1_000_000}` and two value types
//! (`u64` and a 64-byte `[u64; 8]`). Randomness comes from `engine_random::Prng` with fixed
//! seeds, which keeps the shuffles identical across runs and across libraries.

use std::hint::black_box;
use std::time::Duration;

use criterion::{BatchSize, BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use engine_handle::{DenseHandleAllocator, Handle, HandleAllocator, HandlePool};
use engine_random::Prng;
use slotmap::{DefaultKey, DenseSlotMap, SlotMap};
use thunderdome::{Arena, Index};

const SIZES: [usize; 3] = [1_000, 100_000, 1_000_000];
/// Sizes for the scenario groups: most simulations live in the cache-resident range.
const REAL_SIZES: [usize; 4] = [100, 1_000, 10_000, 100_000];

// --- seeded randomness ---------------------------------------------------------------------

/// Deterministic shuffle with a fixed seed, so every library sees identical key orders.
fn shuffle<T>(items: &mut [T]) {
    Prng::from_seed(0x9E37_79B9_7F4A_7C15).shuffle(items);
}

// --- value types ---------------------------------------------------------------------------

trait Value: Clone + 'static {
    const NAME: &'static str;
    fn make(i: u64) -> Self;
    fn fold(&self) -> u64;
    /// A mutating per-element update (stands in for "integrate one field").
    fn bump(&mut self);
}

impl Value for u64 {
    const NAME: &'static str = "u64";
    fn make(i: u64) -> Self {
        i
    }
    fn fold(&self) -> u64 {
        *self
    }
    fn bump(&mut self) {
        *self = self.wrapping_add(1);
    }
}

impl Value for [u64; 2] {
    const NAME: &'static str = "16B";
    fn make(i: u64) -> Self {
        [i; 2]
    }
    fn fold(&self) -> u64 {
        self[0]
    }
    fn bump(&mut self) {
        self[0] = self[0].wrapping_add(1);
    }
}

impl Value for [u64; 8] {
    const NAME: &'static str = "64B";
    fn make(i: u64) -> Self {
        [i; 8]
    }
    fn fold(&self) -> u64 {
        self[0]
    }
    fn bump(&mut self) {
        self[0] = self[0].wrapping_add(1);
    }
}

// --- containers under test -------------------------------------------------------------------

trait Store<V: Value> {
    type Key: Copy;
    const NAME: &'static str;
    fn new() -> Self;
    fn insert(&mut self, v: V) -> Self::Key;
    fn get(&self, k: Self::Key) -> Option<&V>;
    fn remove(&mut self, k: Self::Key) -> Option<V>;
    fn fold_all(&self) -> u64;
    /// Extra fast path for containers that expose their values as a slice.
    fn fold_slice(&self) -> Option<u64> {
        None
    }
    /// Applies `Value::bump` to every element through the container's own iteration.
    fn update_all(&mut self);
    /// Like `update_all` through a contiguous slice; `false` if the container has none.
    fn update_slice(&mut self) -> bool {
        false
    }
}

struct Pool<V>(HandlePool<V>);
struct Slot<V>(SlotMap<DefaultKey, V>);
struct Dense<V>(DenseSlotMap<DefaultKey, V>);
struct Thunder<V>(Arena<V>);

impl<V: Value> Store<V> for Pool<V> {
    type Key = Handle<V>;
    const NAME: &'static str = "engine_handle";
    fn new() -> Self {
        Self(HandlePool::new())
    }
    fn insert(&mut self, v: V) -> Handle<V> {
        self.0.insert(v)
    }
    fn get(&self, k: Handle<V>) -> Option<&V> {
        self.0.get(k)
    }
    fn remove(&mut self, k: Handle<V>) -> Option<V> {
        self.0.remove(k)
    }
    fn fold_all(&self) -> u64 {
        self.0.iter().map(|(_, v)| v.fold()).sum()
    }
    fn fold_slice(&self) -> Option<u64> {
        Some(self.0.values().iter().map(Value::fold).sum())
    }
    fn update_all(&mut self) {
        for (_, v) in &mut self.0 {
            v.bump();
        }
    }
    fn update_slice(&mut self) -> bool {
        self.0.values_mut().iter_mut().for_each(V::bump);
        true
    }
}

impl<V: Value> Store<V> for Slot<V> {
    type Key = DefaultKey;
    const NAME: &'static str = "slotmap";
    fn new() -> Self {
        Self(SlotMap::new())
    }
    fn insert(&mut self, v: V) -> DefaultKey {
        self.0.insert(v)
    }
    fn get(&self, k: DefaultKey) -> Option<&V> {
        self.0.get(k)
    }
    fn remove(&mut self, k: DefaultKey) -> Option<V> {
        self.0.remove(k)
    }
    fn fold_all(&self) -> u64 {
        self.0.values().map(Value::fold).sum()
    }
    fn update_all(&mut self) {
        self.0.values_mut().for_each(V::bump);
    }
}

impl<V: Value> Store<V> for Dense<V> {
    type Key = DefaultKey;
    const NAME: &'static str = "slotmap_dense";
    fn new() -> Self {
        Self(DenseSlotMap::new())
    }
    fn insert(&mut self, v: V) -> DefaultKey {
        self.0.insert(v)
    }
    fn get(&self, k: DefaultKey) -> Option<&V> {
        self.0.get(k)
    }
    fn remove(&mut self, k: DefaultKey) -> Option<V> {
        self.0.remove(k)
    }
    fn fold_all(&self) -> u64 {
        self.0.values().map(Value::fold).sum()
    }
    fn update_all(&mut self) {
        self.0.values_mut().for_each(V::bump);
    }
}

impl<V: Value> Store<V> for Thunder<V> {
    type Key = Index;
    const NAME: &'static str = "thunderdome";
    fn new() -> Self {
        Self(Arena::new())
    }
    fn insert(&mut self, v: V) -> Index {
        self.0.insert(v)
    }
    fn get(&self, k: Index) -> Option<&V> {
        self.0.get(k)
    }
    fn remove(&mut self, k: Index) -> Option<V> {
        self.0.remove(k)
    }
    fn fold_all(&self) -> u64 {
        self.0.iter().map(|(_, v)| v.fold()).sum()
    }
    fn update_all(&mut self) {
        for (_, v) in &mut self.0 {
            v.bump();
        }
    }
}

// --- helpers -----------------------------------------------------------------------------------

fn filled<S: Store<V>, V: Value>(n: usize) -> (S, Vec<S::Key>) {
    let mut store = S::new();
    let keys = (0..n as u64).map(|i| store.insert(V::make(i))).collect();
    (store, keys)
}

fn id<S: Store<V>, V: Value>(n: usize) -> BenchmarkId {
    BenchmarkId::new(S::NAME, format!("{}/{n}", V::NAME))
}

fn group<'a>(
    c: &'a mut Criterion,
    name: &str,
    n: usize,
) -> criterion::BenchmarkGroup<'a, criterion::measurement::WallTime> {
    let mut g = c.benchmark_group(name);
    g.throughput(Throughput::Elements(n as u64));
    if n >= 1_000_000 {
        g.sample_size(10);
    }
    g
}

// --- pool benchmarks -----------------------------------------------------------------------------

fn bench_insert<S: Store<V>, V: Value>(c: &mut Criterion) {
    for n in SIZES {
        group(c, "insert", n).bench_function(id::<S, V>(n), |b| {
            b.iter(|| {
                let mut store = S::new();
                for i in 0..n as u64 {
                    black_box(store.insert(V::make(i)));
                }
                store
            });
        });
    }
}

fn bench_get_sequential<S: Store<V>, V: Value>(c: &mut Criterion) {
    for n in SIZES {
        let (store, keys) = filled::<S, V>(n);
        group(c, "get_sequential", n).bench_function(id::<S, V>(n), |b| {
            b.iter(|| {
                keys.iter()
                    .map(|k| store.get(*k).map_or(0, Value::fold))
                    .sum::<u64>()
            });
        });
    }
}

fn bench_get_random<S: Store<V>, V: Value>(c: &mut Criterion) {
    for n in SIZES {
        let (store, mut keys) = filled::<S, V>(n);
        shuffle(&mut keys);
        group(c, "get_random", n).bench_function(id::<S, V>(n), |b| {
            b.iter(|| {
                keys.iter()
                    .map(|k| store.get(*k).map_or(0, Value::fold))
                    .sum::<u64>()
            });
        });
    }
}

fn bench_remove_all<S: Store<V>, V: Value>(c: &mut Criterion) {
    for n in SIZES {
        group(c, "remove_all", n).bench_function(id::<S, V>(n), |b| {
            b.iter_batched(
                || filled::<S, V>(n),
                |(mut store, keys)| {
                    for k in keys {
                        black_box(store.remove(k));
                    }
                    store
                },
                BatchSize::LargeInput,
            );
        });
    }
}

fn bench_iterate<S: Store<V>, V: Value>(c: &mut Criterion) {
    for n in SIZES {
        let (store, _keys) = filled::<S, V>(n);
        let mut g = group(c, "iterate", n);
        g.bench_function(id::<S, V>(n), |b| b.iter(|| store.fold_all()));
        if store.fold_slice().is_some() {
            g.bench_function(
                BenchmarkId::new(
                    format!("{}_values_slice", S::NAME),
                    format!("{}/{n}", V::NAME),
                ),
                |b| b.iter(|| store.fold_slice()),
            );
        }
    }
}

/// Remove a seeded-random half, reinsert the same number, then get everything that is alive.
fn bench_churn_then_get<S: Store<V>, V: Value>(c: &mut Criterion) {
    for n in SIZES {
        let (mut store, mut keys) = filled::<S, V>(n);
        shuffle(&mut keys);
        let removed = keys.split_off(n / 2);
        for k in removed {
            store.remove(k);
        }
        keys.extend((0..(n - n / 2) as u64).map(|i| store.insert(V::make(i))));
        shuffle(&mut keys);
        group(c, "churn_then_get", n).bench_function(id::<S, V>(n), |b| {
            b.iter(|| {
                keys.iter()
                    .map(|k| store.get(*k).map_or(0, Value::fold))
                    .sum::<u64>()
            });
        });
    }
}

/// Remove a seeded-random half and never reinsert, then iterate what is left.
/// A sparse container must step over the vacant slots; a dense one does not.
fn bench_iterate_after_churn<S: Store<V>, V: Value>(c: &mut Criterion) {
    for n in SIZES {
        let (mut store, mut keys) = filled::<S, V>(n);
        shuffle(&mut keys);
        for k in keys.split_off(n / 2) {
            store.remove(k);
        }
        let mut g = group(c, "iterate_after_churn", n);
        g.bench_function(id::<S, V>(n), |b| b.iter(|| store.fold_all()));
        if store.fold_slice().is_some() {
            g.bench_function(
                BenchmarkId::new(
                    format!("{}_values_slice", S::NAME),
                    format!("{}/{n}", V::NAME),
                ),
                |b| b.iter(|| store.fold_slice()),
            );
        }
    }
}

// --- realistic scenarios ------------------------------------------------------------------------
//
// These model how a simulation uses a handle container, not single operations in isolation.
// Sizes are mostly cache-resident (100..100k), and values are 16 B and 64 B.

fn real_group<'a>(
    c: &'a mut Criterion,
    name: &str,
    n: usize,
) -> criterion::BenchmarkGroup<'a, criterion::measurement::WallTime> {
    let mut g = c.benchmark_group(name);
    g.throughput(Throughput::Elements(n as u64));
    g.warm_up_time(Duration::from_secs(1));
    g.measurement_time(Duration::from_secs(3));
    g
}

/// The per-frame hot loop: mutate every element through the container's iteration.
fn bench_frame_update<S: Store<V>, V: Value>(c: &mut Criterion) {
    for n in REAL_SIZES {
        let (mut store, _keys) = filled::<S, V>(n);
        let mut g = real_group(c, "scenario/frame_update", n);
        g.bench_function(id::<S, V>(n), |b| {
            b.iter(|| {
                store.update_all();
                black_box(&store);
            });
        });
        if store.update_slice() {
            g.bench_function(
                BenchmarkId::new(
                    format!("{}_values_slice", S::NAME),
                    format!("{}/{n}", V::NAME),
                ),
                |b| {
                    b.iter(|| {
                        store.update_slice();
                        black_box(&store);
                    });
                },
            );
        }
    }
}

/// Bullets and NPCs: a fixed population where 2% die and 2% spawn every frame, then
/// everything is updated. State persists across iterations, so this is steady-state.
fn bench_steady_churn<S: Store<V>, V: Value>(c: &mut Criterion) {
    for n in REAL_SIZES {
        let (mut store, mut keys) = filled::<S, V>(n);
        let mut rng = Prng::from_seed(0xD1B5_4A32_D192_ED03);
        let turnover = (n / 50).max(1);
        let mut next = n as u64;
        real_group(c, "scenario/steady_churn_frame", n).bench_function(id::<S, V>(n), |b| {
            b.iter(|| {
                for _ in 0..turnover {
                    let victim = keys.swap_remove(rng.range(0..keys.len()));
                    black_box(store.remove(victim));
                }
                for _ in 0..turnover {
                    next += 1;
                    keys.push(store.insert(V::make(next)));
                }
                store.update_all();
                black_box(&store);
            });
        });
    }
}

/// Deaths happen in game order, not insertion order: despawn everything in shuffled order.
fn bench_despawn_random_order<S: Store<V>, V: Value>(c: &mut Criterion) {
    for n in REAL_SIZES {
        real_group(c, "scenario/despawn_random_order", n).bench_function(id::<S, V>(n), |b| {
            b.iter_batched(
                || {
                    let (store, mut keys) = filled::<S, V>(n);
                    shuffle(&mut keys);
                    (store, keys)
                },
                |(mut store, keys)| {
                    for k in keys {
                        black_box(store.remove(k));
                    }
                    store
                },
                BatchSize::LargeInput,
            );
        });
    }
}

/// Each entity resolves its own handle and the handle of a random "target" entity.
fn bench_resolve_target<S: Store<V>, V: Value>(c: &mut Criterion) {
    for n in REAL_SIZES {
        let (store, keys) = filled::<S, V>(n);
        let mut targets = keys.clone();
        shuffle(&mut targets);
        real_group(c, "scenario/resolve_target", n).bench_function(id::<S, V>(n), |b| {
            b.iter(|| {
                keys.iter()
                    .zip(&targets)
                    .map(|(k, t)| {
                        store.get(*k).map_or(0, Value::fold) ^ store.get(*t).map_or(0, Value::fold)
                    })
                    .sum::<u64>()
            });
        });
    }
}

/// Dangling references: a fraction of the handles being resolved were despawned (stale),
/// so `get` must take its miss path.
fn bench_get_stale<S: Store<V>, V: Value>(c: &mut Criterion) {
    for pct in [10_usize, 50] {
        for n in REAL_SIZES {
            let (mut store, mut keys) = filled::<S, V>(n);
            shuffle(&mut keys);
            for k in &keys[..n * pct / 100] {
                store.remove(*k);
            }
            shuffle(&mut keys);
            real_group(c, &format!("scenario/get_stale_{pct}pct"), n).bench_function(
                id::<S, V>(n),
                |b| {
                    b.iter(|| {
                        keys.iter()
                            .map(|k| store.get(*k).map_or(0, Value::fold))
                            .sum::<u64>()
                    });
                },
            );
        }
    }
}

// --- allocator benchmarks (no values: slotmap with `()` is the comparison) --------------------

trait Alloc {
    type Key: Copy;
    const NAME: &'static str;
    fn new() -> Self;
    fn allocate(&mut self) -> Self::Key;
    fn free(&mut self, k: Self::Key) -> bool;
    fn is_alive(&self, k: Self::Key) -> bool;
}

struct EngineAlloc(DenseHandleAllocator<()>);
struct LeanAlloc(HandleAllocator<()>);
struct SlotAlloc(SlotMap<DefaultKey, ()>);

impl Alloc for LeanAlloc {
    type Key = Handle<()>;
    const NAME: &'static str = "engine_handle_lean";
    fn new() -> Self {
        Self(HandleAllocator::new())
    }
    fn allocate(&mut self) -> Handle<()> {
        self.0.allocate()
    }
    fn free(&mut self, k: Handle<()>) -> bool {
        self.0.free(k)
    }
    fn is_alive(&self, k: Handle<()>) -> bool {
        self.0.is_alive(k)
    }
}

impl Alloc for EngineAlloc {
    type Key = Handle<()>;
    const NAME: &'static str = "engine_handle_dense";
    fn new() -> Self {
        Self(DenseHandleAllocator::new())
    }
    fn allocate(&mut self) -> Handle<()> {
        self.0.allocate()
    }
    fn free(&mut self, k: Handle<()>) -> bool {
        self.0.free(k)
    }
    fn is_alive(&self, k: Handle<()>) -> bool {
        self.0.is_alive(k)
    }
}

impl Alloc for SlotAlloc {
    type Key = DefaultKey;
    const NAME: &'static str = "slotmap";
    fn new() -> Self {
        Self(SlotMap::new())
    }
    fn allocate(&mut self) -> DefaultKey {
        self.0.insert(())
    }
    fn free(&mut self, k: DefaultKey) -> bool {
        self.0.remove(k).is_some()
    }
    fn is_alive(&self, k: DefaultKey) -> bool {
        self.0.contains_key(k)
    }
}

fn allocated<A: Alloc>(n: usize) -> (A, Vec<A::Key>) {
    let mut alloc = A::new();
    let keys = (0..n).map(|_| alloc.allocate()).collect();
    (alloc, keys)
}

fn bench_allocate<A: Alloc>(c: &mut Criterion) {
    for n in SIZES {
        group(c, "allocator/allocate", n).bench_function(BenchmarkId::new(A::NAME, n), |b| {
            b.iter(|| allocated::<A>(n));
        });
    }
}

/// Spawning `n` handles at once: a loop of `allocate` against `allocate_many`, on a fresh
/// allocator (all new slots) and on one whose free list holds `n` slots (all recycled).
fn bench_batch_allocate(c: &mut Criterion) {
    for n in SIZES {
        for recycled in [false, true] {
            let name = if recycled {
                "allocator/batch_recycled"
            } else {
                "allocator/batch_fresh"
            };
            let setup = move || {
                let mut a = HandleAllocator::<()>::new();
                if recycled {
                    let hs: Vec<_> = (0..n).map(|_| a.allocate()).collect();
                    for h in hs {
                        a.free(h);
                    }
                }
                a
            };
            let mut g = group(c, name, n);
            // Both variants build their own result vector, so its allocation counts for both.
            g.bench_function(BenchmarkId::new("loop", n), |b| {
                b.iter_batched(
                    setup,
                    |mut a| {
                        let mut out = Vec::with_capacity(n);
                        for _ in 0..n {
                            out.push(a.allocate());
                        }
                        (a, out)
                    },
                    BatchSize::LargeInput,
                );
            });
            g.bench_function(BenchmarkId::new("batch", n), |b| {
                b.iter_batched(
                    setup,
                    |mut a| {
                        let out = a.allocate_many(n);
                        (a, out)
                    },
                    BatchSize::LargeInput,
                );
            });
            g.finish();
        }
    }
}

fn bench_free_realloc_churn<A: Alloc>(c: &mut Criterion) {
    for n in SIZES {
        group(c, "allocator/free_realloc_churn", n).bench_function(
            BenchmarkId::new(A::NAME, n),
            |b| {
                b.iter_batched(
                    || {
                        let (alloc, mut keys) = allocated::<A>(n);
                        shuffle(&mut keys);
                        (alloc, keys)
                    },
                    |(mut alloc, keys)| {
                        for k in &keys[..n / 2] {
                            black_box(alloc.free(*k));
                        }
                        for _ in 0..n / 2 {
                            black_box(alloc.allocate());
                        }
                        alloc
                    },
                    BatchSize::LargeInput,
                );
            },
        );
    }
}

fn bench_is_alive<A: Alloc>(c: &mut Criterion) {
    for n in SIZES {
        let (alloc, mut keys) = allocated::<A>(n);
        shuffle(&mut keys);
        group(c, "allocator/is_alive", n).bench_function(BenchmarkId::new(A::NAME, n), |b| {
            b.iter(|| keys.iter().filter(|k| alloc.is_alive(**k)).count());
        });
    }
}

// --- wiring ---------------------------------------------------------------------------------------

/// Runs one generic store benchmark for every container and both value types.
macro_rules! all_stores {
    ($c:expr, $bench:ident) => {
        $bench::<Pool<u64>, u64>($c);
        $bench::<Slot<u64>, u64>($c);
        $bench::<Dense<u64>, u64>($c);
        $bench::<Thunder<u64>, u64>($c);
        $bench::<Pool<[u64; 8]>, [u64; 8]>($c);
        $bench::<Slot<[u64; 8]>, [u64; 8]>($c);
        $bench::<Dense<[u64; 8]>, [u64; 8]>($c);
        $bench::<Thunder<[u64; 8]>, [u64; 8]>($c);
    };
}

/// Like `all_stores!`, for the scenario groups: 16 B and 64 B values.
macro_rules! real_stores {
    ($c:expr, $bench:ident) => {
        $bench::<Pool<[u64; 2]>, [u64; 2]>($c);
        $bench::<Slot<[u64; 2]>, [u64; 2]>($c);
        $bench::<Dense<[u64; 2]>, [u64; 2]>($c);
        $bench::<Thunder<[u64; 2]>, [u64; 2]>($c);
        $bench::<Pool<[u64; 8]>, [u64; 8]>($c);
        $bench::<Slot<[u64; 8]>, [u64; 8]>($c);
        $bench::<Dense<[u64; 8]>, [u64; 8]>($c);
        $bench::<Thunder<[u64; 8]>, [u64; 8]>($c);
    };
}

fn scenario_benches(c: &mut Criterion) {
    real_stores!(c, bench_frame_update);
    real_stores!(c, bench_steady_churn);
    real_stores!(c, bench_despawn_random_order);
    real_stores!(c, bench_resolve_target);
    real_stores!(c, bench_get_stale);
}

fn pool_benches(c: &mut Criterion) {
    all_stores!(c, bench_insert);
    all_stores!(c, bench_get_sequential);
    all_stores!(c, bench_get_random);
    all_stores!(c, bench_remove_all);
    all_stores!(c, bench_iterate);
    all_stores!(c, bench_churn_then_get);
    all_stores!(c, bench_iterate_after_churn);
}

fn allocator_benches(c: &mut Criterion) {
    bench_allocate::<EngineAlloc>(c);
    bench_allocate::<LeanAlloc>(c);
    bench_allocate::<SlotAlloc>(c);
    bench_free_realloc_churn::<EngineAlloc>(c);
    bench_free_realloc_churn::<LeanAlloc>(c);
    bench_free_realloc_churn::<SlotAlloc>(c);
    bench_is_alive::<EngineAlloc>(c);
    bench_is_alive::<LeanAlloc>(c);
    bench_is_alive::<SlotAlloc>(c);
    bench_batch_allocate(c);
}

criterion_group!(benches, pool_benches, allocator_benches, scenario_benches);
criterion_main!(benches);
