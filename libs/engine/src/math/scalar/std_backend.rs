//! The `std` backend of [`scalar`](super): each function calls the inherent `f32` or `f64` method,
//! so results are bit-identical to `std`'s. It has the same shape as `libm_backend`, which a
//! build without `std` uses instead.
#![expect(
    clippy::disallowed_methods,
    reason = "engine::math::scalar is the sanctioned transcendental funnel (ADR 0003)"
)]

/// The `f32` operations, from `std`.
pub(super) mod single {
    #[inline]
    pub(crate) fn sin(x: f32) -> f32 {
        x.sin()
    }
    #[inline]
    pub(crate) fn cos(x: f32) -> f32 {
        x.cos()
    }
    #[inline]
    pub(crate) fn tan(x: f32) -> f32 {
        x.tan()
    }
    #[inline]
    pub(crate) fn asin(x: f32) -> f32 {
        x.asin()
    }
    #[inline]
    pub(crate) fn acos(x: f32) -> f32 {
        x.acos()
    }
    #[inline]
    pub(crate) fn atan(x: f32) -> f32 {
        x.atan()
    }
    #[inline]
    pub(crate) fn sinh(x: f32) -> f32 {
        x.sinh()
    }
    #[inline]
    pub(crate) fn cosh(x: f32) -> f32 {
        x.cosh()
    }
    #[inline]
    pub(crate) fn tanh(x: f32) -> f32 {
        x.tanh()
    }
    #[inline]
    pub(crate) fn asinh(x: f32) -> f32 {
        x.asinh()
    }
    #[inline]
    pub(crate) fn acosh(x: f32) -> f32 {
        x.acosh()
    }
    #[inline]
    pub(crate) fn atanh(x: f32) -> f32 {
        x.atanh()
    }
    #[inline]
    pub(crate) fn exp(x: f32) -> f32 {
        x.exp()
    }
    #[inline]
    pub(crate) fn exp2(x: f32) -> f32 {
        x.exp2()
    }
    #[inline]
    pub(crate) fn exp_m1(x: f32) -> f32 {
        x.exp_m1()
    }
    #[inline]
    pub(crate) fn ln(x: f32) -> f32 {
        x.ln()
    }
    #[inline]
    pub(crate) fn ln_1p(x: f32) -> f32 {
        x.ln_1p()
    }
    #[inline]
    pub(crate) fn log2(x: f32) -> f32 {
        x.log2()
    }
    #[inline]
    pub(crate) fn log10(x: f32) -> f32 {
        x.log10()
    }
    #[inline]
    pub(crate) fn cbrt(x: f32) -> f32 {
        x.cbrt()
    }
    #[inline]
    pub(crate) fn sin_cos(x: f32) -> (f32, f32) {
        x.sin_cos()
    }
    #[inline]
    pub(crate) fn atan2(y: f32, x: f32) -> f32 {
        y.atan2(x)
    }
    #[inline]
    pub(crate) fn hypot(x: f32, y: f32) -> f32 {
        x.hypot(y)
    }
    #[inline]
    pub(crate) fn powf(x: f32, y: f32) -> f32 {
        x.powf(y)
    }
    #[inline]
    pub(crate) fn powi(x: f32, n: i32) -> f32 {
        x.powi(n)
    }
    #[inline]
    pub(crate) fn log(x: f32, base: f32) -> f32 {
        x.log(base)
    }
}

/// The `f64` operations, from `std`.
pub(super) mod double {
    #[inline]
    pub(crate) fn sin(x: f64) -> f64 {
        x.sin()
    }
    #[inline]
    pub(crate) fn cos(x: f64) -> f64 {
        x.cos()
    }
    #[inline]
    pub(crate) fn tan(x: f64) -> f64 {
        x.tan()
    }
    #[inline]
    pub(crate) fn asin(x: f64) -> f64 {
        x.asin()
    }
    #[inline]
    pub(crate) fn acos(x: f64) -> f64 {
        x.acos()
    }
    #[inline]
    pub(crate) fn atan(x: f64) -> f64 {
        x.atan()
    }
    #[inline]
    pub(crate) fn sinh(x: f64) -> f64 {
        x.sinh()
    }
    #[inline]
    pub(crate) fn cosh(x: f64) -> f64 {
        x.cosh()
    }
    #[inline]
    pub(crate) fn tanh(x: f64) -> f64 {
        x.tanh()
    }
    #[inline]
    pub(crate) fn asinh(x: f64) -> f64 {
        x.asinh()
    }
    #[inline]
    pub(crate) fn acosh(x: f64) -> f64 {
        x.acosh()
    }
    #[inline]
    pub(crate) fn atanh(x: f64) -> f64 {
        x.atanh()
    }
    #[inline]
    pub(crate) fn exp(x: f64) -> f64 {
        x.exp()
    }
    #[inline]
    pub(crate) fn exp2(x: f64) -> f64 {
        x.exp2()
    }
    #[inline]
    pub(crate) fn exp_m1(x: f64) -> f64 {
        x.exp_m1()
    }
    #[inline]
    pub(crate) fn ln(x: f64) -> f64 {
        x.ln()
    }
    #[inline]
    pub(crate) fn ln_1p(x: f64) -> f64 {
        x.ln_1p()
    }
    #[inline]
    pub(crate) fn log2(x: f64) -> f64 {
        x.log2()
    }
    #[inline]
    pub(crate) fn log10(x: f64) -> f64 {
        x.log10()
    }
    #[inline]
    pub(crate) fn cbrt(x: f64) -> f64 {
        x.cbrt()
    }
    #[inline]
    pub(crate) fn sin_cos(x: f64) -> (f64, f64) {
        x.sin_cos()
    }
    #[inline]
    pub(crate) fn atan2(y: f64, x: f64) -> f64 {
        y.atan2(x)
    }
    #[inline]
    pub(crate) fn hypot(x: f64, y: f64) -> f64 {
        x.hypot(y)
    }
    #[inline]
    pub(crate) fn powf(x: f64, y: f64) -> f64 {
        x.powf(y)
    }
    #[inline]
    pub(crate) fn powi(x: f64, n: i32) -> f64 {
        x.powi(n)
    }
    #[inline]
    pub(crate) fn log(x: f64, base: f64) -> f64 {
        x.log(base)
    }
}
