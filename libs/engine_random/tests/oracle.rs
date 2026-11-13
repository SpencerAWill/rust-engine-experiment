//! Cross-checks against an independent implementation (`rand_xoshiro`, used as a test
//! oracle only; nothing is copied from it).
#![forbid(unsafe_code)]

use engine_random::Prng;
use rand_xoshiro::rand_core::{Rng, SeedableRng};
use rand_xoshiro::{SplitMix64, Xoshiro256PlusPlus};

const SEEDS: [u64; 5] = [0, 1, 42, u64::MAX, 0xDEAD_BEEF];

#[test]
fn xoshiro_matches_independent_implementation() {
    for seed in SEEDS {
        let ours = Prng::from_seed(seed);
        let bytes: Vec<u8> = ours
            .to_state()
            .iter()
            .flat_map(|w| w.to_le_bytes())
            .collect();
        let mut theirs = Xoshiro256PlusPlus::from_seed(bytes.try_into().unwrap());
        let mut ours = ours;
        for i in 0..1000 {
            assert_eq!(ours.next_u64(), theirs.next_u64(), "seed {seed}, draw {i}");
        }
    }
}

#[test]
fn splitmix_seeding_matches_independent_implementation() {
    for seed in SEEDS {
        let mut theirs = SplitMix64::from_seed(seed.to_le_bytes());
        let expected = [
            theirs.next_u64(),
            theirs.next_u64(),
            theirs.next_u64(),
            theirs.next_u64(),
        ];
        assert_eq!(Prng::from_seed(seed).to_state(), expected, "seed {seed}");
    }
}

/// The value `range(0..n)` must return, from first principles: draw 64-bit values `x` and take
/// the first whose product `x * n` has a low half of at least `2^64 mod n` (this removes the
/// bias); the answer is the high half of that product.
#[allow(
    clippy::cast_possible_truncation,
    reason = "taking the low and high 64-bit halves is the definition"
)]
fn bounded_reference(stream: &mut Prng, n: u64) -> u64 {
    let threshold = ((1_u128 << 64) % u128::from(n)) as u64;
    loop {
        let product = u128::from(stream.next_u64()) * u128::from(n);
        if product as u64 >= threshold {
            return (product >> 64) as u64;
        }
    }
}

#[test]
fn bounded_ranges_match_the_rejection_rule_for_spans_that_reject_often() {
    // Spans above 2^63 reject about half the draws, so the rejection loop runs constantly.
    for n in [(1_u64 << 63) + 1, 3 << 62, u64::MAX] {
        let (mut actual, mut expected) = (Prng::from_seed(n), Prng::from_seed(n));
        for _ in 0..200 {
            assert_eq!(actual.range(0..n), bounded_reference(&mut expected, n));
        }
        assert_eq!(
            actual.next_u64(),
            expected.next_u64(),
            "same draws consumed"
        );
    }
}

#[test]
#[allow(
    clippy::cast_possible_truncation,
    reason = "taking the low half of a product is the point"
)]
fn a_draw_exactly_at_the_rejection_threshold_is_accepted() {
    let n = (1_u64 << 63) + 1;
    let threshold = n.wrapping_neg() % n;
    // The draw whose product with `n` has its low half exactly at the threshold: `n` is odd, so
    // it has an inverse modulo 2^64 (Newton's iteration doubles the correct bits each step).
    let mut inverse = n;
    for _ in 0..6 {
        inverse = inverse.wrapping_mul(2_u64.wrapping_sub(n.wrapping_mul(inverse)));
    }
    let draw = threshold.wrapping_mul(inverse);
    assert_eq!(
        u128::from(draw).wrapping_mul(u128::from(n)) as u64,
        threshold
    );
    // xoshiro256++ returns `rotl(s0 + s3, 23) + s0`, so pick `s3` to make the first output `draw`.
    let s0 = 1_u64;
    let s3 = draw.wrapping_sub(s0).rotate_right(23).wrapping_sub(s0);
    let start = Prng::from_state([s0, 1, 1, s3]).unwrap();
    assert_eq!(start.clone().next_u64(), draw);

    let mut rng = start.clone();
    let value = rng.range(0..n);
    assert_eq!(value, ((u128::from(draw) * u128::from(n)) >> 64) as u64);
    let mut one_draw = start;
    let _ = one_draw.next_u64();
    assert_eq!(
        rng.next_u64(),
        one_draw.next_u64(),
        "accepted without a second draw"
    );
}
