//! The conformance check: a candidate backend against the reference backend.

use core::fmt;

use crate::{Params, Workload};

/// The first tick at which a candidate's checksum differed from the reference's.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Divergence {
    /// The workload's name.
    pub workload: &'static str,
    /// The reference backend's name.
    pub reference: &'static str,
    /// The candidate backend's name.
    pub candidate: &'static str,
    /// 0 for the state after `setup`, otherwise the number of steps run.
    pub tick: u64,
    /// The reference's checksum.
    pub expected: u64,
    /// The candidate's checksum.
    pub actual: u64,
}

impl fmt::Display for Divergence {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}: {} diverges from {} at tick {} (expected {:#018x}, got {:#018x})",
            self.workload, self.candidate, self.reference, self.tick, self.expected, self.actual
        )
    }
}

impl std::error::Error for Divergence {}

/// Sets up the reference `R` and the candidate `C` with `params`, compares their checksums, then
/// steps both `ticks` times and compares after every step.
///
/// Returns the first [`Divergence`], or `Ok(())`. It never panics on divergence, so tests can
/// check that a broken candidate is detected.
///
/// # Errors
///
/// A [`Divergence`] naming the first tick (0 is the state right after `setup`) at which the
/// checksums differ.
pub fn check_conformance<R, C>(params: Params, ticks: u64) -> Result<(), Divergence>
where
    R: Workload,
    C: Workload<Spec = R::Spec>,
{
    let mut reference = R::setup(params);
    let mut candidate = C::setup(params);
    let compare = |tick: u64, reference: &R, candidate: &C| {
        let (expected, actual) = (reference.checksum(), candidate.checksum());
        if expected == actual {
            Ok(())
        } else {
            Err(Divergence {
                workload: <R::Spec as crate::Spec>::NAME,
                reference: R::BACKEND,
                candidate: C::BACKEND,
                tick,
                expected,
                actual,
            })
        }
    };
    compare(0, &reference, &candidate)?;
    for tick in 1..=ticks {
        reference.step();
        candidate.step();
        compare(tick, &reference, &candidate)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Params, Spec, Workload};

    struct CounterSpec;
    impl Spec for CounterSpec {
        const NAME: &'static str = "counter";
        const SIZES: &'static [usize] = &[1];
    }

    struct Counter(u64);
    impl Workload for Counter {
        type Spec = CounterSpec;
        const BACKEND: &'static str = "counter_ref";
        fn setup(_: Params) -> Self {
            Self(0)
        }
        fn step(&mut self) {
            self.0 += 1;
        }
        fn checksum(&self) -> u64 {
            self.0
        }
    }

    struct OffByOneAtSetup(u64);
    impl Workload for OffByOneAtSetup {
        type Spec = CounterSpec;
        const BACKEND: &'static str = "off_by_one";
        fn setup(_: Params) -> Self {
            Self(1)
        }
        fn step(&mut self) {
            self.0 += 1;
        }
        fn checksum(&self) -> u64 {
            self.0
        }
    }

    struct BreaksAtTick3 {
        state: u64,
        steps: u64,
    }
    impl Workload for BreaksAtTick3 {
        type Spec = CounterSpec;
        const BACKEND: &'static str = "breaks_at_3";
        fn setup(_: Params) -> Self {
            Self { state: 0, steps: 0 }
        }
        fn step(&mut self) {
            self.steps += 1;
            self.state += if self.steps == 3 { 2 } else { 1 };
        }
        fn checksum(&self) -> u64 {
            self.state
        }
    }

    const P: Params = Params { count: 1, seed: 42 };

    #[test]
    fn a_workload_conforms_to_itself() {
        assert_eq!(check_conformance::<Counter, Counter>(P, 10), Ok(()));
    }

    #[test]
    fn divergence_after_setup_is_tick_zero() {
        let d = check_conformance::<Counter, OffByOneAtSetup>(P, 10).unwrap_err();
        assert_eq!((d.tick, d.expected, d.actual), (0, 0, 1));
    }

    #[test]
    fn divergence_reports_the_first_tick() {
        let d = check_conformance::<Counter, BreaksAtTick3>(P, 10).unwrap_err();
        assert_eq!(d.tick, 3);
        assert_eq!(
            (d.workload, d.reference, d.candidate),
            ("counter", "counter_ref", "breaks_at_3")
        );
    }

    #[test]
    fn divergence_message_names_everything() {
        let d = check_conformance::<Counter, BreaksAtTick3>(P, 10).unwrap_err();
        let message = d.to_string();
        for part in [
            "counter",
            "counter_ref",
            "breaks_at_3",
            "tick 3",
            "0x0000000000000003",
            "0x0000000000000004",
        ] {
            assert!(message.contains(part), "{message:?} lacks {part:?}");
        }
    }
}
