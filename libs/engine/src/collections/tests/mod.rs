//! Unit tests that need crate-private access (the swappable hasher).

mod bit_array_model;
mod bit_set_model;
mod golden;
mod map_model;
mod set_model;

/// Collects at most `limit` items.
///
/// Mutation testing replaces an iterator's `next` with a function that always returns `Some`.
/// An unbounded `collect` would then grow a `Vec` until the machine runs out of memory, instead
/// of failing an assertion. Pass one more than the expected length, so an iterator that is too
/// long still compares unequal.
pub(crate) fn bounded<I: Iterator>(iter: I, limit: usize) -> std::vec::Vec<I::Item> {
    iter.take(limit).collect()
}

/// Formats `value` with `Debug`, stopping once the text passes `limit` bytes.
///
/// `Debug` for a set walks its iterator, so an iterator that never ends (see [`bounded`]) would
/// build an endless `String`. Past the limit the text is cut short, so the comparison fails.
pub(crate) fn debug_bounded(value: &dyn core::fmt::Debug, limit: usize) -> std::string::String {
    use core::fmt::Write;

    struct Limited {
        text: std::string::String,
        limit: usize,
    }
    impl Write for Limited {
        fn write_str(&mut self, part: &str) -> core::fmt::Result {
            if self.text.len() + part.len() > self.limit {
                return Err(core::fmt::Error);
            }
            self.text.push_str(part);
            Ok(())
        }
    }

    let mut out = Limited {
        text: std::string::String::new(),
        limit,
    };
    let _ = write!(out, "{value:?}"); // an error here only means the limit was reached
    out.text
}
