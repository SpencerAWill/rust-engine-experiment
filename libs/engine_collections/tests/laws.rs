//! Equality and hashing respect order (spec §4.6).
#![forbid(unsafe_code)]

use std::hash::BuildHasher;

use engine_collections::{IndexMap, IndexSet};
use foldhash::fast::FixedState;
use proptest::prelude::*;

fn hash_of<T: std::hash::Hash>(value: &T) -> u64 {
    FixedState::with_seed(0).hash_one(value)
}

fn entries() -> impl Strategy<Value = Vec<(u8, u16)>> {
    prop::collection::vec((any::<u8>(), any::<u16>()), 0..40)
}

proptest! {
    #[test]
    fn eq_is_reflexive_and_symmetric(a in entries(), b in entries()) {
        let a: IndexMap<u8, u16> = a.into_iter().collect();
        let b: IndexMap<u8, u16> = b.into_iter().collect();
        prop_assert!(a == a);
        prop_assert_eq!(a == b, b == a);
        prop_assert!(a == a.clone());
    }

    #[test]
    fn equal_maps_hash_equal(a in entries(), b in entries()) {
        let a: IndexMap<u8, u16> = a.into_iter().collect();
        let b: IndexMap<u8, u16> = b.into_iter().collect();
        prop_assert_eq!(hash_of(&a), hash_of(&a.clone()));
        if a == b {
            prop_assert_eq!(hash_of(&a), hash_of(&b));
        }
    }

    #[test]
    fn permutation_is_unequal_until_sorted(a in entries()) {
        let forward: IndexMap<u8, u16> = a.into_iter().collect();
        let mut reversed: IndexMap<u8, u16> =
            forward.iter().rev().map(|(k, v)| (*k, *v)).collect();
        if forward.len() >= 2 {
            prop_assert!(forward != reversed);
        }
        let mut forward = forward;
        forward.sort_keys();
        reversed.sort_keys();
        prop_assert!(forward == reversed);
        prop_assert_eq!(hash_of(&forward), hash_of(&reversed));
    }
}

#[test]
fn hash_depends_on_length_and_contents() {
    let empty = IndexMap::<u8, u8>::new();
    let one: IndexMap<u8, u8> = [(0, 0)].into();
    assert_ne!(hash_of(&empty), hash_of(&one));
}

fn items() -> impl Strategy<Value = Vec<u8>> {
    prop::collection::vec(any::<u8>(), 0..40)
}

proptest! {
    #[test]
    fn set_eq_is_reflexive_and_symmetric(a in items(), b in items()) {
        let a: IndexSet<u8> = a.into_iter().collect();
        let b: IndexSet<u8> = b.into_iter().collect();
        prop_assert!(a == a);
        prop_assert_eq!(a == b, b == a);
        prop_assert!(a == a.clone());
    }

    #[test]
    fn equal_sets_hash_equal(a in items(), b in items()) {
        let a: IndexSet<u8> = a.into_iter().collect();
        let b: IndexSet<u8> = b.into_iter().collect();
        prop_assert_eq!(hash_of(&a), hash_of(&a.clone()));
        if a == b {
            prop_assert_eq!(hash_of(&a), hash_of(&b));
        }
    }

    #[test]
    fn set_permutation_is_unequal_until_sorted(a in items()) {
        let forward: IndexSet<u8> = a.into_iter().collect();
        let mut reversed: IndexSet<u8> = forward.iter().rev().copied().collect();
        if forward.len() >= 2 {
            prop_assert!(forward != reversed);
        }
        let mut forward = forward;
        forward.sort();
        reversed.sort();
        prop_assert!(forward == reversed);
        prop_assert_eq!(hash_of(&forward), hash_of(&reversed));
    }
}

#[test]
fn guide_level_example_order_matters_until_sorted() {
    let mut ab: IndexSet<u32> = [1, 2].into();
    let mut ba: IndexSet<u32> = [2, 1].into();
    assert_ne!(ab, ba);
    ab.sort();
    ba.sort();
    assert_eq!(ab, ba);
}
