//! Auto-trait and size guarantees for `GenId`.
#![forbid(unsafe_code)]

use engine_genid::GenId;

#[test]
fn id_is_send_sync_copy_even_for_non_send_t() {
    fn assert_traits<T: Send + Sync + Copy + Eq + Ord + core::hash::Hash>() {}
    assert_traits::<GenId<std::rc::Rc<u8>>>();
    assert_eq!(size_of::<GenId<u8>>(), 8);
    assert_eq!(size_of::<Option<GenId<u8>>>(), 8);
}
