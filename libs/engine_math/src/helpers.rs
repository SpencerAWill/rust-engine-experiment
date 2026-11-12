//! Small `f32` scalar helpers.
//!
//! These are `f32`-only in M1; `f64` variants are added when something needs them.

/// Linear interpolation: `a + (b - a) * t`. `t` is **not** clamped, so values outside `[0, 1]`
/// extrapolate.
///
/// # Examples
///
/// ```
/// use engine_math::lerp;
///
/// assert_eq!(lerp(0.0, 10.0, 0.25), 2.5);
/// assert_eq!(lerp(0.0, 10.0, 1.5), 15.0); // extrapolates
/// ```
#[inline]
pub fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// The `t` for which `lerp(a, b, t) == v`: `(v - a) / (b - a)`.
///
/// When `a == b` the result is `NaN` (documented, not a panic).
///
/// # Examples
///
/// ```
/// use engine_math::inverse_lerp;
///
/// assert_eq!(inverse_lerp(0.0, 10.0, 2.5), 0.25);
/// assert!(inverse_lerp(1.0, 1.0, 1.0).is_nan());
/// ```
#[inline]
pub fn inverse_lerp(a: f32, b: f32, v: f32) -> f32 {
    (v - a) / (b - a)
}

/// Maps `v` from the range `from` to the range `to`, preserving its relative position.
/// Like [`lerp`], it does not clamp.
///
/// # Examples
///
/// ```
/// use engine_math::remap;
///
/// assert_eq!(remap(5.0, (0.0, 10.0), (100.0, 200.0)), 150.0);
/// ```
#[inline]
pub fn remap(v: f32, from: (f32, f32), to: (f32, f32)) -> f32 {
    lerp(to.0, to.1, inverse_lerp(from.0, from.1, v))
}

/// Whether `a` and `b` differ by at most `eps`: `(a - b).abs() <= eps`. Always `false` if
/// either is `NaN`.
///
/// # Examples
///
/// ```
/// use engine_math::approx_eq;
///
/// assert!(approx_eq(1.0, 1.0 + 1e-7, 1e-6));
/// assert!(!approx_eq(1.0, 1.1, 1e-6));
/// ```
#[inline]
pub fn approx_eq(a: f32, b: f32, eps: f32) -> bool {
    (a - b).abs() <= eps
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn helpers() {
        assert_eq!(lerp(0.0, 10.0, 0.25).to_bits(), 2.5_f32.to_bits());
        assert_eq!(inverse_lerp(0.0, 10.0, 2.5).to_bits(), 0.25_f32.to_bits());
        assert!(inverse_lerp(1.0, 1.0, 1.0).is_nan());
        assert_eq!(
            remap(5.0, (0.0, 10.0), (100.0, 200.0)).to_bits(),
            150.0_f32.to_bits()
        );
        assert!(approx_eq(1.0, 1.0 + 1e-7, 1e-6) && !approx_eq(1.0, 1.1, 1e-6));
    }

    #[test]
    fn lerp_is_not_clamped_and_approx_eq_handles_nan() {
        assert_eq!(lerp(0.0, 10.0, 1.5).to_bits(), 15.0_f32.to_bits());
        assert_eq!(lerp(0.0, 10.0, -0.5).to_bits(), (-5.0_f32).to_bits());
        assert!(!approx_eq(f32::NAN, f32::NAN, 1.0));
    }
}
