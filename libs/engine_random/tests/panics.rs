//! Documented panics: location (`#[track_caller]`) and message quality.
//!
//! Panics abort on WASI, so these tests run on native targets only (spec §4.7).
#![forbid(unsafe_code)]
#![cfg(not(target_os = "wasi"))]

use std::panic::{self, AssertUnwindSafe};
use std::sync::Mutex;

use engine_random::Prng;

static CAPTURED: Mutex<Option<(String, String)>> = Mutex::new(None);

#[test]
fn empty_range_panics_at_caller_with_actionable_message() {
    let mut rng = Prng::from_seed(1);

    panic::set_hook(Box::new(|info| {
        let file = info
            .location()
            .map(|l| l.file().replace('\\', "/"))
            .unwrap_or_default();
        let message = info.payload_as_str().unwrap_or_default().to_string();
        *CAPTURED.lock().unwrap() = Some((file, message));
    }));
    let result = panic::catch_unwind(AssertUnwindSafe(|| rng.range(5..5)));
    drop(panic::take_hook());

    assert!(result.is_err());
    let (file, message) = CAPTURED.lock().unwrap().take().unwrap();
    assert!(
        file.ends_with("tests/panics.rs"),
        "panic location was {file}"
    );
    assert!(
        message.contains("empty range 5..5"),
        "message was {message}"
    );
    assert!(message.contains("try_range"), "message was {message}");
}
