//! Checks against published reference values (in addition to the independent-implementation
//! oracle in `oracle.rs`).
//!
//! - `SplitMix64` seeded with 0: the first outputs `e220a8397b1dcdaf`, `6e789e6aa1b965f4`,
//!   `06c45d188009454f`, `f88bb8a8724c81ec` are the widely quoted reference values.
//! - xoshiro256++ from the state `[1, 2, 3, 4]`: the first output follows by hand from the
//!   definition, `rotl(1 + 4, 23) + 1 = 5 * 2^23 + 1 = 41943041`; the following values are the
//!   reference test vector used by xoshiro implementations.
#![forbid(unsafe_code)]

use engine_random::Prng;

#[test]
fn splitmix64_seed_zero_reference_values() {
    assert_eq!(
        Prng::from_seed(0).to_state(),
        [
            0xe220_a839_7b1d_cdaf,
            0x6e78_9e6a_a1b9_65f4,
            0x06c4_5d18_8009_454f,
            0xf88b_b8a8_724c_81ec
        ]
    );
}

#[test]
fn xoshiro256pp_reference_vector_for_state_1_2_3_4() {
    let mut rng = Prng::from_state([1, 2, 3, 4]).unwrap();
    assert_eq!(rng.next_u64(), 41_943_041); // derivable by hand from the definition
    let expected = [
        58_720_359_u64,
        3_588_806_011_781_223,
        3_591_011_842_654_386,
        9_228_616_714_210_784_205,
        9_973_669_472_204_895_162,
    ];
    for (i, want) in expected.into_iter().enumerate() {
        assert_eq!(rng.next_u64(), want, "output {}", i + 2);
    }
}
