//! Auto-trait and size guarantees for `GenId` and `UntypedGenId`.
#![forbid(unsafe_code)]

use std::fmt::Debug;
use std::panic::{RefUnwindSafe, UnwindSafe};

use engine::genid::{GenId, UntypedGenId};

#[test]
fn id_is_send_sync_copy_even_for_non_send_t() {
    fn assert_traits<T: Send + Sync + Copy + Eq + Ord + core::hash::Hash>() {}
    assert_traits::<GenId<std::rc::Rc<u8>>>();
    assert_traits::<UntypedGenId>();
    assert_eq!(size_of::<GenId<u8>>(), 8);
    assert_eq!(size_of::<Option<GenId<u8>>>(), 8);
    assert_eq!(size_of::<UntypedGenId>(), 8);
    assert_eq!(size_of::<Option<UntypedGenId>>(), 8);
}

#[test]
fn untyped_id_has_every_auto_trait() {
    fn assert_traits<T: Send + Sync + Unpin + UnwindSafe + RefUnwindSafe + Debug>() {}
    assert_traits::<UntypedGenId>();
}
