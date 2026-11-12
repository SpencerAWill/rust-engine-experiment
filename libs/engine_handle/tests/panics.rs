//! Documented panics: location (`#[track_caller]`) and message quality.
//!
//! Panics abort on WASI, so these tests run on native targets only (spec §4.7).
#![forbid(unsafe_code)]
#![cfg(not(target_os = "wasi"))]

use std::panic::{self, AssertUnwindSafe};
use std::sync::Mutex;

use engine_handle::HandlePool;

static CAPTURED: Mutex<Option<(String, String)>> = Mutex::new(None);

#[test]
fn index_on_stale_handle_panics_at_caller() {
    let mut pool = HandlePool::new();
    let stale = pool.insert(1_u8);
    pool.remove(stale);

    panic::set_hook(Box::new(|info| {
        let file = info
            .location()
            .map(|l| l.file().replace('\\', "/"))
            .unwrap_or_default();
        let message = info.payload_as_str().unwrap_or_default().to_string();
        *CAPTURED.lock().unwrap() = Some((file, message));
    }));
    let result = panic::catch_unwind(AssertUnwindSafe(|| pool[stale]));
    drop(panic::take_hook());

    assert!(result.is_err());
    let (file, message) = CAPTURED.lock().unwrap().take().unwrap();
    assert!(
        file.ends_with("tests/panics.rs"),
        "panic location was {file}"
    );
    assert!(message.contains("stale handle"), "message was {message}");
    assert!(message.contains("use get()"), "message was {message}");
}
