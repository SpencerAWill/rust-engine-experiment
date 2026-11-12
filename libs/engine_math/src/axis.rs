//! Named directions in the engine's convention.
//!
//! The convention is right-handed and identical to glTF 2.0's axes: +X right, +Y up, with the
//! default camera looking along -Z. Two meanings of "forward" are both named here and never
//! conflated: [`VIEW_FORWARD`] (the way a camera looks) and [`ASSET_FRONT`] (the way an imported
//! asset's front faces). Conventions disagree on these (Unity's camera and asset fronts both
//! point along +Z, for example), which is why [`convert`](crate::convert) avoids the word
//! "forward" entirely. See ADR 0005.

use glam::{Vec2, Vec3};

/// Right: +X.
pub const RIGHT: Vec3 = Vec3::X;
/// Left: -X.
pub const LEFT: Vec3 = Vec3::NEG_X;
/// Up: +Y.
pub const UP: Vec3 = Vec3::Y;
/// Down: -Y.
pub const DOWN: Vec3 = Vec3::NEG_Y;
/// View forward: the direction a default camera looks (-Z), with +X to the viewer's right and
/// +Y up (glam's `look_to_rh`).
pub const VIEW_FORWARD: Vec3 = Vec3::NEG_Z;
/// Asset front: the direction an imported asset's front faces (+Z), per the glTF convention.
/// Assets therefore face a default camera.
pub const ASSET_FRONT: Vec3 = Vec3::Z;
/// 2D right: +X.
pub const RIGHT_2D: Vec2 = Vec2::X;
/// 2D up: +Y.
pub const UP_2D: Vec2 = Vec2::Y;
