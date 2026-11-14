// benchmark harness, not simulation code
#![forbid(unsafe_code)]
#![allow(
    missing_docs,
    clippy::disallowed_types,
    clippy::disallowed_methods,
    clippy::unwrap_used
)]
//! PRNG comparisons: `engine_random::Prng` vs `rand_xoshiro` (same algorithm), `rand`'s
//! `SmallRng`, and `fastrand`.
//!
//! Each benchmark performs 1,000,000 draws per iteration (the shuffle benchmark shuffles 10,000
//! elements), so the reported time divided by 10^6 is the cost per draw. The libraries are not
//! bit-compatible (different range algorithms), so this measures speed, not agreement.

use std::hint::black_box;

use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use engine_random::Prng;
use rand::rngs::SmallRng;
use rand::seq::SliceRandom;
use rand::{Rng, RngExt, SeedableRng};
use rand_xoshiro::Xoshiro256PlusPlus;

const DRAWS: u64 = 1_000_000;
const SEED: u64 = 0x5EED;

/// The four operations under test, behind one interface.
trait Generator {
    const NAME: &'static str;
    fn new() -> Self;
    fn next_u64(&mut self) -> u64;
    fn range_u32(&mut self, lo: u32, hi: u32) -> u32;
    fn range_f32(&mut self) -> f32;
    fn shuffle(&mut self, items: &mut [u32]);
}

impl Generator for Prng {
    const NAME: &'static str = "engine_random";
    fn new() -> Self {
        Prng::from_seed(SEED)
    }
    fn next_u64(&mut self) -> u64 {
        Prng::next_u64(self)
    }
    fn range_u32(&mut self, lo: u32, hi: u32) -> u32 {
        self.range(lo..hi)
    }
    fn range_f32(&mut self) -> f32 {
        self.range(0.0_f32..1.0)
    }
    fn shuffle(&mut self, items: &mut [u32]) {
        Prng::shuffle(self, items);
    }
}

/// Implements `Generator` for a `rand`-style RNG.
macro_rules! impl_rand_generator {
    ($ty:ty, $name:literal) => {
        impl Generator for $ty {
            const NAME: &'static str = $name;
            fn new() -> Self {
                <$ty>::seed_from_u64(SEED)
            }
            fn next_u64(&mut self) -> u64 {
                Rng::next_u64(self)
            }
            fn range_u32(&mut self, lo: u32, hi: u32) -> u32 {
                self.random_range(lo..hi)
            }
            fn range_f32(&mut self) -> f32 {
                self.random_range(0.0_f32..1.0)
            }
            fn shuffle(&mut self, items: &mut [u32]) {
                items.shuffle(self);
            }
        }
    };
}

impl_rand_generator!(Xoshiro256PlusPlus, "rand_xoshiro");
impl_rand_generator!(SmallRng, "rand_small_rng");

impl Generator for fastrand::Rng {
    const NAME: &'static str = "fastrand";
    fn new() -> Self {
        fastrand::Rng::with_seed(SEED)
    }
    fn next_u64(&mut self) -> u64 {
        self.u64(..)
    }
    fn range_u32(&mut self, lo: u32, hi: u32) -> u32 {
        self.u32(lo..hi)
    }
    fn range_f32(&mut self) -> f32 {
        self.f32()
    }
    fn shuffle(&mut self, items: &mut [u32]) {
        fastrand::Rng::shuffle(self, items);
    }
}

fn bench_next_u64<G: Generator>(c: &mut Criterion) {
    let mut g = c.benchmark_group("prng/next_u64");
    g.throughput(Throughput::Elements(DRAWS));
    let mut rng = G::new();
    g.bench_function(G::NAME, |b| {
        b.iter(|| {
            let mut acc = 0_u64;
            for _ in 0..DRAWS {
                acc ^= rng.next_u64();
            }
            black_box(acc)
        });
    });
}

fn bench_range_u32<G: Generator>(c: &mut Criterion) {
    let mut g = c.benchmark_group("prng/range_u32_0_1000");
    g.throughput(Throughput::Elements(DRAWS));
    let mut rng = G::new();
    g.bench_function(G::NAME, |b| {
        b.iter(|| {
            let mut acc = 0_u32;
            for _ in 0..DRAWS {
                acc ^= rng.range_u32(0, 1000);
            }
            black_box(acc)
        });
    });
}

fn bench_range_f32<G: Generator>(c: &mut Criterion) {
    let mut g = c.benchmark_group("prng/range_f32_0_1");
    g.throughput(Throughput::Elements(DRAWS));
    let mut rng = G::new();
    g.bench_function(G::NAME, |b| {
        b.iter(|| {
            let mut acc = 0.0_f32;
            for _ in 0..DRAWS {
                acc += rng.range_f32();
            }
            black_box(acc)
        });
    });
}

fn bench_shuffle<G: Generator>(c: &mut Criterion) {
    let mut g = c.benchmark_group("prng/shuffle_10k");
    g.throughput(Throughput::Elements(10_000));
    let mut rng = G::new();
    let mut items: Vec<u32> = (0..10_000).collect();
    g.bench_function(G::NAME, |b| {
        b.iter(|| {
            rng.shuffle(&mut items);
            black_box(&items);
        });
    });
}

/// Runs one generic benchmark for every generator.
macro_rules! all_generators {
    ($c:expr, $bench:ident) => {
        $bench::<Prng>($c);
        $bench::<Xoshiro256PlusPlus>($c);
        $bench::<SmallRng>($c);
        $bench::<fastrand::Rng>($c);
    };
}

fn prng_benches(c: &mut Criterion) {
    all_generators!(c, bench_next_u64);
    all_generators!(c, bench_range_u32);
    all_generators!(c, bench_range_f32);
    all_generators!(c, bench_shuffle);
}

criterion_group!(benches, prng_benches);
criterion_main!(benches);
