//! Property tests: `range` never leaves its bounds, and `try_range` rejects exactly the invalid
//! ranges without drawing.
#![forbid(unsafe_code)]

use engine_random::Prng;
use proptest::prelude::*;

/// WASI has no writable directory for proptest's failure persistence.
fn config() -> ProptestConfig {
    if cfg!(target_os = "wasi") {
        ProptestConfig {
            failure_persistence: None,
            ..ProptestConfig::default()
        }
    } else {
        ProptestConfig::default()
    }
}

/// Checks `range`/`try_range` for an integer type over `lo..hi` and `lo..=hi`.
macro_rules! check_int {
    ($seed:expr, $lo:expr, $hi:expr) => {{
        let (lo, hi) = ($lo, $hi);
        let mut rng = Prng::from_seed($seed);
        let before = rng.clone();

        // Exclusive range: valid iff lo < hi.
        match rng.try_range(lo..hi) {
            Some(v) => prop_assert!(lo < hi && (lo..hi).contains(&v), "{v} outside {lo}..{hi}"),
            None => {
                prop_assert!(lo >= hi);
                prop_assert_eq!(&rng, &before, "an invalid range must not draw");
            }
        }
        // Inclusive range: valid iff lo <= hi.
        let mut rng = Prng::from_seed($seed);
        match rng.try_range(lo..=hi) {
            Some(v) => prop_assert!(
                lo <= hi && (lo..=hi).contains(&v),
                "{v} outside {lo}..={hi}"
            ),
            None => {
                prop_assert!(lo > hi);
                prop_assert_eq!(&rng, &before, "an invalid range must not draw");
            }
        }
    }};
}

proptest! {
    #![proptest_config(config())]

    #[test]
    fn u8_ranges(seed in any::<u64>(), lo in any::<u8>(), hi in any::<u8>()) { check_int!(seed, lo, hi); }

    #[test]
    fn i16_ranges(seed in any::<u64>(), lo in any::<i16>(), hi in any::<i16>()) { check_int!(seed, lo, hi); }

    #[test]
    fn u32_ranges(seed in any::<u64>(), lo in any::<u32>(), hi in any::<u32>()) { check_int!(seed, lo, hi); }

    #[test]
    fn i64_ranges(seed in any::<u64>(), lo in any::<i64>(), hi in any::<i64>()) { check_int!(seed, lo, hi); }

    #[test]
    fn usize_ranges(seed in any::<u64>(), lo in any::<usize>(), hi in any::<usize>()) { check_int!(seed, lo, hi); }

    #[test]
    fn isize_ranges(seed in any::<u64>(), lo in any::<isize>(), hi in any::<isize>()) { check_int!(seed, lo, hi); }

    #[test]
    fn f32_ranges(seed in any::<u64>(), lo in -1e30_f32..1e30, hi in -1e30_f32..1e30) {
        let mut rng = Prng::from_seed(seed);
        if let Some(v) = rng.try_range(lo..hi) {
            prop_assert!(lo < hi && v >= lo && v < hi, "{v} outside {lo}..{hi}");
        } else {
            prop_assert!(lo >= hi || !(hi - lo).is_finite());
        }
        let mut rng = Prng::from_seed(seed);
        if let Some(v) = rng.try_range(lo..=hi) {
            prop_assert!(lo <= hi && v >= lo && v <= hi, "{v} outside {lo}..={hi}");
        } else {
            prop_assert!(lo > hi || !(hi - lo).is_finite());
        }
    }

    #[test]
    fn f64_ranges(seed in any::<u64>(), lo in -1e300_f64..1e300, hi in -1e300_f64..1e300) {
        let mut rng = Prng::from_seed(seed);
        if let Some(v) = rng.try_range(lo..hi) {
            prop_assert!(lo < hi && v >= lo && v < hi, "{v} outside {lo}..{hi}");
        } else {
            prop_assert!(lo >= hi || !(hi - lo).is_finite());
        }
    }
}
