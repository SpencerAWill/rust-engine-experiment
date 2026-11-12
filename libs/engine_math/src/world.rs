//! [`WorldTransform`]: large-world placement with `f64` translation.

use glam::{DAffine3, DVec3, Quat, Vec3};

use crate::Transform;

/// High-precision placement in the world: the translation is `f64`, while rotation and scale
/// stay `f32` (their magnitudes are bounded, so they do not lose precision far from the origin).
///
/// **Rule:** `WorldTransform` is for absolute placement; [`Transform`] is for local or relative
/// data. Narrowing from `f64` to `f32` happens only through [`relative_to`](Self::relative_to).
/// There is deliberately no `From` impl, so precision loss is always explicit at the call site.
///
/// **Precision guarantee (tested):** for any absolute position up to 1e9 m from the origin (past
/// the Earth–Moon distance), and a local offset within 10 km of the chosen origin,
/// [`relative_to`](Self::relative_to) followed by [`Transform::to_world`] round-trips with an
/// error of at most 1 mm. See ADR 0006.
///
/// `#[repr(C)]` fixes the field order, and the layout is pinned by static assertions.
///
/// # Examples
///
/// ```
/// use engine_math::{DVec3, WorldTransform};
///
/// let camera = DVec3::new(6.4e6, 0.0, 0.0);
/// let object = WorldTransform::from_translation(camera + DVec3::new(3.0, 4.0, 0.0));
///
/// // Work in a precise local frame near the camera, then go back.
/// let local = object.relative_to(camera);
/// assert!((local.translation.length() - 5.0).abs() < 1e-6);
/// assert_eq!(local.to_world(camera).translation, object.translation);
/// ```
#[derive(Clone, Copy, Debug, PartialEq)]
#[repr(C)]
pub struct WorldTransform {
    /// Absolute position, in `f64`.
    pub translation: DVec3,
    /// Orientation. Expected to be a unit quaternion.
    pub rotation: Quat,
    /// Scale along each axis.
    pub scale: Vec3,
}

impl WorldTransform {
    /// At the origin, no rotation, unit scale.
    pub const IDENTITY: Self = Self {
        translation: DVec3::ZERO,
        rotation: Quat::IDENTITY,
        scale: Vec3::ONE,
    };

    /// A pure translation.
    ///
    /// # Examples
    ///
    /// ```
    /// # use engine_math::{DVec3, WorldTransform};
    /// assert_eq!(WorldTransform::from_translation(DVec3::X).translation, DVec3::X);
    /// ```
    #[inline]
    pub const fn from_translation(t: DVec3) -> Self {
        Self {
            translation: t,
            ..Self::IDENTITY
        }
    }

    /// Re-expresses this placement relative to a nearby `origin` (for example the camera or a
    /// physics island's center): `translation = (self.translation - origin)` narrowed to `f32`,
    /// with rotation and scale copied.
    ///
    /// # Examples
    ///
    /// ```
    /// # use engine_math::{DVec3, Vec3, WorldTransform};
    /// let w = WorldTransform::from_translation(DVec3::new(1e9 + 2.0, 0.0, 0.0));
    /// assert_eq!(w.relative_to(DVec3::new(1e9, 0.0, 0.0)).translation, Vec3::new(2.0, 0.0, 0.0));
    /// ```
    #[inline]
    pub fn relative_to(self, origin: DVec3) -> Transform {
        Transform {
            translation: (self.translation - origin).as_vec3(),
            rotation: self.rotation,
            scale: self.scale,
        }
    }

    /// Places a local (`f32`) `child` under this world-space parent.
    ///
    /// # Examples
    ///
    /// ```
    /// # use engine_math::{DVec3, Transform, Vec3, WorldTransform};
    /// let parent = WorldTransform::from_translation(DVec3::new(1e9, 0.0, 0.0));
    /// let placed = parent.mul_transform(Transform::from_translation(Vec3::Y));
    /// assert_eq!(placed.translation, DVec3::new(1e9, 1.0, 0.0));
    /// ```
    #[inline]
    #[must_use]
    pub fn mul_transform(self, child: Transform) -> Self {
        Self {
            translation: self.translation
                + (self.rotation * (self.scale * child.translation)).as_dvec3(),
            rotation: self.rotation * child.rotation,
            scale: self.scale * child.scale,
        }
    }

    /// Transforms a local (`f32`) point into a world (`f64`) point.
    ///
    /// # Examples
    ///
    /// ```
    /// # use engine_math::{DVec3, Vec3, WorldTransform};
    /// let w = WorldTransform::from_translation(DVec3::new(1e9, 0.0, 0.0));
    /// assert_eq!(w.transform_point(Vec3::X), DVec3::new(1e9 + 1.0, 0.0, 0.0));
    /// ```
    #[inline]
    pub fn transform_point(self, p: Vec3) -> DVec3 {
        self.translation + (self.rotation * (self.scale * p)).as_dvec3()
    }

    /// The equivalent double-precision affine transform.
    ///
    /// # Examples
    ///
    /// ```
    /// # use engine_math::{DAffine3, WorldTransform};
    /// assert_eq!(WorldTransform::IDENTITY.to_daffine(), DAffine3::IDENTITY);
    /// ```
    #[inline]
    pub fn to_daffine(self) -> DAffine3 {
        DAffine3::from_scale_rotation_translation(
            self.scale.as_dvec3(),
            self.rotation.as_dquat(),
            self.translation,
        )
    }
}

impl Default for WorldTransform {
    /// [`WorldTransform::IDENTITY`].
    #[inline]
    fn default() -> Self {
        Self::IDENTITY
    }
}

impl Transform {
    /// The inverse of [`WorldTransform::relative_to`]: lifts this local transform into world
    /// space by adding `origin` to the translation in `f64`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use engine_math::{DVec3, Transform, Vec3};
    /// let t = Transform::from_translation(Vec3::Y);
    /// assert_eq!(t.to_world(DVec3::new(1e9, 0.0, 0.0)).translation, DVec3::new(1e9, 1.0, 0.0));
    /// ```
    #[inline]
    pub fn to_world(self, origin: DVec3) -> WorldTransform {
        WorldTransform {
            translation: origin + self.translation.as_dvec3(),
            rotation: self.rotation,
            scale: self.scale,
        }
    }
}
