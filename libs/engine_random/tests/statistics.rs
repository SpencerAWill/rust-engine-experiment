//! Statistical sanity checks with fixed seeds. All `#[ignore]`: run with
//! `cargo xtask test --release --include-ignored`.
//!
//! Chi-square thresholds are the p = 0.001 critical values, so a correct generator fails a
//! given fixed seed with probability 0.1%, and the outcome is deterministic.
#![forbid(unsafe_code)]
#![allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "counts and indices are small"
)]

use std::collections::BTreeMap;

use engine_random::Prng;

fn chi_square(counts: &[u64], expected: f64) -> f64 {
    counts
        .iter()
        .map(|&c| {
            let d = c as f64 - expected;
            d * d / expected
        })
        .sum()
}

fn bucket_test(n_buckets: usize, samples: u64, mut draw: impl FnMut() -> usize) -> f64 {
    let mut counts = vec![0_u64; n_buckets];
    for _ in 0..samples {
        counts[draw()] += 1;
    }
    chi_square(&counts, samples as f64 / n_buckets as f64)
}

#[test]
#[ignore = "statistical, run in release"]
fn range_0_10_is_uniform() {
    let mut p = Prng::from_seed(101);
    let stat = bucket_test(10, 1_000_000, || p.range(0..10_usize));
    assert!(stat < 27.88, "chi-square {stat} (df = 9)");
}

#[test]
#[ignore = "statistical, run in release"]
fn range_0_7_is_uniform() {
    let mut p = Prng::from_seed(102);
    let stat = bucket_test(7, 1_000_000, || p.range(0..7_usize));
    assert!(stat < 22.46, "chi-square {stat} (df = 6)");
}

#[test]
#[ignore = "statistical, run in release"]
fn next_f32_is_uniform() {
    let mut p = Prng::from_seed(103);
    let stat = bucket_test(16, 1_000_000, || (p.next_f32() * 16.0) as usize);
    assert!(stat < 37.70, "chi-square {stat} (df = 15)");
}

#[test]
#[ignore = "statistical, run in release"]
fn next_f64_is_uniform() {
    let mut p = Prng::from_seed(104);
    let stat = bucket_test(16, 1_000_000, || (p.next_f64() * 16.0) as usize);
    assert!(stat < 37.70, "chi-square {stat} (df = 15)");
}

#[test]
#[ignore = "statistical, run in release"]
fn next_bool_is_balanced() {
    let mut p = Prng::from_seed(105);
    let n = 1_000_000_u32;
    let trues = (0..n).filter(|_| p.next_bool()).count();
    let fraction = trues as f64 / f64::from(n);
    assert!(
        (fraction - 0.5).abs() < 0.002,
        "fraction of true was {fraction}"
    );
}

#[test]
#[ignore = "statistical, run in release"]
fn shuffle_of_four_hits_all_24_permutations_uniformly() {
    let mut p = Prng::from_seed(106);
    let mut counts: BTreeMap<[u8; 4], u64> = BTreeMap::new();
    for _ in 0..240_000 {
        let mut v = [0_u8, 1, 2, 3];
        p.shuffle(&mut v);
        *counts.entry(v).or_default() += 1;
    }
    assert_eq!(counts.len(), 24, "every permutation must occur");
    let counts: Vec<u64> = counts.into_values().collect();
    let stat = chi_square(&counts, 10_000.0);
    assert!(stat < 49.73, "chi-square {stat} (df = 23)");
}
