//! Every public type is `Send + Sync + Debug` unless documented otherwise (spec §4.4).
//!
//! Handles and allocators are generic over a phantom type, so they are checked with a
//! non-`Send` type parameter (`Rc<u8>`) to prove the marker does not leak into them.
#![forbid(unsafe_code)]

use std::fmt::Debug;
use std::rc::Rc;

use engine::handle::{CapacityError, DenseHandleAllocator, Handle, HandleAllocator, HandlePool};
use engine::math::convert::CoordinateSystem;
use engine::math::{Transform, Transform2d, WorldTransform, WorldTransform2d};
use engine::random::Prng;
use engine::time::{
    FixedTimestep, FixedTimestepState, ManualClock, NonMonotonicError, Steps, Tick, Timestamp,
};

fn assert_traits<T: Send + Sync + Debug>() {}

#[test]
fn handle_types_are_send_sync_debug() {
    assert_traits::<Handle<Rc<u8>>>();
    assert_traits::<HandlePool<u8>>();
    assert_traits::<HandleAllocator<Rc<u8>>>();
    assert_traits::<DenseHandleAllocator<Rc<u8>>>();
    assert_traits::<CapacityError<u8>>();
}

#[test]
fn random_and_time_types_are_send_sync_debug() {
    assert_traits::<Prng>();
    assert_traits::<Tick>();
    assert_traits::<Timestamp>();
    assert_traits::<FixedTimestep>();
    assert_traits::<FixedTimestepState>();
    assert_traits::<Steps>();
    assert_traits::<ManualClock>();
    assert_traits::<NonMonotonicError>();
}

#[cfg(all(
    feature = "std",
    not(all(target_arch = "wasm32", target_os = "unknown"))
))]
#[test]
fn system_clock_is_send_sync_debug() {
    assert_traits::<engine::time::SystemClock>();
}

#[test]
fn math_types_are_send_sync_debug() {
    assert_traits::<Transform>();
    assert_traits::<WorldTransform>();
    assert_traits::<Transform2d>();
    assert_traits::<WorldTransform2d>();
    assert_traits::<CoordinateSystem>();
}
