//! Fuzzes `Handle::from_bits` and `to_bits` against the layout the documentation promises.

#![no_main]
#![forbid(unsafe_code)]

use engine_handle::Handle;
use libfuzzer_sys::fuzz_target;

/// Checks every property that holds for a single bit pattern.
fn check_one(bits: u64) {
    let parsed = Handle::<u8>::from_bits(bits);

    // `from_bits` rejects exactly the patterns whose generation (low) half is zero.
    assert_eq!(parsed.is_none(), bits as u32 == 0, "bits {bits:#018x}");

    // The `TryFrom` conversion agrees, and its error carries the rejected bits.
    match Handle::<u8>::try_from(bits) {
        Ok(handle) => assert_eq!(Some(handle), parsed),
        Err(error) => {
            assert!(parsed.is_none());
            assert_eq!(error.bits(), bits);
        }
    }

    if let Some(handle) = parsed {
        assert_eq!(handle.to_bits(), bits);
        assert_eq!(u64::from(handle), bits);
        assert_eq!(handle.index(), (bits >> 32) as u32);
        assert_eq!(handle.generation().get(), bits as u32);
        assert_eq!(Handle::<u8>::from_bits(handle.to_bits()), Some(handle));
    }
}

fuzz_target!(|input: (u64, u64)| {
    let (a, b) = input;
    check_one(a);
    check_one(b);

    // The documented ordering claim: handle order is bit order.
    if let (Some(ha), Some(hb)) = (Handle::<u8>::from_bits(a), Handle::<u8>::from_bits(b)) {
        assert_eq!(ha.cmp(&hb), a.cmp(&b));
        assert_eq!(ha == hb, a == b);
    }
});
