//! Documented panics: location (`#[track_caller]`) and message quality.
//!
//! Panics abort on WASI, so these tests run on native targets only (spec §4.7).
#![forbid(unsafe_code)]
#![cfg(not(target_os = "wasi"))]

use std::panic::{self, AssertUnwindSafe};
use std::sync::Mutex;

use engine_handle::{DenseHandleAllocator, HandleAllocator, HandlePool};

static CAPTURED: Mutex<Option<(String, String)>> = Mutex::new(None);

/// The panic hook is process-wide, so tests that install one must not overlap.
static HOOK_LOCK: Mutex<()> = Mutex::new(());

#[test]
fn index_on_stale_handle_panics_at_caller() {
    let mut pool = HandlePool::new();
    let stale = pool.insert(1_u8);
    pool.remove(stale);

    let (file, message) = panic_info(|| {
        let _ = pool[stale];
    });
    assert!(
        file.ends_with("tests/panics.rs"),
        "panic location was {file}"
    );
    assert!(message.contains("stale handle"), "message was {message}");
    assert!(message.contains("use get()"), "message was {message}");
}

/// Runs `f`, which must panic, and returns where it panicked and what it said.
fn panic_info(f: impl FnOnce()) -> (String, String) {
    let _serial = HOOK_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    panic::set_hook(Box::new(|info| {
        let file = info
            .location()
            .map(|l| l.file().replace('\\', "/"))
            .unwrap_or_default();
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

/// A named closure that is expected to panic.
type Case = Box<dyn FnOnce()>;

#[test]
fn reserve_overflow_panics_at_caller_with_an_actionable_message() {
    let cases: [(&str, Case); 3] = [
        (
            "HandlePool",
            Box::new(|| HandlePool::<u8>::new().reserve(usize::MAX)),
        ),
        (
            "HandleAllocator",
            Box::new(|| HandleAllocator::<u8>::new().reserve(usize::MAX)),
        ),
        (
            "DenseHandleAllocator",
            Box::new(|| DenseHandleAllocator::<u8>::new().reserve(usize::MAX)),
        ),
    ];
    for (name, case) in cases {
        let (file, message) = panic_info(case);
        assert!(
            file.ends_with("tests/panics.rs"),
            "{name}: panic location was {file}"
        );
        assert!(
            message.contains("try_reserve"),
            "{name}: message was {message}"
        );
    }
}
