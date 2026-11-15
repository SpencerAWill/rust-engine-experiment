//! The transcendental-math funnel.
//!
//! All transcendental math in the engine goes through these functions instead of the inherent
//! `f32`/`f64` methods, which `clippy.toml` bans (ADR 0003). The functions have two backends,
//! chosen by the `std` feature:
//!
//! - **With `std` (the default),** each function calls the inherent method, so results are
//!   bit-identical to `std`'s. Inside glam, `std` is the backend too.
//! - **Without `std`,** each function calls the [`libm`](https://docs.rs/libm) function of the
//!   same name (the mapping is on each function), and glam uses `libm` through its `nostd-libm`
//!   feature. No other backend exists there.
//!
//! Within one build the backend is fixed, so results are deterministic for that build. A `std`
//! build and a `no_std` build may differ in the last bits, and so may two platforms, because
//! `std` takes its functions from each platform's maths library. Moving to results that are
//! identical across platforms is a contained change, because every call site goes through here:
//! use `libm` everywhere and enable glam's `scalar-math`, as one deliberate engine-wide switch
//! (never a Cargo feature; see ADR 0003).
//!
//! **`default-features = false` on a target that has `std`** selects the `libm` backend, as it
//! does on a bare-metal target: the `std` feature chooses the backend, it does not detect the
//! target.
//!
//! The functions are generic over the sealed [`Float`] trait (`f32` and `f64`), so the type is
//! inferred: `scalar::sin(x)`.
//!
//! Correctly-rounded IEEE operations (`+ - * /`, `sqrt`, `mul_add`, `abs`, `floor`, `ceil`,
//! `round`, `trunc`, `min`, `max`, `copysign`) are not transcendental and stay available as
//! ordinary methods.

mod libm_backend;
#[cfg(feature = "std")]
mod std_backend;

// The backend the funnel calls: `std`'s inherent methods, or `libm` without `std`.
#[cfg(not(feature = "std"))]
use libm_backend as backend;
#[cfg(feature = "std")]
use std_backend as backend;

mod sealed {
    /// Prevents implementations outside this module.
    pub trait Sealed {}

    /// An argument no code outside this module can construct, so the trait's methods cannot be
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
/// use engine::math::scalar;
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

/// Implements the sealed [`Float`] trait for `$t`, calling the functions of the backend module
/// `$m` (`single` or `double`): `std_backend` with the `std` feature, else `libm_backend`.
macro_rules! impl_float {
    ($t:ident, $m:ident) => {
        impl sealed::Sealed for $t {}

        impl Float for $t {
            #[inline]
            fn sin(self, _: Token) -> Self {
                backend::$m::sin(self)
            }
            #[inline]
            fn cos(self, _: Token) -> Self {
                backend::$m::cos(self)
            }
            #[inline]
            fn tan(self, _: Token) -> Self {
                backend::$m::tan(self)
            }
            #[inline]
            fn asin(self, _: Token) -> Self {
                backend::$m::asin(self)
            }
            #[inline]
            fn acos(self, _: Token) -> Self {
                backend::$m::acos(self)
            }
            #[inline]
            fn atan(self, _: Token) -> Self {
                backend::$m::atan(self)
            }
            #[inline]
            fn sinh(self, _: Token) -> Self {
                backend::$m::sinh(self)
            }
            #[inline]
            fn cosh(self, _: Token) -> Self {
                backend::$m::cosh(self)
            }
            #[inline]
            fn tanh(self, _: Token) -> Self {
                backend::$m::tanh(self)
            }
            #[inline]
            fn asinh(self, _: Token) -> Self {
                backend::$m::asinh(self)
            }
            #[inline]
            fn acosh(self, _: Token) -> Self {
                backend::$m::acosh(self)
            }
            #[inline]
            fn atanh(self, _: Token) -> Self {
                backend::$m::atanh(self)
            }
            #[inline]
            fn exp(self, _: Token) -> Self {
                backend::$m::exp(self)
            }
            #[inline]
            fn exp2(self, _: Token) -> Self {
                backend::$m::exp2(self)
            }
            #[inline]
            fn exp_m1(self, _: Token) -> Self {
                backend::$m::exp_m1(self)
            }
            #[inline]
            fn ln(self, _: Token) -> Self {
                backend::$m::ln(self)
            }
            #[inline]
            fn ln_1p(self, _: Token) -> Self {
                backend::$m::ln_1p(self)
            }
            #[inline]
            fn log2(self, _: Token) -> Self {
                backend::$m::log2(self)
            }
            #[inline]
            fn log10(self, _: Token) -> Self {
                backend::$m::log10(self)
            }
            #[inline]
            fn cbrt(self, _: Token) -> Self {
                backend::$m::cbrt(self)
            }
            #[inline]
            fn sin_cos(self, _: Token) -> (Self, Self) {
                backend::$m::sin_cos(self)
            }
            #[inline]
            fn atan2(self, other: Self, _: Token) -> Self {
                backend::$m::atan2(self, other)
            }
            #[inline]
            fn hypot(self, other: Self, _: Token) -> Self {
                backend::$m::hypot(self, other)
            }
            #[inline]
            fn powf(self, exponent: Self, _: Token) -> Self {
                backend::$m::powf(self, exponent)
            }
            #[inline]
            fn powi(self, exponent: i32, _: Token) -> Self {
                backend::$m::powi(self, exponent)
            }
            #[inline]
            fn log(self, base: Self, _: Token) -> Self {
                backend::$m::log(self, base)
            }
        }
    };
}

impl_float!(f32, single);
impl_float!(f64, double);

/// The sine of `x`.
///
/// Without `std`: `libm::sinf` / `libm::sin`.
#[inline]
pub fn sin<T: Float>(x: T) -> T {
    x.sin(Token(()))
}

/// The cosine of `x`.
///
/// Without `std`: `libm::cosf` / `libm::cos`.
#[inline]
pub fn cos<T: Float>(x: T) -> T {
    x.cos(Token(()))
}

/// The tangent of `x`.
///
/// Without `std`: `libm::tanf` / `libm::tan`.
#[inline]
pub fn tan<T: Float>(x: T) -> T {
    x.tan(Token(()))
}

/// The arcsine of `x`.
///
/// Without `std`: `libm::asinf` / `libm::asin`.
#[inline]
pub fn asin<T: Float>(x: T) -> T {
    x.asin(Token(()))
}

/// The arccosine of `x`.
///
/// Without `std`: `libm::acosf` / `libm::acos`.
#[inline]
pub fn acos<T: Float>(x: T) -> T {
    x.acos(Token(()))
}

/// The arctangent of `x`.
///
/// Without `std`: `libm::atanf` / `libm::atan`.
#[inline]
pub fn atan<T: Float>(x: T) -> T {
    x.atan(Token(()))
}

/// The hyperbolic sine of `x`.
///
/// Without `std`: `libm::sinhf` / `libm::sinh`.
#[inline]
pub fn sinh<T: Float>(x: T) -> T {
    x.sinh(Token(()))
}

/// The hyperbolic cosine of `x`.
///
/// Without `std`: `libm::coshf` / `libm::cosh`.
#[inline]
pub fn cosh<T: Float>(x: T) -> T {
    x.cosh(Token(()))
}

/// The hyperbolic tangent of `x`.
///
/// Without `std`: `libm::tanhf` / `libm::tanh`.
#[inline]
pub fn tanh<T: Float>(x: T) -> T {
    x.tanh(Token(()))
}

/// The inverse hyperbolic sine of `x`.
///
/// Without `std`: `libm::asinhf` / `libm::asinh`.
#[inline]
pub fn asinh<T: Float>(x: T) -> T {
    x.asinh(Token(()))
}

/// The inverse hyperbolic cosine of `x`.
///
/// Without `std`: `libm::acoshf` / `libm::acosh`.
#[inline]
pub fn acosh<T: Float>(x: T) -> T {
    x.acosh(Token(()))
}

/// The inverse hyperbolic tangent of `x`.
///
/// Without `std`: `libm::atanhf` / `libm::atanh`.
#[inline]
pub fn atanh<T: Float>(x: T) -> T {
    x.atanh(Token(()))
}

/// The `e^x` of `x`.
///
/// Without `std`: `libm::expf` / `libm::exp`.
#[inline]
pub fn exp<T: Float>(x: T) -> T {
    x.exp(Token(()))
}

/// The `2^x` of `x`.
///
/// Without `std`: `libm::exp2f` / `libm::exp2`.
#[inline]
pub fn exp2<T: Float>(x: T) -> T {
    x.exp2(Token(()))
}

/// The `e^x - 1`, accurate near zero of `x`.
///
/// Without `std`: `libm::expm1f` / `libm::expm1`.
#[inline]
pub fn exp_m1<T: Float>(x: T) -> T {
    x.exp_m1(Token(()))
}

/// The natural logarithm of `x`.
///
/// Without `std`: `libm::logf` / `libm::log`.
#[inline]
pub fn ln<T: Float>(x: T) -> T {
    x.ln(Token(()))
}

/// The `ln(1 + x)`, accurate near zero of `x`.
///
/// Without `std`: `libm::log1pf` / `libm::log1p`.
#[inline]
pub fn ln_1p<T: Float>(x: T) -> T {
    x.ln_1p(Token(()))
}

/// The base-2 logarithm of `x`.
///
/// Without `std`: `libm::log2f` / `libm::log2`.
#[inline]
pub fn log2<T: Float>(x: T) -> T {
    x.log2(Token(()))
}

/// The base-10 logarithm of `x`.
///
/// Without `std`: `libm::log10f` / `libm::log10`.
#[inline]
pub fn log10<T: Float>(x: T) -> T {
    x.log10(Token(()))
}

/// The cube root of `x`.
///
/// Without `std`: `libm::cbrtf` / `libm::cbrt`.
#[inline]
pub fn cbrt<T: Float>(x: T) -> T {
    x.cbrt(Token(()))
}

/// Sine and cosine of `x`, as `(sin x, cos x)`.
///
/// Without `std`: `libm::sincosf` / `libm::sincos`.
#[inline]
pub fn sin_cos<T: Float>(x: T) -> (T, T) {
    x.sin_cos(Token(()))
}

/// The four-quadrant arctangent of `y / x` (radians, in `(-pi, pi]`).
///
/// Without `std`: `libm::atan2f` / `libm::atan2`.
#[inline]
pub fn atan2<T: Float>(y: T, x: T) -> T {
    y.atan2(x, Token(()))
}

/// The length of the hypotenuse, `sqrt(x^2 + y^2)`, without intermediate overflow.
///
/// Without `std`: `libm::hypotf` / `libm::hypot`.
#[inline]
pub fn hypot<T: Float>(x: T, y: T) -> T {
    x.hypot(y, Token(()))
}

/// `x` raised to the floating-point power `y`.
///
/// Without `std`: `libm::powf` / `libm::pow`.
#[inline]
pub fn powf<T: Float>(x: T, y: T) -> T {
    x.powf(y, Token(()))
}

/// `x` raised to the integer power `n`.
///
/// Without `std`: `libm::powf` / `libm::pow`, with `n` converted to the float type (`libm` has no `powi`). `std`'s `powi` multiplies repeatedly, so for a large `|n|` the two backends can differ by more than a few ulps.
#[inline]
pub fn powi<T: Float>(x: T, n: i32) -> T {
    x.powi(n, Token(()))
}

/// The logarithm of `x` in the given `base`.
///
/// Without `std`: `ln(x) / ln(base)`, with `ln` as above.
#[inline]
pub fn log<T: Float>(x: T, base: T) -> T {
    x.log(base, Token(()))
}

#[cfg(test)]
#[allow(
    clippy::disallowed_methods,
    reason = "the tests compare the funnel with the std methods it wraps"
)]
mod tests {
    use super::*;

    #[test]
    #[cfg(feature = "std")]
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

    /// Inputs spread over the signs, zero, and the unit interval, in both float widths.
    const INPUTS: [f64; 10] = [-2.5, -1.0, -0.5, -0.0, 0.0, 0.25, 0.5, 1.0, 2.0, 3.0];
    const EXPONENTS: [i32; 4] = [-3, 0, 1, 5];

    /// One unary function: the funnel's, `std`'s, `libm`'s, and the inputs it is defined on.
    type Unary<T> = (
        &'static str,
        fn(T) -> T,
        fn(T) -> T,
        fn(T) -> T,
        fn(T) -> bool,
    );

    macro_rules! unary_cases {
        ($t:ident, $backend:ident, [$(($name:ident, $domain:expr)),* $(,)?]) => {
            [$((
                stringify!($name),
                $name::<$t> as fn($t) -> $t,
                (|x: $t| x.$name()) as fn($t) -> $t,
                libm_backend::$backend::$name as fn($t) -> $t,
                $domain as fn($t) -> bool,
            )),*]
        };
    }

    macro_rules! all_unary {
        ($t:ident, $backend:ident) => {
            unary_cases!(
                $t,
                $backend,
                [
                    (sin, |_| true),
                    (cos, |_| true),
                    (tan, |_| true),
                    (asin, |x| x.abs() <= 1.0),
                    (acos, |x| x.abs() <= 1.0),
                    (atan, |_| true),
                    (sinh, |_| true),
                    (cosh, |_| true),
                    (tanh, |_| true),
                    (asinh, |_| true),
                    (acosh, |x| x >= 1.0),
                    (atanh, |x| x.abs() < 1.0),
                    (exp, |_| true),
                    (exp2, |_| true),
                    (exp_m1, |_| true),
                    (ln, |x| x > 0.0),
                    (ln_1p, |x| x > -1.0),
                    (log2, |x| x > 0.0),
                    (log10, |x| x > 0.0),
                    (cbrt, |_| true),
                ]
            )
        };
    }

    /// The unary functions of both widths, as `(name, funnel, std, libm, domain)`.
    fn unary_f32() -> [Unary<f32>; 20] {
        all_unary!(f32, single)
    }
    fn unary_f64() -> [Unary<f64>; 20] {
        all_unary!(f64, double)
    }

    #[allow(
        clippy::cast_possible_truncation,
        reason = "the inputs are exact in f32"
    )]
    fn inputs_f32() -> impl Iterator<Item = f32> {
        INPUTS.into_iter().map(|x| x as f32)
    }

    /// `|a - b|` is within four ulps of one, scaled by the larger of `|b|` and one.
    fn close_f32(a: f32, b: f32) -> bool {
        (a - b).abs() <= 4.0 * f32::EPSILON * b.abs().max(1.0)
    }
    fn close_f64(a: f64, b: f64) -> bool {
        (a - b).abs() <= 4.0 * f64::EPSILON * b.abs().max(1.0)
    }

    #[test]
    #[cfg(feature = "std")]
    #[allow(
        clippy::disallowed_methods,
        reason = "tests compare against the std methods the funnel wraps"
    )]
    fn std_backend_is_the_inherent_method() {
        for (name, funnel, std, _, domain) in unary_f32() {
            for x in inputs_f32().filter(|&x| domain(x)) {
                assert_eq!(funnel(x).to_bits(), std(x).to_bits(), "{name}({x})");
            }
        }
        for (name, funnel, std, _, domain) in unary_f64() {
            for x in INPUTS.into_iter().filter(|&x| domain(x)) {
                assert_eq!(funnel(x).to_bits(), std(x).to_bits(), "{name}({x})");
            }
        }
        for x in INPUTS {
            for y in INPUTS {
                assert_eq!(atan2(y, x).to_bits(), y.atan2(x).to_bits());
                assert_eq!(hypot(x, y).to_bits(), x.hypot(y).to_bits());
                if x > 0.0 {
                    assert_eq!(powf(x, y).to_bits(), x.powf(y).to_bits());
                }
            }
            if x > 0.0 {
                for base in [2.0, 10.0] {
                    assert_eq!(log(x, base).to_bits(), x.log(base).to_bits());
                }
            }
            for n in EXPONENTS.into_iter().filter(|&n| n >= 0 || x != 0.0) {
                assert_eq!(powi(x, n).to_bits(), x.powi(n).to_bits(), "powi({x}, {n})");
            }
            let (s, c) = sin_cos(x);
            assert_eq!(
                (s.to_bits(), c.to_bits()),
                (x.sin().to_bits(), x.cos().to_bits())
            );
        }
    }

    #[test]
    #[allow(
        clippy::disallowed_methods,
        clippy::cast_possible_truncation,
        reason = "tests compare the libm backend against the std methods"
    )]
    fn libm_backend_agrees_with_std() {
        for (name, _, std, libm, domain) in unary_f32() {
            for x in inputs_f32().filter(|&x| domain(x)) {
                assert!(
                    close_f32(libm(x), std(x)),
                    "{name}({x}): libm {} std {}",
                    libm(x),
                    std(x)
                );
            }
        }
        for (name, _, std, libm, domain) in unary_f64() {
            for x in INPUTS.into_iter().filter(|&x| domain(x)) {
                assert!(
                    close_f64(libm(x), std(x)),
                    "{name}({x}): libm {} std {}",
                    libm(x),
                    std(x)
                );
            }
        }
        for x in INPUTS {
            let xs = x as f32;
            for y in INPUTS {
                let ys = y as f32;
                assert!(
                    close_f64(libm_backend::double::atan2(y, x), y.atan2(x)),
                    "atan2 f64"
                );
                assert!(
                    close_f32(libm_backend::single::atan2(ys, xs), ys.atan2(xs)),
                    "atan2 f32"
                );
                assert!(
                    close_f64(libm_backend::double::hypot(x, y), x.hypot(y)),
                    "hypot f64"
                );
                assert!(
                    close_f32(libm_backend::single::hypot(xs, ys), xs.hypot(ys)),
                    "hypot f32"
                );
                if x > 0.0 {
                    assert!(
                        close_f64(libm_backend::double::powf(x, y), x.powf(y)),
                        "powf f64"
                    );
                    assert!(
                        close_f32(libm_backend::single::powf(xs, ys), xs.powf(ys)),
                        "powf f32"
                    );
                }
            }
            if x > 0.0 {
                for base in [2.0, 10.0] {
                    assert!(
                        close_f64(libm_backend::double::log(x, base), x.log(base)),
                        "log f64"
                    );
                    let (xs, bs) = (xs, base as f32);
                    assert!(
                        close_f32(libm_backend::single::log(xs, bs), xs.log(bs)),
                        "log f32"
                    );
                }
            }
            for n in EXPONENTS.into_iter().filter(|&n| n >= 0 || x != 0.0) {
                assert!(
                    close_f64(libm_backend::double::powi(x, n), x.powi(n)),
                    "powi({x}, {n}) f64"
                );
                assert!(
                    close_f32(libm_backend::single::powi(xs, n), xs.powi(n)),
                    "powi({xs}, {n}) f32"
                );
            }
            let (s, c) = libm_backend::double::sin_cos(x);
            assert!(
                close_f64(s, x.sin()) && close_f64(c, x.cos()),
                "sin_cos f64"
            );
            let (s, c) = libm_backend::single::sin_cos(xs);
            assert!(
                close_f32(s, xs.sin()) && close_f32(c, xs.cos()),
                "sin_cos f32"
            );
        }
    }
}
