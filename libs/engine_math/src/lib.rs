#![forbid(unsafe_code)]
//! The engine's math vocabulary: one blessed set of vector, matrix, and transform types.
//!
//! `engine_math` is a separate crate because it carries the `glam` dependency and is `std`-only
//! (ADR 0004). It re-exports the [`glam`] types the engine uses (the whole crate is also
//! available as [`engine_math::glam`](glam) for escape-hatch access), and adds:
//!
//! - [`scalar`]: the funnel through which all transcendental math flows (ADR 0003);
//! - small `f32` helpers: [`lerp`], [`inverse_lerp`], [`remap`], [`approx_eq`].
//!
//! # Conventions
//!
//! Right-handed, +Y up, glTF-compatible axes; meters, seconds, radians; column-major matrices
//! with column vectors (`M * v`), as in glam. See ADR 0005 for the full convention and ADR 0006
//! for the large-world precision design.
//!
//! # Feature-unification warning
//!
//! glam's version is part of this crate's public API. Any crate in a dependency graph can enable
//! glam's `fast-math` or `scalar-math` features through Cargo feature unification, silently
//! changing numerics. `scalar-math` changes type sizes and alignments, which the layout
//! assertions in this crate catch. `fast-math` cannot be detected, so it is **unsupported**.
//!
//! # Quick start
//!
//! ```
//! use engine_math::{Vec3, lerp, scalar};
//!
//! let halfway = lerp(0.0, 10.0, 0.5);
//! assert_eq!(halfway, 5.0);
//! let (s, c) = scalar::sin_cos(0.0_f32);
//! assert_eq!((s, c), (0.0, 1.0));
//! assert_eq!(Vec3::X + Vec3::Y, Vec3::new(1.0, 1.0, 0.0));
//! ```

mod helpers;
mod layout;
pub mod scalar;
mod transform;
mod transform2d;
mod world;

pub use glam;
pub use glam::{
    Affine2, Affine3A, DAffine2, DAffine3, DMat2, DMat3, DMat4, DQuat, DVec2, DVec3, DVec4, IVec2,
    IVec3, IVec4, Mat2, Mat3, Mat3A, Mat4, Quat, UVec2, UVec3, UVec4, Vec2, Vec3, Vec3A, Vec4,
};
pub use helpers::{approx_eq, inverse_lerp, lerp, remap};
pub use transform::Transform;
pub use transform2d::{Transform2d, WorldTransform2d};
pub use world::WorldTransform;
