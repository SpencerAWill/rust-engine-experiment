//! Fuzzes `GenId::from_bits`, `UntypedGenId::from_bits` and `to_bits` against the layout the
//! documentation promises.

#![no_main]
#![forbid(unsafe_code)]

use engine_genid::{GenId, UntypedGenId};
use libfuzzer_sys::fuzz_target;

/// Checks every property that holds for a single bit pattern.
fn check_one(bits: u64) {
    let parsed = GenId::<u8>::from_bits(bits);

    // `from_bits` rejects exactly the patterns whose generation (low) half is zero.
    assert_eq!(parsed.is_none(), bits as u32 == 0, "bits {bits:#018x}");

    // The `TryFrom` conversion agrees, and its error carries the rejected bits.
    match GenId::<u8>::try_from(bits) {
        Ok(id) => assert_eq!(Some(id), parsed),
        Err(error) => {
            assert!(parsed.is_none());
            assert_eq!(error.bits(), bits);
        }
    }

    // The untyped core accepts exactly the same patterns and round-trips them.
    let untyped = UntypedGenId::from_bits(bits);
    assert_eq!(untyped, parsed.map(GenId::untyped));
    assert_eq!(untyped.is_none(), bits as u32 == 0, "bits {bits:#018x}");
    if let Some(core) = untyped {
        assert_eq!(core.to_bits(), bits);
        assert_eq!(u64::from(core), bits);
        assert_eq!(UntypedGenId::from_bits(core.to_bits()), Some(core));
        assert_eq!(GenId::<u8>::from_untyped(core).untyped(), core);
    }

    if let Some(id) = parsed {
        assert_eq!(id.to_bits(), bits);
        assert_eq!(u64::from(id), bits);
        assert_eq!(id.index(), (bits >> 32) as u32);
        assert_eq!(id.generation().get(), bits as u32);
        assert_eq!(GenId::<u8>::from_bits(id.to_bits()), Some(id));
    }
}

fuzz_target!(|input: (u64, u64)| {
    let (a, b) = input;
    check_one(a);
    check_one(b);

    // The documented ordering claim: ID order is bit order.
    if let (Some(ha), Some(hb)) = (GenId::<u8>::from_bits(a), GenId::<u8>::from_bits(b)) {
        assert_eq!(ha.cmp(&hb), a.cmp(&b));
        assert_eq!(ha == hb, a == b);
        assert_eq!(ha.untyped().cmp(&hb.untyped()), a.cmp(&b));
    }
});
