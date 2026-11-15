//! `f32` operations that `core` lacks and `std` provides, written to give `std`'s exact results.
//!
//! None of these is transcendental, so none goes through the `libm` backend of
//! [`scalar`](super::scalar). Each is bit-identical to the `std` method of the same name, which a
//! test pins.

/// The Euclidean remainder of `a / b`: always in `[0, |b|)` for finite `a` and non-zero `b`.
///
/// This is `f32::rem_euclid`, which `core` lacks.
#[inline]
pub(super) fn rem_euclid(a: f32, b: f32) -> f32 {
    let r = a % b;
    if r < 0.0 { r + b.abs() } else { r }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Finite values of both signs, the zeros, the infinities and a NaN.
    const SAMPLES: [f32; 14] = [
        -2.5,
        -1.0,
        -0.5,
        -0.0,
        0.0,
        0.25,
        0.5,
        1.0,
        2.0,
        3.0,
        f32::NAN,
        f32::INFINITY,
        f32::NEG_INFINITY,
        f32::MAX,
    ];

    #[test]
    fn rem_euclid_is_bit_identical_to_std() {
        for a in SAMPLES {
            for b in SAMPLES {
                assert_eq!(
                    rem_euclid(a, b).to_bits(),
                    a.rem_euclid(b).to_bits(),
                    "rem_euclid({a}, {b})"
                );
            }
        }
    }
}
