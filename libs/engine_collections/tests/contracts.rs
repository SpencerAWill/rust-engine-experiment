//! Public-API behaviour that the model and law tests do not observe: capacity, mutable access by
//! position, iterator sizes and `Debug` output. Each test names the mutant class it closes.
#![forbid(unsafe_code)]

use std::hash::BuildHasher;

use engine_collections::{IndexMap, IndexSet};
use foldhash::fast::FixedState;

fn hash_of<T: std::hash::Hash>(value: &T) -> u64 {
    FixedState::with_seed(0).hash_one(value)
}

#[test]
fn shrink_to_fit_releases_unused_capacity() {
    let mut map = IndexMap::<u8, u8>::with_capacity(100);
    map.insert(1, 1);
    map.shrink_to_fit();
    assert!(map.capacity() < 100);
    let mut set = IndexSet::<u8>::with_capacity(100);
    set.insert(1);
    set.shrink_to_fit();
    assert!(set.capacity() < 100);
}

#[test]
fn set_capacity_follows_reservations() {
    let mut set = IndexSet::<u32>::with_capacity(10);
    assert!(set.capacity() >= 10);
    set.reserve(100);
    assert!(set.capacity() >= 100);
    assert_eq!(IndexSet::<u32>::new().capacity(), 0);
}

#[test]
fn set_try_reserve_overflow_is_an_error_and_leaves_the_set_usable() {
    let mut set = IndexSet::<u32>::new();
    assert!(set.try_reserve(usize::MAX).is_err());
    assert!(set.try_reserve(4).is_ok());
    assert!(set.is_empty());
}

#[test]
fn positional_mutable_access_changes_the_stored_value() {
    let mut map: IndexMap<char, u32> = [('a', 1), ('b', 2), ('c', 3)].into();
    let (key, value) = map.get_index_mut(1).unwrap();
    assert_eq!(*key, 'b');
    *value = 20;
    let (key, value) = map.first_mut().unwrap();
    assert_eq!(*key, 'a');
    *value = 10;
    let (key, value) = map.last_mut().unwrap();
    assert_eq!(*key, 'c');
    *value = 30;
    assert_eq!(
        map.iter().map(|(_, v)| *v).collect::<Vec<_>>(),
        [10, 20, 30]
    );
    assert!(map.get_index_mut(3).is_none());
}

#[test]
fn set_hash_depends_on_length_and_contents() {
    let empty = IndexSet::<u8>::new();
    let one: IndexSet<u8> = [1].into();
    let other: IndexSet<u8> = [2].into();
    assert_ne!(hash_of(&empty), hash_of(&one));
    assert_ne!(hash_of(&one), hash_of(&other));
}

#[test]
fn set_iterators_report_exact_sizes_as_they_advance() {
    let set: IndexSet<u8> = [1, 2, 3].into();
    let mut iter = set.iter();
    assert_eq!(iter.size_hint(), (3, Some(3)));
    iter.next();
    assert_eq!(iter.size_hint(), (2, Some(2)));
    assert_eq!(iter.len(), 2);

    let mut owned = set.into_iter();
    assert_eq!(owned.size_hint(), (3, Some(3)));
    assert_eq!(owned.len(), 3);
    assert_eq!(owned.next(), Some(1));
    assert_eq!(owned.next(), Some(2));
    assert_eq!(owned.size_hint(), (1, Some(1)));
    assert_eq!(owned.len(), 1);
    assert_eq!(owned.next(), Some(3));
    assert_eq!(owned.next(), None);
}

#[test]
fn iterator_debug_output_shows_the_remaining_items() {
    let map: IndexMap<u8, char> = [(1, 'a'), (2, 'b')].into();
    assert_eq!(format!("{:?}", map.iter()), "[(1, 'a'), (2, 'b')]");
    assert_eq!(format!("{:?}", map.keys()), "[1, 2]");
    assert_eq!(format!("{:?}", map.values()), "['a', 'b']");
    let set: IndexSet<u8> = [1, 2].into();
    assert_eq!(format!("{:?}", set.iter()), "[1, 2]");
}

#[test]
fn owning_and_mutable_iterators_and_entries_have_nonempty_debug_output() {
    let mut map: IndexMap<u8, char> = [(1, 'a'), (2, 'b')].into();
    let shown = [
        format!("{:?}", map.iter_mut()),
        format!("{:?}", map.values_mut()),
        format!("{:?}", map.clone().into_iter()),
        format!("{:?}", map.clone().into_keys()),
        format!("{:?}", map.clone().into_values()),
        format!("{:?}", map.entry(1)),
        format!("{:?}", IndexSet::from([1, 2]).into_iter()),
    ];
    for text in shown {
        assert!(
            text.contains('1') || text.contains('a'),
            "debug output was {text:?}"
        );
    }
}
