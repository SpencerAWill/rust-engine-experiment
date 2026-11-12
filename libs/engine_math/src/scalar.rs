//! The transcendental-math funnel.
//!
//! All transcendental math in the engine goes through these functions instead of the inherent
//! `f32`/`f64` methods, which `clippy.toml` bans (ADR 0003). In M1 they delegate to `std`, so
//! results are deterministic for a given build but may differ *across* platforms. Because every
//! call site already goes through here, moving to cross-platform determinism later is a
//! contained change: back this module with the pure-Rust `libm` crate (and enable glam's `libm`
//! and `scalar-math` features, as one deliberate, versioned switch; see ADR 0006).
//!
//! The functions are generic over the sealed [`Float`] trait (`f32` and `f64`), so the type is
//! inferred: `scalar::sin(x)`.
//!
//! Correctly-rounded IEEE operations (`+ - * /`, `sqrt`, `mul_add`, `abs`, `floor`, `ceil`,
//! `round`, `trunc`, `min`, `max`, `rem_euclid`, `copysign`) are not transcendental and stay
//! available as ordinary methods.
#![expect(
    clippy::disallowed_methods,
    reason = "engine_math::scalar is the sanctioned transcendental funnel (ADR 0003)"
)]

mod sealed {
    /// Prevents implementations outside this crate.
    pub trait Sealed {}

    /// An argument no code outside this crate can construct, so the trait's methods cannot be
    /// called from outside; use the free functions in [`scalar`](super).
    #[derive(Debug)]
    pub struct Token(pub(super) ());
}

use sealed::Token;

/// A floating-point type the funnel supports: `f32` or `f64`. Sealed.
///
/// # Examples
///
/// ```
/// use engine_math::scalar;
///
/// fn hypotenuse<T: scalar::Float>(a: T, b: T) -> T {
///     scalar::hypot(a, b)
/// }
/// assert_eq!(hypotenuse(3.0_f32, 4.0), 5.0);
/// assert_eq!(hypotenuse(6.0_f64, 8.0), 10.0);
/// ```
pub trait Float: sealed::Sealed + Copy {
    #[doc(hidden)]
    #[must_use]
    fn sin(self, token: Token) -> Self;
    #[doc(hidden)]
    #[must_use]
    fn cos(self, token: Token) -> Self;
    #[doc(hidden)]
    #[must_use]
    fn tan(self, token: Token) -> Self;
    #[doc(hidden)]
    #[must_use]
    fn asin(self, token: Token) -> Self;
    #[doc(hidden)]
    #[must_use]
    fn acos(self, token: Token) -> Self;
    #[doc(hidden)]
    #[must_use]
    fn atan(self, token: Token) -> Self;
    #[doc(hidden)]
    #[must_use]
    fn sinh(self, token: Token) -> Self;
    #[doc(hidden)]
    #[must_use]
    fn cosh(self, token: Token) -> Self;
    #[doc(hidden)]
    #[must_use]
    fn tanh(self, token: Token) -> Self;
    #[doc(hidden)]
    #[must_use]
    fn asinh(self, token: Token) -> Self;
    #[doc(hidden)]
    #[must_use]
    fn acosh(self, token: Token) -> Self;
    #[doc(hidden)]
    #[must_use]
    fn atanh(self, token: Token) -> Self;
    #[doc(hidden)]
    #[must_use]
    fn exp(self, token: Token) -> Self;
    #[doc(hidden)]
    #[must_use]
    fn exp2(self, token: Token) -> Self;
    #[doc(hidden)]
    #[must_use]
    fn exp_m1(self, token: Token) -> Self;
    #[doc(hidden)]
    #[must_use]
    fn ln(self, token: Token) -> Self;
    #[doc(hidden)]
    #[must_use]
    fn ln_1p(self, token: Token) -> Self;
    #[doc(hidden)]
    #[must_use]
    fn log2(self, token: Token) -> Self;
    #[doc(hidden)]
    #[must_use]
    fn log10(self, token: Token) -> Self;
    #[doc(hidden)]
    #[must_use]
    fn cbrt(self, token: Token) -> Self;
    #[doc(hidden)]
    #[must_use]
    fn sin_cos(self, token: Token) -> (Self, Self);
    #[doc(hidden)]
    #[must_use]
    fn atan2(self, other: Self, token: Token) -> Self;
    #[doc(hidden)]
    #[must_use]
    fn hypot(self, other: Self, token: Token) -> Self;
    #[doc(hidden)]
    #[must_use]
    fn powf(self, exponent: Self, token: Token) -> Self;
    #[doc(hidden)]
    #[must_use]
    fn powi(self, exponent: i32, token: Token) -> Self;
    #[doc(hidden)]
    #[must_use]
    fn log(self, base: Self, token: Token) -> Self;
}

impl sealed::Sealed for f32 {}

impl Float for f32 {
    #[inline]
    fn sin(self, _: Token) -> Self {
        f32::sin(self)
    }
    #[inline]
    fn cos(self, _: Token) -> Self {
        f32::cos(self)
    }
    #[inline]
    fn tan(self, _: Token) -> Self {
        f32::tan(self)
    }
    #[inline]
    fn asin(self, _: Token) -> Self {
        f32::asin(self)
    }
    #[inline]
    fn acos(self, _: Token) -> Self {
        f32::acos(self)
    }
    #[inline]
    fn atan(self, _: Token) -> Self {
        f32::atan(self)
    }
    #[inline]
    fn sinh(self, _: Token) -> Self {
        f32::sinh(self)
    }
    #[inline]
    fn cosh(self, _: Token) -> Self {
        f32::cosh(self)
    }
    #[inline]
    fn tanh(self, _: Token) -> Self {
        f32::tanh(self)
    }
    #[inline]
    fn asinh(self, _: Token) -> Self {
        f32::asinh(self)
    }
    #[inline]
    fn acosh(self, _: Token) -> Self {
        f32::acosh(self)
    }
    #[inline]
    fn atanh(self, _: Token) -> Self {
        f32::atanh(self)
    }
    #[inline]
    fn exp(self, _: Token) -> Self {
        f32::exp(self)
    }
    #[inline]
    fn exp2(self, _: Token) -> Self {
        f32::exp2(self)
    }
    #[inline]
    fn exp_m1(self, _: Token) -> Self {
        f32::exp_m1(self)
    }
    #[inline]
    fn ln(self, _: Token) -> Self {
        f32::ln(self)
    }
    #[inline]
    fn ln_1p(self, _: Token) -> Self {
        f32::ln_1p(self)
    }
    #[inline]
    fn log2(self, _: Token) -> Self {
        f32::log2(self)
    }
    #[inline]
    fn log10(self, _: Token) -> Self {
        f32::log10(self)
    }
    #[inline]
    fn cbrt(self, _: Token) -> Self {
        f32::cbrt(self)
    }
    #[inline]
    fn sin_cos(self, _: Token) -> (Self, Self) {
        f32::sin_cos(self)
    }
    #[inline]
    fn atan2(self, other: Self, _: Token) -> Self {
        f32::atan2(self, other)
    }
    #[inline]
    fn hypot(self, other: Self, _: Token) -> Self {
        f32::hypot(self, other)
    }
    #[inline]
    fn powf(self, exponent: Self, _: Token) -> Self {
        f32::powf(self, exponent)
    }
    #[inline]
    fn powi(self, exponent: i32, _: Token) -> Self {
        f32::powi(self, exponent)
    }
    #[inline]
    fn log(self, base: Self, _: Token) -> Self {
        f32::log(self, base)
    }
}

impl sealed::Sealed for f64 {}

impl Float for f64 {
    #[inline]
    fn sin(self, _: Token) -> Self {
        f64::sin(self)
    }
    #[inline]
    fn cos(self, _: Token) -> Self {
        f64::cos(self)
    }
    #[inline]
    fn tan(self, _: Token) -> Self {
        f64::tan(self)
    }
    #[inline]
    fn asin(self, _: Token) -> Self {
        f64::asin(self)
    }
    #[inline]
    fn acos(self, _: Token) -> Self {
        f64::acos(self)
    }
    #[inline]
    fn atan(self, _: Token) -> Self {
        f64::atan(self)
    }
    #[inline]
    fn sinh(self, _: Token) -> Self {
        f64::sinh(self)
    }
    #[inline]
    fn cosh(self, _: Token) -> Self {
        f64::cosh(self)
    }
    #[inline]
    fn tanh(self, _: Token) -> Self {
        f64::tanh(self)
    }
    #[inline]
    fn asinh(self, _: Token) -> Self {
        f64::asinh(self)
    }
    #[inline]
    fn acosh(self, _: Token) -> Self {
        f64::acosh(self)
    }
    #[inline]
    fn atanh(self, _: Token) -> Self {
        f64::atanh(self)
    }
    #[inline]
    fn exp(self, _: Token) -> Self {
        f64::exp(self)
    }
    #[inline]
    fn exp2(self, _: Token) -> Self {
        f64::exp2(self)
    }
    #[inline]
    fn exp_m1(self, _: Token) -> Self {
        f64::exp_m1(self)
    }
    #[inline]
    fn ln(self, _: Token) -> Self {
        f64::ln(self)
    }
    #[inline]
    fn ln_1p(self, _: Token) -> Self {
        f64::ln_1p(self)
    }
    #[inline]
    fn log2(self, _: Token) -> Self {
        f64::log2(self)
    }
    #[inline]
    fn log10(self, _: Token) -> Self {
        f64::log10(self)
    }
    #[inline]
    fn cbrt(self, _: Token) -> Self {
        f64::cbrt(self)
    }
    #[inline]
    fn sin_cos(self, _: Token) -> (Self, Self) {
        f64::sin_cos(self)
    }
    #[inline]
    fn atan2(self, other: Self, _: Token) -> Self {
        f64::atan2(self, other)
    }
    #[inline]
    fn hypot(self, other: Self, _: Token) -> Self {
        f64::hypot(self, other)
    }
    #[inline]
    fn powf(self, exponent: Self, _: Token) -> Self {
        f64::powf(self, exponent)
    }
    #[inline]
    fn powi(self, exponent: i32, _: Token) -> Self {
        f64::powi(self, exponent)
    }
    #[inline]
    fn log(self, base: Self, _: Token) -> Self {
        f64::log(self, base)
    }
}

/// The sine of `x`.
#[inline]
pub fn sin<T: Float>(x: T) -> T {
    x.sin(Token(()))
}

/// The cosine of `x`.
#[inline]
pub fn cos<T: Float>(x: T) -> T {
    x.cos(Token(()))
}

/// The tangent of `x`.
#[inline]
pub fn tan<T: Float>(x: T) -> T {
    x.tan(Token(()))
}

/// The arcsine of `x`.
#[inline]
pub fn asin<T: Float>(x: T) -> T {
    x.asin(Token(()))
}

/// The arccosine of `x`.
#[inline]
pub fn acos<T: Float>(x: T) -> T {
    x.acos(Token(()))
}

/// The arctangent of `x`.
#[inline]
pub fn atan<T: Float>(x: T) -> T {
    x.atan(Token(()))
}

/// The hyperbolic sine of `x`.
#[inline]
pub fn sinh<T: Float>(x: T) -> T {
    x.sinh(Token(()))
}

/// The hyperbolic cosine of `x`.
#[inline]
pub fn cosh<T: Float>(x: T) -> T {
    x.cosh(Token(()))
}

/// The hyperbolic tangent of `x`.
#[inline]
pub fn tanh<T: Float>(x: T) -> T {
    x.tanh(Token(()))
}

/// The inverse hyperbolic sine of `x`.
#[inline]
pub fn asinh<T: Float>(x: T) -> T {
    x.asinh(Token(()))
}

/// The inverse hyperbolic cosine of `x`.
#[inline]
pub fn acosh<T: Float>(x: T) -> T {
    x.acosh(Token(()))
}

/// The inverse hyperbolic tangent of `x`.
#[inline]
pub fn atanh<T: Float>(x: T) -> T {
    x.atanh(Token(()))
}

/// The `e^x` of `x`.
#[inline]
pub fn exp<T: Float>(x: T) -> T {
    x.exp(Token(()))
}

/// The `2^x` of `x`.
#[inline]
pub fn exp2<T: Float>(x: T) -> T {
    x.exp2(Token(()))
}

/// The `e^x - 1`, accurate near zero of `x`.
#[inline]
pub fn exp_m1<T: Float>(x: T) -> T {
    x.exp_m1(Token(()))
}

/// The natural logarithm of `x`.
#[inline]
pub fn ln<T: Float>(x: T) -> T {
    x.ln(Token(()))
}

/// The `ln(1 + x)`, accurate near zero of `x`.
#[inline]
pub fn ln_1p<T: Float>(x: T) -> T {
    x.ln_1p(Token(()))
}

/// The base-2 logarithm of `x`.
#[inline]
pub fn log2<T: Float>(x: T) -> T {
    x.log2(Token(()))
}

/// The base-10 logarithm of `x`.
#[inline]
pub fn log10<T: Float>(x: T) -> T {
    x.log10(Token(()))
}

/// The cube root of `x`.
#[inline]
pub fn cbrt<T: Float>(x: T) -> T {
    x.cbrt(Token(()))
}

/// Sine and cosine of `x`, as `(sin x, cos x)`.
#[inline]
pub fn sin_cos<T: Float>(x: T) -> (T, T) {
    x.sin_cos(Token(()))
}

/// The four-quadrant arctangent of `y / x` (radians, in `(-pi, pi]`).
#[inline]
pub fn atan2<T: Float>(y: T, x: T) -> T {
    y.atan2(x, Token(()))
}

/// The length of the hypotenuse, `sqrt(x^2 + y^2)`, without intermediate overflow.
#[inline]
pub fn hypot<T: Float>(x: T, y: T) -> T {
    x.hypot(y, Token(()))
}

/// `x` raised to the floating-point power `y`.
#[inline]
pub fn powf<T: Float>(x: T, y: T) -> T {
    x.powf(y, Token(()))
}

/// `x` raised to the integer power `n`.
#[inline]
pub fn powi<T: Float>(x: T, n: i32) -> T {
    x.powi(n, Token(()))
}

/// The logarithm of `x` in the given `base`.
#[inline]
pub fn log<T: Float>(x: T, base: T) -> T {
    x.log(base, Token(()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[allow(
        clippy::disallowed_methods,
        reason = "tests compare against the std methods the funnel wraps"
    )]
    fn scalar_matches_std_bitwise() {
        for x in [0.0_f32, 0.5, 1.0, -2.25, 100.0] {
            assert_eq!(sin(x).to_bits(), x.sin().to_bits());
            assert_eq!(cos(x).to_bits(), x.cos().to_bits());
            assert_eq!(exp(x).to_bits(), x.exp().to_bits());
            assert_eq!(atan(x).to_bits(), x.atan().to_bits());
            assert_eq!(cbrt(x).to_bits(), x.cbrt().to_bits());
        }
        for x in [0.0_f64, 0.5, 1.0, -2.25, 100.0] {
            assert_eq!(sin(x).to_bits(), x.sin().to_bits());
            assert_eq!(tanh(x).to_bits(), x.tanh().to_bits());
        }
        assert_eq!(powi(2.0_f64, 10).to_bits(), 1024.0_f64.to_bits());
        let (s, c) = sin_cos(1.0_f32);
        assert_eq!(
            (s.to_bits(), c.to_bits()),
            (1.0_f32.sin_cos().0.to_bits(), 1.0_f32.sin_cos().1.to_bits())
        );
        assert_eq!(atan2(1.0_f32, 2.0).to_bits(), 1.0_f32.atan2(2.0).to_bits());
        assert_eq!(hypot(3.0_f64, 4.0).to_bits(), 5.0_f64.to_bits());
        assert_eq!(log(8.0_f64, 2.0).to_bits(), 8.0_f64.log(2.0).to_bits());
        assert_eq!(powf(2.0_f32, 0.5).to_bits(), 2.0_f32.powf(0.5).to_bits());
    }

    #[test]
    fn inverse_functions_roundtrip() {
        for x in [0.1_f64, 0.5, 0.9] {
            assert!((asin(sin(x)) - x).abs() < 1e-12);
            assert!((acos(cos(x)) - x).abs() < 1e-12);
            assert!((ln(exp(x)) - x).abs() < 1e-12);
            assert!((log2(exp2(x)) - x).abs() < 1e-12);
            assert!((log10(powf(10.0, x)) - x).abs() < 1e-12);
            assert!((asinh(sinh(x)) - x).abs() < 1e-12);
            assert!((acosh(cosh(x)) - x).abs() < 1e-12);
            assert!((atanh(tanh(x)) - x).abs() < 1e-12);
            assert!((ln_1p(exp_m1(x)) - x).abs() < 1e-12);
            assert!((tan(atan(x)) - x).abs() < 1e-12);
        }
    }
}
