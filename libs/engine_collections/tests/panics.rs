//! Documented panics: location (`#[track_caller]`) and message quality.
//!
//! Panics abort on WASI, so these tests run on native targets only.
#![forbid(unsafe_code)]
#![cfg(not(target_os = "wasi"))]

use std::panic::{self, AssertUnwindSafe};
use std::sync::Mutex;

use engine_collections::IndexMap;

static CAPTURED: Mutex<Option<(String, String)>> = Mutex::new(None);

/// The panic hook is process-wide, so tests that install one must not overlap.
static HOOK_LOCK: Mutex<()> = Mutex::new(());

#[test]
fn index_missing_key_panics_at_caller() {
    let map = IndexMap::<u8, u8>::new();
    let (file, message) = panic_info(|| {
        let _ = map[&1];
    });
    assert!(
        file.ends_with("tests/panics.rs"),
        "panic location was {file}"
    );
    assert!(
        message.contains("no entry for this key"),
        "message was {message}"
    );
    assert!(message.contains("use get"), "message was {message}");
}

#[test]
fn index_mut_missing_key_panics_at_caller() {
    let mut map = IndexMap::<u8, u8>::new();
    let (file, message) = panic_info(|| {
        map[&1] = 5;
    });
    assert!(
        file.ends_with("tests/panics.rs"),
        "panic location was {file}"
    );
    assert!(
        message.contains("no entry for this key"),
        "message was {message}"
    );
    assert!(message.contains("use get"), "message was {message}");
}

/// Runs `f`, which must panic, and returns where it panicked and what it said.
fn panic_info(f: impl FnOnce()) -> (String, String) {
    let _serial = HOOK_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    panic::set_hook(Box::new(|info| {
        let file = info
            .location()
            .map_or_default(|l| l.file().replace('\\', "/"));
        let message = info.payload_as_str().unwrap_or_default().to_string();
        *CAPTURED.lock().expect("captured") = Some((file, message));
    }));
    let result = panic::catch_unwind(AssertUnwindSafe(f));
    drop(panic::take_hook());
    assert!(result.is_err(), "expected a panic");
    CAPTURED
        .lock()
        .expect("captured")
        .take()
        .expect("the panic hook did not run")
}
