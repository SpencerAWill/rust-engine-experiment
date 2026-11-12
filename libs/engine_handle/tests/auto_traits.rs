//! Auto-trait and size guarantees for `Handle`.
#![forbid(unsafe_code)]

use engine_handle::Handle;

#[test]
fn handle_is_send_sync_copy_even_for_non_send_t() {
    fn assert_traits<T: Send + Sync + Copy + Eq + Ord + core::hash::Hash>() {}
    assert_traits::<Handle<std::rc::Rc<u8>>>();
    assert_eq!(size_of::<Handle<u8>>(), 8);
    assert_eq!(size_of::<Option<Handle<u8>>>(), 8);
}
