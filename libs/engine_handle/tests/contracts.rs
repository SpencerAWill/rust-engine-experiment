//! Behaviour that callers rely on and that only doctests used to check: emptiness, capacity,
//! cloning, hashing, `get_mut`, exact iterator sizes and the readable `Debug` forms.
#![forbid(unsafe_code)]

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use engine_handle::{DenseHandleAllocator, Handle, HandleAllocator, HandlePool};

fn hash_of(handle: Handle<u8>) -> u64 {
    let mut hasher = DefaultHasher::new();
    handle.hash(&mut hasher);
    hasher.finish()
}

#[test]
fn emptiness_follows_the_live_count() {
    let mut pool = HandlePool::new();
    assert!(pool.is_empty());
    let h = pool.insert(1_u8);
    assert!(!pool.is_empty());
    pool.remove(h);
    assert!(pool.is_empty());

    let mut alloc = HandleAllocator::<u8>::new();
    assert!(alloc.is_empty());
    let h = alloc.allocate();
    assert!(!alloc.is_empty());
    assert!(alloc.free(h));
    assert!(alloc.is_empty());

    let mut dense = DenseHandleAllocator::<u8>::new();
    assert!(dense.is_empty());
    let h = dense.allocate();
    assert!(!dense.is_empty());
    assert!(dense.free(h));
    assert!(dense.is_empty());
}

#[test]
fn with_capacity_reserves_room_for_the_values() {
    let pool = HandlePool::<u64>::with_capacity(32);
    assert!(pool.capacity() >= 32);
    assert!(pool.is_empty());
}

#[test]
fn a_clone_is_independent_and_continues_identically() {
    let mut alloc = HandleAllocator::<u8>::new();
    let hs = alloc.allocate_many(4);
    assert!(alloc.free(hs[1]));
    let mut copy = alloc.clone();
    assert_eq!(copy.len(), alloc.len());
    assert!(hs.iter().all(|h| copy.is_alive(*h) == alloc.is_alive(*h)));
    // Both hand out the same handles from here on, and neither affects the other.
    assert_eq!(copy.allocate(), alloc.allocate());
    let _ = copy.allocate();
    assert_eq!(copy.len(), alloc.len() + 1);

    let mut dense = DenseHandleAllocator::<u8>::new();
    let hs: Vec<_> = (0..4).map(|_| dense.allocate()).collect();
    assert!(dense.free(hs[2]));
    let mut copy = dense.clone();
    assert_eq!(copy.len(), dense.len());
    assert!(hs.iter().all(|h| copy.is_alive(*h) == dense.is_alive(*h)));
    assert_eq!(
        copy.iter().collect::<Vec<_>>(),
        dense.iter().collect::<Vec<_>>()
    );
    assert_eq!(copy.allocate(), dense.allocate());
    let _ = copy.allocate();
    assert_eq!(copy.len(), dense.len() + 1);
}

#[test]
fn hashing_distinguishes_handles_and_agrees_for_equal_ones() {
    let mut pool = HandlePool::new();
    let a = pool.insert(0_u8);
    let b = pool.insert(0_u8);
    assert_ne!(a, b);
    assert_ne!(hash_of(a), hash_of(b));
    assert_eq!(hash_of(a), hash_of(a));
    let again = Handle::<u8>::from_bits(a.to_bits()).unwrap();
    assert_eq!(hash_of(a), hash_of(again));
}

#[test]
fn get_mut_changes_the_stored_value_and_rejects_stale_handles() {
    let mut pool = HandlePool::new();
    let h = pool.insert(10_u32);
    *pool.get_mut(h).unwrap() += 5;
    assert_eq!(pool.get(h), Some(&15));
    pool.remove(h);
    assert!(pool.get_mut(h).is_none());
}

#[test]
fn iterators_report_exact_sizes_as_they_advance() {
    let mut pool = HandlePool::new();
    for i in 0..3_u8 {
        let _ = pool.insert(i);
    }
    {
        let mut iter = pool.iter_mut();
        assert_eq!(iter.size_hint(), (3, Some(3)));
        assert_eq!(iter.len(), 3);
        let _ = iter.next();
        assert_eq!(iter.size_hint(), (2, Some(2)));
    }

    let mut into = pool.into_iter();
    assert_eq!(into.size_hint(), (3, Some(3)));
    let _ = into.next();
    assert_eq!(into.size_hint(), (2, Some(2)));
    assert_eq!(into.by_ref().count(), 2);
    assert_eq!(into.size_hint(), (0, Some(0)));
}

#[test]
fn iterator_debug_output_shows_the_remaining_count() {
    let mut pool = HandlePool::new();
    for i in 0..3_u8 {
        let _ = pool.insert(i);
    }
    let mut into = pool.into_iter();
    let _ = into.next();
    assert_eq!(format!("{into:?}"), "IntoIter { remaining: 2, .. }");

    let mut dense = DenseHandleAllocator::<u8>::new();
    for _ in 0..3 {
        let _ = dense.allocate();
    }
    let mut iter = dense.iter();
    assert_eq!(format!("{iter:?}"), "DenseAllocIter { remaining: 3, .. }");
    let _ = iter.next();
    assert_eq!(format!("{iter:?}"), "DenseAllocIter { remaining: 2, .. }");
}
