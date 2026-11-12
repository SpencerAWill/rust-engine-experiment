//! Compile-time layout assertions.
//!
//! `const` assertions pin `size_of` and `align_of` of the transform types (added with the
//! types themselves, in later tasks), so that anything that silently changes their layout, such
//! as a glam `scalar-math` feature flip through Cargo feature unification or a glam upgrade,
//! fails the build loudly instead of changing snapshot or FFI layouts (ADR 0003, ADR 0006).

use core::mem::{align_of, size_of};

use crate::{Transform, WorldTransform};

// Measured on aarch64 (native) and wasm32; identical because glam keeps `Quat` 16-byte aligned
// on every target unless its `scalar-math` feature is on. A mismatch here means a glam feature
// flip through Cargo feature unification, or a glam upgrade: see ADR 0006 before changing these.
const _: () = {
    assert!(
        size_of::<Transform>() == 48,
        "Transform size changed (glam scalar-math? see ADR 0006)"
    );
    assert!(
        align_of::<Transform>() == 16,
        "Transform alignment changed (glam scalar-math? see ADR 0006)"
    );
    assert!(
        size_of::<WorldTransform>() == 64,
        "WorldTransform size changed (glam scalar-math? see ADR 0006)"
    );
    assert!(
        align_of::<WorldTransform>() == 16,
        "WorldTransform alignment changed (glam scalar-math? see ADR 0006)"
    );
};

#[cfg(test)]
mod tests {
    use crate::{Transform, WorldTransform};
    use core::mem::{align_of, size_of};

    /// Prints the layout of every transform type, so CI logs show the values per target
    /// (`cargo test -- --nocapture`). The values are then pinned by the assertions above.
    #[test]
    #[allow(clippy::print_stdout, reason = "the output is the point of this test")]
    fn print_layouts() {
        println!("layout arch={}", std::env::consts::ARCH);
        println!(
            "layout Transform size={} align={}",
            size_of::<Transform>(),
            align_of::<Transform>()
        );
        println!(
            "layout WorldTransform size={} align={}",
            size_of::<WorldTransform>(),
            align_of::<WorldTransform>()
        );
    }
}
