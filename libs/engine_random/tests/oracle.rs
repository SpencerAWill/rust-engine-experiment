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
