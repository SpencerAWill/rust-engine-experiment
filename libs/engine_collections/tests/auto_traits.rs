//! `Send`, `Sync` and `Unpin` whenever the element types are (spec §4.6).
#![forbid(unsafe_code)]

use engine_collections::index_map::{
    Entry, IntoIter, IntoKeys, IntoValues, Iter, IterMut, Keys, Values, ValuesMut,
};
use engine_collections::{IndexMap, IndexSet, index_set};

fn assert_traits<T: Send + Sync + Unpin>() {}

#[test]
fn map_and_its_iterators_are_send_sync_unpin() {
    assert_traits::<IndexMap<u32, String>>();
    assert_traits::<Iter<'static, u32, String>>();
    assert_traits::<IterMut<'static, u32, String>>();
    assert_traits::<Keys<'static, u32, String>>();
    assert_traits::<Values<'static, u32, String>>();
    assert_traits::<ValuesMut<'static, u32, String>>();
    assert_traits::<IntoIter<u32, String>>();
    assert_traits::<IntoKeys<u32, String>>();
    assert_traits::<IntoValues<u32, String>>();
    assert_traits::<Entry<'static, u32, String>>();
}

#[test]
fn set_and_its_iterators_are_send_sync_unpin() {
    assert_traits::<IndexSet<String>>();
    assert_traits::<index_set::Iter<'static, String>>();
    assert_traits::<index_set::IntoIter<String>>();
}
