//! Documented panics: location (`#[track_caller]`) and message quality.
//!
//! Panics abort on WASI, so these tests run on native targets only (spec §4.7).
#![forbid(unsafe_code)]
#![cfg(not(target_os = "wasi"))]

use std::panic::{self, AssertUnwindSafe};
use std::sync::Mutex;
use std::time::Duration;

use engine_time::{FixedTimestep, ManualClock, Tick, Timestamp};

static CAPTURED: Mutex<Option<(String, String)>> = Mutex::new(None);

/// Runs `f`, which must panic, and returns the panic's (file, message).
fn capture(f: impl FnOnce()) -> (String, String) {
    panic::set_hook(Box::new(|info| {
        let file = info
            .location()
            .map_or_default(|l| l.file().replace('\\', "/"));
        let message = info.payload_as_str().unwrap_or_default().to_string();
        *CAPTURED.lock().expect("lock") = Some((file, message));
    }));
    let result = panic::catch_unwind(AssertUnwindSafe(f));
    drop(panic::take_hook());
    assert!(result.is_err(), "expected a panic");
    CAPTURED.lock().expect("lock").take().expect("hook ran")
}

#[test]
fn panics_point_at_the_caller_and_name_the_fallible_twin() {
    // One test function: the panic hook is process-global.
    let (file, message) = capture(|| {
        let mut clock = ManualClock::starting_at(Timestamp::from_duration(Duration::from_secs(5)));
        clock.set(Timestamp::default());
    });
    assert!(
        file.ends_with("tests/panics.rs"),
        "set: location was {file}"
    );
    assert!(message.contains("try_set"), "set: message was {message}");

    let (file, message) = capture(|| {
        let _ = Tick::new(u64::MAX).next();
    });
    assert!(
        file.ends_with("tests/panics.rs"),
        "next: location was {file}"
    );
    assert!(
        message.contains("checked_next"),
        "next: message was {message}"
    );

    let (file, message) = capture(|| {
        let _ = FixedTimestep::new(Duration::ZERO);
    });
    assert!(
        file.ends_with("tests/panics.rs"),
        "new: location was {file}"
    );
    assert!(message.contains("try_new"), "new: message was {message}");
}
