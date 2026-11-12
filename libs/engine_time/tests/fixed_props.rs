//! Property test: splitting elapsed time differently never changes the tick sequence
//! (as long as the step cap is not hit).

use std::num::NonZeroU32;
use std::time::Duration;

use engine_time::{FixedTimestep, Tick};
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

proptest! {
    #![proptest_config(config())]

    /// Total time is at most 64 chunks of 31 ms (about 2 s). Both timesteps use the largest
    /// possible cap, so it is never hit by either the chunked or the single call.
    #[test]
    fn splitting_elapsed_time_never_changes_the_ticks(
        step_ms in 1_u64..=50,
        chunks_ns in proptest::collection::vec(1_u64..=31_000_000, 1..=64),
    ) {
        let step = Duration::from_millis(step_ms);
        let new = || FixedTimestep::new(step).with_max_steps_per_update(NonZeroU32::MAX);

        let mut chunked = new();
        let chunked_ticks: Vec<Tick> = chunks_ns
            .iter()
            .flat_map(|ns| chunked.accumulate(Duration::from_nanos(*ns)).collect::<Vec<_>>())
            .collect();

        let mut single = new();
        let total = Duration::from_nanos(chunks_ns.iter().sum());
        let single_ticks: Vec<Tick> = single.accumulate(total).collect();

        prop_assert_eq!(chunked_ticks, single_ticks);
        prop_assert_eq!(chunked.to_state(), single.to_state());
    }
}
