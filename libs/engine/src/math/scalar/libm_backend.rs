//! The `libm` backend of [`scalar`](super): the transcendental functions when `std` is off.
//!
//! One function per scalar operation and float width, in the module [`single`] (`f32`) or
//! [`double`] (`f64`). The module is always compiled, so a host test can check each mapping
//! against `std` (`libm_backend_agrees_with_std` in [`scalar`](super)); only a build without
//! `std` calls it.
#![expect(
    clippy::disallowed_methods,
    reason = "engine::math::scalar is the sanctioned transcendental funnel (ADR 0003)"
)]
#![cfg_attr(
    all(feature = "std", not(test)),
    allow(
        dead_code,
        reason = "the no_std backend; with std it is only exercised by the agreement test"
    )
)]

/// The `f32` operations, from `libm`.
pub(super) mod single {
    #[inline]
    pub(crate) fn sin(x: f32) -> f32 {
        libm::sinf(x)
    }
    #[inline]
    pub(crate) fn cos(x: f32) -> f32 {
        libm::cosf(x)
    }
    #[inline]
    pub(crate) fn tan(x: f32) -> f32 {
        libm::tanf(x)
    }
    #[inline]
    pub(crate) fn asin(x: f32) -> f32 {
        libm::asinf(x)
    }
    #[inline]
    pub(crate) fn acos(x: f32) -> f32 {
        libm::acosf(x)
    }
    #[inline]
    pub(crate) fn atan(x: f32) -> f32 {
        libm::atanf(x)
    }
    #[inline]
    pub(crate) fn sinh(x: f32) -> f32 {
        libm::sinhf(x)
    }
    #[inline]
    pub(crate) fn cosh(x: f32) -> f32 {
        libm::coshf(x)
    }
    #[inline]
    pub(crate) fn tanh(x: f32) -> f32 {
        libm::tanhf(x)
    }
    #[inline]
    pub(crate) fn asinh(x: f32) -> f32 {
        libm::asinhf(x)
    }
    #[inline]
    pub(crate) fn acosh(x: f32) -> f32 {
        libm::acoshf(x)
    }
    #[inline]
    pub(crate) fn atanh(x: f32) -> f32 {
        libm::atanhf(x)
    }
    #[inline]
    pub(crate) fn exp(x: f32) -> f32 {
        libm::expf(x)
    }
    #[inline]
    pub(crate) fn exp2(x: f32) -> f32 {
        libm::exp2f(x)
    }
    #[inline]
    pub(crate) fn exp_m1(x: f32) -> f32 {
        libm::expm1f(x)
    }
    #[inline]
    pub(crate) fn ln(x: f32) -> f32 {
        libm::logf(x)
    }
    #[inline]
    pub(crate) fn ln_1p(x: f32) -> f32 {
        libm::log1pf(x)
    }
    #[inline]
    pub(crate) fn log2(x: f32) -> f32 {
        libm::log2f(x)
    }
    #[inline]
    pub(crate) fn log10(x: f32) -> f32 {
        libm::log10f(x)
    }
    #[inline]
    pub(crate) fn cbrt(x: f32) -> f32 {
        libm::cbrtf(x)
    }
    #[inline]
    pub(crate) fn sin_cos(x: f32) -> (f32, f32) {
        libm::sincosf(x)
    }
    #[inline]
    pub(crate) fn atan2(x: f32, y: f32) -> f32 {
        libm::atan2f(x, y)
    }
    #[inline]
    pub(crate) fn hypot(x: f32, y: f32) -> f32 {
        libm::hypotf(x, y)
    }
    #[inline]
    pub(crate) fn powf(x: f32, y: f32) -> f32 {
        libm::powf(x, y)
    }
    /// `libm` has no `powi`: `powf` with the exponent converted to the float type.
    #[inline]
    #[allow(
        clippy::cast_precision_loss,
        reason = "the exponent is converted to the float type, as the docs say"
    )]
    pub(crate) fn powi(x: f32, n: i32) -> f32 {
        libm::powf(x, n as f32)
    }
    /// `ln(x) / ln(base)`.
    #[inline]
    pub(crate) fn log(x: f32, base: f32) -> f32 {
        libm::logf(x) / libm::logf(base)
    }
}

/// The `f64` operations, from `libm`.
pub(super) mod double {
    #[inline]
    pub(crate) fn sin(x: f64) -> f64 {
        libm::sin(x)
    }
    #[inline]
    pub(crate) fn cos(x: f64) -> f64 {
        libm::cos(x)
    }
    #[inline]
    pub(crate) fn tan(x: f64) -> f64 {
        libm::tan(x)
    }
    #[inline]
    pub(crate) fn asin(x: f64) -> f64 {
        libm::asin(x)
    }
    #[inline]
    pub(crate) fn acos(x: f64) -> f64 {
        libm::acos(x)
    }
    #[inline]
    pub(crate) fn atan(x: f64) -> f64 {
        libm::atan(x)
    }
    #[inline]
    pub(crate) fn sinh(x: f64) -> f64 {
        libm::sinh(x)
    }
    #[inline]
    pub(crate) fn cosh(x: f64) -> f64 {
        libm::cosh(x)
    }
    #[inline]
    pub(crate) fn tanh(x: f64) -> f64 {
        libm::tanh(x)
    }
    #[inline]
    pub(crate) fn asinh(x: f64) -> f64 {
        libm::asinh(x)
    }
    #[inline]
    pub(crate) fn acosh(x: f64) -> f64 {
        libm::acosh(x)
    }
    #[inline]
    pub(crate) fn atanh(x: f64) -> f64 {
        libm::atanh(x)
    }
    #[inline]
    pub(crate) fn exp(x: f64) -> f64 {
        libm::exp(x)
    }
    #[inline]
    pub(crate) fn exp2(x: f64) -> f64 {
        libm::exp2(x)
    }
    #[inline]
    pub(crate) fn exp_m1(x: f64) -> f64 {
        libm::expm1(x)
    }
    #[inline]
    pub(crate) fn ln(x: f64) -> f64 {
        libm::log(x)
    }
    #[inline]
    pub(crate) fn ln_1p(x: f64) -> f64 {
        libm::log1p(x)
    }
    #[inline]
    pub(crate) fn log2(x: f64) -> f64 {
        libm::log2(x)
    }
    #[inline]
    pub(crate) fn log10(x: f64) -> f64 {
        libm::log10(x)
    }
    #[inline]
    pub(crate) fn cbrt(x: f64) -> f64 {
        libm::cbrt(x)
    }
    #[inline]
    pub(crate) fn sin_cos(x: f64) -> (f64, f64) {
        libm::sincos(x)
    }
    #[inline]
    pub(crate) fn atan2(x: f64, y: f64) -> f64 {
        libm::atan2(x, y)
    }
    #[inline]
    pub(crate) fn hypot(x: f64, y: f64) -> f64 {
        libm::hypot(x, y)
    }
    #[inline]
    pub(crate) fn powf(x: f64, y: f64) -> f64 {
        libm::pow(x, y)
    }
    /// `libm` has no `powi`: `powf` with the exponent converted to the float type.
    #[inline]
    pub(crate) fn powi(x: f64, n: i32) -> f64 {
        libm::pow(x, f64::from(n))
    }
    /// `ln(x) / ln(base)`.
    #[inline]
    pub(crate) fn log(x: f64, base: f64) -> f64 {
        libm::log(x) / libm::log(base)
    }
}
