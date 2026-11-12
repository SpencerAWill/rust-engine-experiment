//! [`Transform2d`] and [`WorldTransform2d`]: 2D placement in the XY plane, and the bridges
//! between the 2D and 3D vocabularies.
//!
//! The 2D world is the XY plane of the 3D convention (+X right, +Y up), and rotation is an angle
//! about +Z, counter-clockwise, in radians. All trigonometry goes through [`scalar`], and the 2D
//! matrices are built by hand from `sin`/`cos` rather than with glam's `from_angle` helpers,
//! which call the platform's math library directly (ADR 0003).

use core::f32::consts::{PI, TAU};
use core::ops::Mul;

use glam::{Affine2, DAffine2, DMat2, DVec2, DVec3, Mat2, Mat3, Quat, Vec2, Vec3};

use crate::{Transform, WorldTransform, scalar};

/// Wraps an angle into `(-pi, pi]`.
///
/// Angles already in range are returned unchanged: shifting by `PI` first would round small
/// angles onto the coarse `f32` grid near 3.14 (about 2.4e-7 apart).
fn wrap_angle(angle: f32) -> f32 {
    if angle > -PI && angle <= PI {
        return angle;
    }
    let wrapped = (angle + PI).rem_euclid(TAU) - PI;
    if wrapped <= -PI { PI } else { wrapped }
}

/// Rotates `v` counter-clockwise by `angle` radians.
#[inline]
fn rotate(v: Vec2, angle: f32) -> Vec2 {
    let (s, c) = scalar::sin_cos(angle);
    Vec2::new(c * v.x - s * v.y, s * v.x + c * v.y)
}

/// The quaternion for a rotation of `angle` radians about +Z.
#[inline]
fn quat_about_z(angle: f32) -> Quat {
    let (s, c) = scalar::sin_cos(angle * 0.5);
    Quat::from_xyzw(0.0, 0.0, s, c)
}

/// The twist of `q` about +Z in `(-pi, pi]`, or `0.0` when it is undefined.
///
/// A rotation splits into a *swing* (the part that tilts +Z) followed by a *twist* (a rotation
/// about +Z). The twist's quaternion is the normalized `(0, 0, z, w)` part of `q`, so its angle is
/// `2 * atan2(z, w)`. When `hypot(z, w)` is (nearly) zero, as for a half turn about X or Y, the
/// swing consumes the whole rotation and the twist is undefined; it is reported as `0.0`.
fn twist_about_z(q: Quat) -> f32 {
    if scalar::hypot(q.z, q.w) < 1e-6 {
        0.0
    } else {
        wrap_angle(2.0 * scalar::atan2(q.z, q.w))
    }
}

/// A 2D transform in the XY plane: translation, rotation, scale.
///
/// `rotation` is in radians, counter-clockwise (about +Z). `#[repr(C)]` fixes the field order, and
/// the layout is pinned by static assertions.
///
/// Composition is per-component with the same contract as 3D [`Transform`]: rotations add (and
/// the sum is wrapped into `(-pi, pi]`), and the result equals composing the [`to_affine`]
/// matrices exactly (within float epsilon) when the parent's scale is uniform.
///
/// [`to_affine`]: Self::to_affine
///
/// # Examples
///
/// ```
/// use engine_math::{Transform2d, Vec2};
///
/// let parent = Transform2d::from_translation(Vec2::new(10.0, 0.0))
///     .with_rotation(core::f32::consts::FRAC_PI_2);
/// let child = Transform2d::from_translation(Vec2::X);
///
/// // The child's +X offset is rotated onto +Y by the parent.
/// let placed = parent * child;
/// assert!((placed.translation - Vec2::new(10.0, 1.0)).length() < 1e-6);
/// ```
#[derive(Clone, Copy, Debug, PartialEq)]
#[repr(C)]
pub struct Transform2d {
    /// Position.
    pub translation: Vec2,
    /// Counter-clockwise angle in radians.
    pub rotation: f32,
    /// Scale along each axis.
    pub scale: Vec2,
}

impl Transform2d {
    /// No translation, no rotation, unit scale.
    pub const IDENTITY: Self = Self {
        translation: Vec2::ZERO,
        rotation: 0.0,
        scale: Vec2::ONE,
    };

    /// A pure translation.
    ///
    /// # Examples
    ///
    /// ```
    /// # use engine_math::{Transform2d, Vec2};
    /// assert_eq!(Transform2d::from_translation(Vec2::X).translation, Vec2::X);
    /// ```
    #[inline]
    pub const fn from_translation(t: Vec2) -> Self {
        Self {
            translation: t,
            ..Self::IDENTITY
        }
    }

    /// A pure rotation, in radians.
    ///
    /// # Examples
    ///
    /// ```
    /// # use engine_math::Transform2d;
    /// assert_eq!(Transform2d::from_rotation(1.0).rotation, 1.0);
    /// ```
    #[inline]
    pub const fn from_rotation(radians: f32) -> Self {
        Self {
            rotation: radians,
            ..Self::IDENTITY
        }
    }

    /// A pure scale.
    ///
    /// # Examples
    ///
    /// ```
    /// # use engine_math::{Transform2d, Vec2};
    /// assert_eq!(Transform2d::from_scale(Vec2::splat(2.0)).scale, Vec2::splat(2.0));
    /// ```
    #[inline]
    pub const fn from_scale(s: Vec2) -> Self {
        Self {
            scale: s,
            ..Self::IDENTITY
        }
    }

    /// This transform with its translation replaced.
    ///
    /// # Examples
    ///
    /// ```
    /// # use engine_math::{Transform2d, Vec2};
    /// assert_eq!(Transform2d::IDENTITY.with_translation(Vec2::Y).translation, Vec2::Y);
    /// ```
    #[inline]
    #[must_use]
    pub const fn with_translation(mut self, t: Vec2) -> Self {
        self.translation = t;
        self
    }

    /// This transform with its rotation replaced.
    ///
    /// # Examples
    ///
    /// ```
    /// # use engine_math::Transform2d;
    /// assert_eq!(Transform2d::IDENTITY.with_rotation(0.5).rotation, 0.5);
    /// ```
    #[inline]
    #[must_use]
    pub const fn with_rotation(mut self, radians: f32) -> Self {
        self.rotation = radians;
        self
    }

    /// This transform with its scale replaced.
    ///
    /// # Examples
    ///
    /// ```
    /// # use engine_math::{Transform2d, Vec2};
    /// assert_eq!(Transform2d::IDENTITY.with_scale(Vec2::splat(3.0)).scale, Vec2::splat(3.0));
    /// ```
    #[inline]
    #[must_use]
    pub const fn with_scale(mut self, s: Vec2) -> Self {
        self.scale = s;
        self
    }

    /// `self * child`, by per-component composition (see the type docs). The resulting rotation
    /// is wrapped into `(-pi, pi]`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use engine_math::Transform2d;
    /// // 3.0 + 3.0 radians wraps to 6.0 - 2*pi.
    /// let r = (Transform2d::from_rotation(3.0) * Transform2d::from_rotation(3.0)).rotation;
    /// assert!((r - (6.0 - core::f32::consts::TAU)).abs() < 1e-5);
    /// ```
    #[inline]
    #[must_use]
    pub fn mul_transform(self, child: Self) -> Self {
        Self {
            translation: self.translation + rotate(self.scale * child.translation, self.rotation),
            rotation: wrap_angle(self.rotation + child.rotation),
            scale: self.scale * child.scale,
        }
    }

    /// The inverse, by per-component inversion. Exact when the scale is uniform; a zero scale
    /// yields non-finite values.
    ///
    /// # Examples
    ///
    /// ```
    /// # use engine_math::{Transform2d, Vec2};
    /// let t = Transform2d::from_translation(Vec2::new(1.0, 2.0)).with_rotation(0.5);
    /// assert!((t * t.inverse()).translation.length() < 1e-6);
    /// ```
    #[inline]
    #[must_use]
    pub fn inverse(self) -> Self {
        let rotation = wrap_angle(-self.rotation);
        let scale = Vec2::ONE / self.scale;
        Self {
            translation: -rotate(scale * self.translation, rotation),
            rotation,
            scale,
        }
    }

    /// The `(cos, sin)` of the rotation.
    fn cos_sin(self) -> (f32, f32) {
        let (s, c) = scalar::sin_cos(self.rotation);
        (c, s)
    }

    /// The equivalent affine transform.
    ///
    /// # Examples
    ///
    /// ```
    /// # use engine_math::{Affine2, Transform2d};
    /// assert_eq!(Transform2d::IDENTITY.to_affine(), Affine2::IDENTITY);
    /// ```
    #[inline]
    pub fn to_affine(self) -> Affine2 {
        let (c, s) = self.cos_sin();
        Affine2::from_mat2_translation(
            Mat2::from_cols(
                Vec2::new(c * self.scale.x, s * self.scale.x),
                Vec2::new(-s * self.scale.y, c * self.scale.y),
            ),
            self.translation,
        )
    }

    /// The equivalent homogeneous 2D matrix.
    ///
    /// # Examples
    ///
    /// ```
    /// # use engine_math::{Mat3, Transform2d};
    /// assert_eq!(Transform2d::IDENTITY.to_mat3(), Mat3::IDENTITY);
    /// ```
    #[inline]
    pub fn to_mat3(self) -> Mat3 {
        let (c, s) = self.cos_sin();
        Mat3::from_cols(
            Vec3::new(c * self.scale.x, s * self.scale.x, 0.0),
            Vec3::new(-s * self.scale.y, c * self.scale.y, 0.0),
            Vec3::new(self.translation.x, self.translation.y, 1.0),
        )
    }

    /// Transforms a point: scale, then rotate, then translate.
    ///
    /// # Examples
    ///
    /// ```
    /// # use engine_math::{Transform2d, Vec2};
    /// let t = Transform2d::from_translation(Vec2::new(1.0, 0.0));
    /// assert_eq!(t.transform_point(Vec2::Y), Vec2::new(1.0, 1.0));
    /// ```
    #[inline]
    pub fn transform_point(self, p: Vec2) -> Vec2 {
        self.translation + self.transform_vector(p)
    }

    /// Transforms a direction or offset: scale, then rotate, with no translation.
    ///
    /// # Examples
    ///
    /// ```
    /// # use engine_math::{Transform2d, Vec2};
    /// let t = Transform2d::from_translation(Vec2::new(1.0, 0.0));
    /// assert_eq!(t.transform_vector(Vec2::Y), Vec2::Y);
    /// ```
    #[inline]
    pub fn transform_vector(self, v: Vec2) -> Vec2 {
        rotate(self.scale * v, self.rotation)
    }

    /// The exact lift into 3D at depth `z`: rotation about +Z, with `scale.z = 1`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use engine_math::{Transform2d, Vec2};
    /// let t = Transform2d::from_translation(Vec2::new(1.0, 2.0)).to_3d(5.0);
    /// assert_eq!(t.translation.to_array(), [1.0, 2.0, 5.0]);
    /// ```
    #[inline]
    pub fn to_3d(self, z: f32) -> Transform {
        Transform {
            translation: Vec3::new(self.translation.x, self.translation.y, z),
            rotation: quat_about_z(self.rotation),
            scale: Vec3::new(self.scale.x, self.scale.y, 1.0),
        }
    }

    /// Lifts this local transform into world space by adding `origin` in `f64`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use engine_math::{DVec2, Transform2d, Vec2};
    /// let t = Transform2d::from_translation(Vec2::Y);
    /// assert_eq!(t.to_world(DVec2::new(1e9, 0.0)).translation, DVec2::new(1e9, 1.0));
    /// ```
    #[inline]
    pub fn to_world(self, origin: DVec2) -> WorldTransform2d {
        WorldTransform2d {
            translation: origin + self.translation.as_dvec2(),
            rotation: self.rotation,
            scale: self.scale,
        }
    }
}

impl Default for Transform2d {
    /// [`Transform2d::IDENTITY`].
    #[inline]
    fn default() -> Self {
        Self::IDENTITY
    }
}

impl Mul for Transform2d {
    type Output = Self;

    /// [`Transform2d::mul_transform`].
    #[inline]
    fn mul(self, rhs: Self) -> Self {
        self.mul_transform(rhs)
    }
}

/// Large-world 2D placement (`f64` translation), mirroring [`WorldTransform`].
///
/// Absolute placement uses this type; local or relative data uses [`Transform2d`]. Narrowing to
/// `f32` happens only through [`relative_to`](Self::relative_to), with the same 1 mm round-trip
/// guarantee as 3D (ADR 0006).
///
/// # Examples
///
/// ```
/// use engine_math::{DVec2, WorldTransform2d};
///
/// let camera = DVec2::new(6.4e6, 0.0);
/// let object = WorldTransform2d::from_translation(camera + DVec2::new(3.0, 4.0));
/// let local = object.relative_to(camera);
/// assert!((local.translation.length() - 5.0).abs() < 1e-6);
/// assert_eq!(local.to_world(camera).translation, object.translation);
/// ```
#[derive(Clone, Copy, Debug, PartialEq)]
#[repr(C)]
pub struct WorldTransform2d {
    /// Absolute position, in `f64`.
    pub translation: DVec2,
    /// Counter-clockwise angle in radians.
    pub rotation: f32,
    /// Scale along each axis.
    pub scale: Vec2,
}

impl WorldTransform2d {
    /// At the origin, no rotation, unit scale.
    pub const IDENTITY: Self = Self {
        translation: DVec2::ZERO,
        rotation: 0.0,
        scale: Vec2::ONE,
    };

    /// A pure translation.
    ///
    /// # Examples
    ///
    /// ```
    /// # use engine_math::{DVec2, WorldTransform2d};
    /// assert_eq!(WorldTransform2d::from_translation(DVec2::X).translation, DVec2::X);
    /// ```
    #[inline]
    pub const fn from_translation(t: DVec2) -> Self {
        Self {
            translation: t,
            ..Self::IDENTITY
        }
    }

    /// Re-expresses this placement relative to a nearby `origin`, narrowing the translation to
    /// `f32`; rotation and scale are copied.
    ///
    /// # Examples
    ///
    /// ```
    /// # use engine_math::{DVec2, Vec2, WorldTransform2d};
    /// let w = WorldTransform2d::from_translation(DVec2::new(1e9 + 2.0, 0.0));
    /// assert_eq!(w.relative_to(DVec2::new(1e9, 0.0)).translation, Vec2::new(2.0, 0.0));
    /// ```
    #[inline]
    pub fn relative_to(self, origin: DVec2) -> Transform2d {
        Transform2d {
            translation: (self.translation - origin).as_vec2(),
            rotation: self.rotation,
            scale: self.scale,
        }
    }

    /// Places a local `child` under this world-space parent.
    ///
    /// # Examples
    ///
    /// ```
    /// # use engine_math::{DVec2, Transform2d, Vec2, WorldTransform2d};
    /// let parent = WorldTransform2d::from_translation(DVec2::new(1e9, 0.0));
    /// let placed = parent.mul_transform(Transform2d::from_translation(Vec2::Y));
    /// assert_eq!(placed.translation, DVec2::new(1e9, 1.0));
    /// ```
    #[inline]
    #[must_use]
    pub fn mul_transform(self, child: Transform2d) -> Self {
        Self {
            translation: self.translation
                + rotate(self.scale * child.translation, self.rotation).as_dvec2(),
            rotation: wrap_angle(self.rotation + child.rotation),
            scale: self.scale * child.scale,
        }
    }

    /// Transforms a local point into a world point.
    ///
    /// # Examples
    ///
    /// ```
    /// # use engine_math::{DVec2, Vec2, WorldTransform2d};
    /// let w = WorldTransform2d::from_translation(DVec2::new(1e9, 0.0));
    /// assert_eq!(w.transform_point(Vec2::X), DVec2::new(1e9 + 1.0, 0.0));
    /// ```
    #[inline]
    pub fn transform_point(self, p: Vec2) -> DVec2 {
        self.translation + rotate(self.scale * p, self.rotation).as_dvec2()
    }

    /// The equivalent double-precision affine transform.
    ///
    /// # Examples
    ///
    /// ```
    /// # use engine_math::{DAffine2, WorldTransform2d};
    /// assert_eq!(WorldTransform2d::IDENTITY.to_daffine(), DAffine2::IDENTITY);
    /// ```
    #[inline]
    pub fn to_daffine(self) -> DAffine2 {
        let (s, c) = scalar::sin_cos(f64::from(self.rotation));
        let scale = self.scale.as_dvec2();
        DAffine2::from_mat2_translation(
            DMat2::from_cols(
                DVec2::new(c * scale.x, s * scale.x),
                DVec2::new(-s * scale.y, c * scale.y),
            ),
            self.translation,
        )
    }

    /// The exact lift into 3D at depth `z`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use engine_math::{DVec2, WorldTransform2d};
    /// let w = WorldTransform2d::from_translation(DVec2::new(1.0, 2.0)).to_3d(3.0);
    /// assert_eq!(w.translation.to_array(), [1.0, 2.0, 3.0]);
    /// ```
    #[inline]
    pub fn to_3d(self, z: f64) -> WorldTransform {
        WorldTransform {
            translation: DVec3::new(self.translation.x, self.translation.y, z),
            rotation: quat_about_z(self.rotation),
            scale: Vec3::new(self.scale.x, self.scale.y, 1.0),
        }
    }
}

impl Default for WorldTransform2d {
    /// [`WorldTransform2d::IDENTITY`].
    #[inline]
    fn default() -> Self {
        Self::IDENTITY
    }
}

impl Transform {
    /// Projects onto the XY plane: keeps `translation.xy`, the twist about +Z, and `scale.xy`.
    ///
    /// **Lossy:** it drops `translation.z`, any tilt, and `scale.z`. The rotation is extracted by
    /// swing-twist decomposition: the quaternion is split into a swing (the part that tilts +Z)
    /// followed by a twist (a rotation about +Z), and only the twist is kept. When the twist is
    /// undefined (a half turn about X or Y, where `hypot(z, w) < 1e-6`) the result is `0.0`, so
    /// the output is always finite.
    ///
    /// # Examples
    ///
    /// ```
    /// # use engine_math::{Quat, Transform};
    /// // A 0.7 rad turn about Z with a 0.3 rad tilt about X keeps only the 0.7.
    /// let q = Quat::from_rotation_z(0.7) * Quat::from_rotation_x(0.3);
    /// let t2 = Transform::from_rotation(q).to_2d();
    /// assert!((t2.rotation - 0.7).abs() < 1e-5);
    /// ```
    #[inline]
    pub fn to_2d(self) -> Transform2d {
        Transform2d {
            translation: Vec2::new(self.translation.x, self.translation.y),
            rotation: twist_about_z(self.rotation),
            scale: Vec2::new(self.scale.x, self.scale.y),
        }
    }
}

impl WorldTransform {
    /// Projects onto the XY plane with the same rules as [`Transform::to_2d`].
    ///
    /// # Examples
    ///
    /// ```
    /// # use engine_math::{DVec3, WorldTransform};
    /// let w = WorldTransform::from_translation(DVec3::new(1.0, 2.0, 3.0));
    /// assert_eq!(w.to_2d().translation.to_array(), [1.0, 2.0]);
    /// ```
    #[inline]
    pub fn to_2d(self) -> WorldTransform2d {
        WorldTransform2d {
            translation: DVec2::new(self.translation.x, self.translation.y),
            rotation: twist_about_z(self.rotation),
            scale: Vec2::new(self.scale.x, self.scale.y),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Quat, approx_eq};

    #[test]
    fn to_2d_drops_tilt_keeps_twist() {
        let q = Quat::from_rotation_z(0.7) * Quat::from_rotation_x(0.3);
        assert!(approx_eq(
            Transform::from_rotation(q).to_2d().rotation,
            0.7,
            1e-5
        ));
    }

    #[test]
    fn to_2d_of_half_turn_about_x_is_finite() {
        let t2 = Transform::from_rotation(Quat::from_rotation_x(core::f32::consts::PI)).to_2d();
        assert!(t2.rotation.is_finite());
        assert_eq!(t2.rotation.to_bits(), 0.0_f32.to_bits());
    }

    #[test]
    fn to_2d_keeps_xy_translation_and_scale_and_drops_z() {
        let t = Transform {
            translation: crate::Vec3::new(1.0, 2.0, 3.0),
            rotation: Quat::IDENTITY,
            scale: crate::Vec3::new(4.0, 5.0, 6.0),
        };
        let t2 = t.to_2d();
        assert_eq!(t2.translation, Vec2::new(1.0, 2.0));
        assert_eq!(t2.scale, Vec2::new(4.0, 5.0));
    }

    #[test]
    fn wrap_angle_preserves_small_angles_exactly() {
        for a in [1e-6_f32, -3e-7, 1e-4, -1e-9, 0.5, -0.5, PI, 0.0] {
            assert_eq!(
                wrap_angle(a).to_bits(),
                a.to_bits(),
                "wrap_angle({a:e}) changed it"
            );
        }
        assert_eq!(wrap_angle(-PI).to_bits(), PI.to_bits());
        assert!(approx_eq(wrap_angle(3.0 * PI), PI, 1e-5));
        assert!(approx_eq(wrap_angle(TAU + 1e-3), 1e-3, 1e-5));
    }

    #[test]
    fn composing_tiny_rotations_keeps_them() {
        let a = Transform2d {
            rotation: 1e-6,
            ..Transform2d::IDENTITY
        };
        let c = Transform2d::IDENTITY.mul_transform(a);
        assert_eq!(c.rotation.to_bits(), 1e-6_f32.to_bits());
    }

    #[test]
    fn identity_and_default() {
        assert_eq!(Transform2d::default(), Transform2d::IDENTITY);
        assert_eq!(WorldTransform2d::default(), WorldTransform2d::IDENTITY);
    }
}
