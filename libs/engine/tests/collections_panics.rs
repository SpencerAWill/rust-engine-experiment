//! Documented panics: location (`#[track_caller]`) and message quality.
//!
//! Panics abort on WASI, so these tests run on native targets only.
#![forbid(unsafe_code)]
#![cfg(not(target_os = "wasi"))]

use std::panic::{self, AssertUnwindSafe};
use std::sync::Mutex;

use engine::collections::{BitArray, BitSet, HierarchicalBitSet, IndexMap};

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
        file.ends_with("tests/collections_panics.rs"),
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
        file.ends_with("tests/collections_panics.rs"),
        "panic location was {file}"
    );
    assert!(
        message.contains("no entry for this key"),
        "message was {message}"
    );
    assert!(message.contains("use get"), "message was {message}");
}

#[test]
fn bit_array_insert_out_of_range_panics_at_caller() {
    let mut array = BitArray::<1>::new();
    let (file, message) = panic_info(|| {
        array.insert(64);
    });
    assert!(
        file.ends_with("tests/collections_panics.rs"),
        "panic location was {file}"
    );
    for part in ["bit index 64", "BitArray<1>", "use a larger WORDS"] {
        assert!(message.contains(part), "message was {message}");
    }
}

#[test]
fn bit_array_remove_out_of_range_panics_at_caller() {
    let mut array = BitArray::<1>::new();
    let (file, message) = panic_info(|| {
        array.remove(64);
    });
    assert!(
        file.ends_with("tests/collections_panics.rs"),
        "panic location was {file}"
    );
    for part in ["bit index 64", "BitArray<1>", "use a larger WORDS"] {
        assert!(message.contains(part), "message was {message}");
    }
}

#[test]
fn bit_set_insert_at_max_bits_panics_at_caller() {
    let mut set = BitSet::new();
    let (file, message) = panic_info(|| {
        set.insert(BitSet::MAX_BITS);
    });
    assert!(
        file.ends_with("tests/collections_panics.rs"),
        "panic location was {file}"
    );
    for part in ["MAX_BITS", "use try_insert"] {
        assert!(message.contains(part), "message was {message}");
    }
}

#[test]
fn bit_set_from_iter_at_max_bits_panics_at_caller() {
    let (file, message) = panic_info(|| {
        // `collect` is not `#[track_caller]`, so a panic through it reports a location in `core`.
        let _ = BitSet::from_iter([BitSet::MAX_BITS]);
    });
    assert!(
        file.ends_with("tests/collections_panics.rs"),
        "panic location was {file}"
    );
    assert!(message.contains("try_extend"), "message was {message}");
    assert!(
        message.contains("BitSet::from_iter:"),
        "message was {message}"
    );
}

#[test]
fn bit_set_from_array_at_max_bits_names_from() {
    let (file, message) = panic_info(|| {
        let _ = BitSet::from([BitSet::MAX_BITS]);
    });
    assert!(
        file.ends_with("tests/collections_panics.rs"),
        "panic location was {file}"
    );
    assert!(message.contains("BitSet::from:"), "message was {message}");
}

#[cfg(target_pointer_width = "64")]
#[test]
fn bit_set_with_capacity_past_max_bits_panics_at_caller() {
    let (file, message) = panic_info(|| {
        let _ = BitSet::with_capacity(BitSet::MAX_BITS + 1);
    });
    assert!(
        file.ends_with("tests/collections_panics.rs"),
        "panic location was {file}"
    );
    for part in ["MAX_BITS", "try_with_capacity"] {
        assert!(message.contains(part), "message was {message}");
    }
}

#[cfg(target_pointer_width = "64")]
#[test]
fn bit_set_from_words_past_max_bits_panics_at_caller() {
    // `vec!` of zeros is calloc'd, so its pages stay unmapped until touched.
    let mut words = vec![0_u64; 1 << 26];
    *words.last_mut().unwrap() = 1 << 63; // bit 2^32 - 1 = MAX_BITS
    let (file, message) = panic_info(|| {
        let _ = BitSet::from_words(&words);
    });
    assert!(
        file.ends_with("tests/collections_panics.rs"),
        "panic location was {file}"
    );
    assert!(message.contains("try_from_words"), "message was {message}");
}

#[test]
fn hierarchical_insert_at_max_bits_panics_at_caller() {
    let mut set = HierarchicalBitSet::new();
    let (file, message) = panic_info(|| {
        set.insert(HierarchicalBitSet::MAX_BITS);
    });
    assert!(
        file.ends_with("tests/collections_panics.rs"),
        "panic location was {file}"
    );
    for part in ["MAX_BITS", "use try_insert", "HierarchicalBitSet::insert:"] {
        assert!(message.contains(part), "message was {message}");
    }
}

#[test]
fn hierarchical_from_iter_at_max_bits_panics_at_caller() {
    let (file, message) = panic_info(|| {
        // `collect` is not `#[track_caller]`, so a panic through it reports a location in `core`.
        let _ = HierarchicalBitSet::from_iter([HierarchicalBitSet::MAX_BITS]);
    });
    assert!(
        file.ends_with("tests/collections_panics.rs"),
        "panic location was {file}"
    );
    for part in ["try_extend", "HierarchicalBitSet::from_iter:"] {
        assert!(message.contains(part), "message was {message}");
    }
}

#[cfg(target_pointer_width = "64")]
#[test]
fn hierarchical_with_capacity_past_max_bits_panics_at_caller() {
    let (file, message) = panic_info(|| {
        let _ = HierarchicalBitSet::with_capacity(HierarchicalBitSet::MAX_BITS + 1);
    });
    assert!(
        file.ends_with("tests/collections_panics.rs"),
        "panic location was {file}"
    );
    for part in ["MAX_BITS", "try_with_capacity"] {
        assert!(message.contains(part), "message was {message}");
    }
}

#[test]
fn join_new_with_no_required_sets_panics_at_caller() {
    let (file, message) = panic_info(|| {
        let _ = engine::collections::Join::new(&[], &[]);
    });
    assert!(
        file.ends_with("tests/collections_panics.rs"),
        "panic location was {file}"
    );
    for part in ["no required sets", "pass the set of live indices"] {
        assert!(message.contains(part), "message was {message}");
    }
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
