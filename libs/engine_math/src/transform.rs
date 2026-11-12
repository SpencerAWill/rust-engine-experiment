//! [`Transform`]: translation, rotation, and scale in single precision.

use core::ops::Mul;

use glam::{Affine3A, Mat4, Quat, Vec3};

/// Translation, rotation, and scale: local or relative placement in single precision.
///
/// `#[repr(C)]` fixes the field order, and the layout is pinned by static assertions (see the
/// crate docs). Use [`WorldTransform`](crate::WorldTransform) for absolute placement far from
/// the origin; `Transform` is for local data: parent-to-child offsets, GPU data, and
/// physics-local space.
///
/// `Transform` is a composition and interchange type, not a good default component for every
/// entity: it is 48 bytes, so something that only has a position should store a bare
/// [`Vec3`] (or `Vec2`) and build a `Transform` only where it composes or hands data over
/// (see ADR 0006).
///
/// Composition is per-component, not by multiplying matrices (see
/// [`mul_transform`](Self::mul_transform)). It matches matrix composition exactly (within float
/// epsilon) when the parent's scale is uniform.
///
/// # Examples
///
/// ```
/// use engine_math::{Quat, Transform, Vec3};
///
/// let parent = Transform::from_translation(Vec3::new(10.0, 0.0, 0.0))
///     .with_rotation(Quat::from_rotation_z(core::f32::consts::FRAC_PI_2));
/// let child = Transform::from_translation(Vec3::X);
///
/// // The child's +X offset is rotated onto +Y by the parent.
/// let placed = parent * child;
/// assert!((placed.translation - Vec3::new(10.0, 1.0, 0.0)).length() < 1e-6);
/// ```
#[derive(Clone, Copy, Debug, PartialEq)]
#[repr(C)]
pub struct Transform {
    /// Position.
    pub translation: Vec3,
    /// Orientation. Expected to be a unit quaternion.
    pub rotation: Quat,
    /// Scale along each axis.
    pub scale: Vec3,
}

impl Transform {
    /// No translation, no rotation, unit scale.
    pub const IDENTITY: Self = Self {
        translation: Vec3::ZERO,
        rotation: Quat::IDENTITY,
        scale: Vec3::ONE,
    };

    /// A pure translation.
    ///
    /// # Examples
    ///
    /// ```
    /// # use engine_math::{Transform, Vec3};
    /// assert_eq!(Transform::from_translation(Vec3::X).translation, Vec3::X);
    /// ```
    #[inline]
    pub const fn from_translation(t: Vec3) -> Self {
        Self {
            translation: t,
            ..Self::IDENTITY
        }
    }

    /// A pure rotation.
    ///
    /// # Examples
    ///
    /// ```
    /// # use engine_math::{Quat, Transform};
    /// assert_eq!(Transform::from_rotation(Quat::IDENTITY), Transform::IDENTITY);
    /// ```
    #[inline]
    pub const fn from_rotation(r: Quat) -> Self {
        Self {
            rotation: r,
            ..Self::IDENTITY
        }
    }

    /// A pure scale.
    ///
    /// # Examples
    ///
    /// ```
    /// # use engine_math::{Transform, Vec3};
    /// assert_eq!(Transform::from_scale(Vec3::splat(2.0)).scale, Vec3::splat(2.0));
    /// ```
    #[inline]
    pub const fn from_scale(s: Vec3) -> Self {
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
    /// # use engine_math::{Transform, Vec3};
    /// assert_eq!(Transform::IDENTITY.with_translation(Vec3::Y).translation, Vec3::Y);
    /// ```
    #[inline]
    #[must_use]
    pub const fn with_translation(mut self, t: Vec3) -> Self {
        self.translation = t;
        self
    }

    /// This transform with its rotation replaced.
    ///
    /// # Examples
    ///
    /// ```
    /// # use engine_math::{Quat, Transform};
    /// let q = Quat::from_rotation_x(1.0);
    /// assert_eq!(Transform::IDENTITY.with_rotation(q).rotation, q);
    /// ```
    #[inline]
    #[must_use]
    pub const fn with_rotation(mut self, r: Quat) -> Self {
        self.rotation = r;
        self
    }

    /// This transform with its scale replaced.
    ///
    /// # Examples
    ///
    /// ```
    /// # use engine_math::{Transform, Vec3};
    /// assert_eq!(Transform::IDENTITY.with_scale(Vec3::ONE * 3.0).scale, Vec3::splat(3.0));
    /// ```
    #[inline]
    #[must_use]
    pub const fn with_scale(mut self, s: Vec3) -> Self {
        self.scale = s;
        self
    }

    /// `self * child`: places `child` (expressed in `self`'s local space) into `self`'s parent
    /// space, by per-component composition:
    ///
    /// ```text
    /// translation = self.translation + self.rotation * (self.scale * child.translation)
    /// rotation    = self.rotation * child.rotation
    /// scale       = self.scale * child.scale
    /// ```
    ///
    /// This matches [`Mat4`] composition exactly (within float epsilon) when `self.scale` is
    /// uniform. With a non-uniform parent scale and a rotated child, the shear a matrix would
    /// carry is dropped: this is a documented property of the representation, not an
    /// approximation within epsilon.
    ///
    /// # Examples
    ///
    /// ```
    /// # use engine_math::{Transform, Vec3};
    /// let parent = Transform::from_translation(Vec3::new(1.0, 2.0, 3.0)).with_scale(Vec3::splat(2.0));
    /// let child = Transform::from_translation(Vec3::X);
    /// assert_eq!(parent.mul_transform(child).translation, Vec3::new(3.0, 2.0, 3.0));
    /// ```
    #[inline]
    #[must_use]
    pub fn mul_transform(self, child: Self) -> Self {
        Self {
            translation: self.translation + self.rotation * (self.scale * child.translation),
            rotation: self.rotation * child.rotation,
            scale: self.scale * child.scale,
        }
    }

    /// The inverse, by per-component inversion.
    ///
    /// Exact (`t * t.inverse()` is the identity within float epsilon) when the scale is uniform;
    /// with a non-uniform scale it is the per-component inverse, which does not undo a rotated
    /// non-uniformly scaled transform exactly. A zero scale yields non-finite values.
    ///
    /// # Examples
    ///
    /// ```
    /// # use engine_math::{Transform, Vec3};
    /// let t = Transform::from_translation(Vec3::new(1.0, 2.0, 3.0)).with_scale(Vec3::splat(2.0));
    /// let identity = t * t.inverse();
    /// assert!((identity.translation).length() < 1e-6);
    /// ```
    #[inline]
    #[must_use]
    pub fn inverse(self) -> Self {
        let rotation = self.rotation.inverse();
        let scale = Vec3::ONE / self.scale;
        Self {
            translation: -(rotation * (scale * self.translation)),
            rotation,
            scale,
        }
    }

    /// The equivalent 4x4 matrix.
    ///
    /// # Examples
    ///
    /// ```
    /// # use engine_math::{Mat4, Transform};
    /// assert_eq!(Transform::IDENTITY.to_matrix(), Mat4::IDENTITY);
    /// ```
    #[inline]
    pub fn to_matrix(self) -> Mat4 {
        Mat4::from_scale_rotation_translation(self.scale, self.rotation, self.translation)
    }

    /// The equivalent affine transform.
    ///
    /// # Examples
    ///
    /// ```
    /// # use engine_math::{Affine3A, Transform};
    /// assert_eq!(Transform::IDENTITY.to_affine(), Affine3A::IDENTITY);
    /// ```
    #[inline]
    pub fn to_affine(self) -> Affine3A {
        Affine3A::from_scale_rotation_translation(self.scale, self.rotation, self.translation)
    }

    /// Transforms a point: scale, then rotate, then translate.
    ///
    /// # Examples
    ///
    /// ```
    /// # use engine_math::{Transform, Vec3};
    /// let t = Transform::from_translation(Vec3::new(1.0, 0.0, 0.0));
    /// assert_eq!(t.transform_point(Vec3::Y), Vec3::new(1.0, 1.0, 0.0));
    /// ```
    #[inline]
    pub fn transform_point(self, p: Vec3) -> Vec3 {
        self.translation + self.transform_vector(p)
    }

    /// Transforms a direction or offset: scale, then rotate, with no translation.
    ///
    /// # Examples
    ///
    /// ```
    /// # use engine_math::{Transform, Vec3};
    /// let t = Transform::from_translation(Vec3::new(1.0, 0.0, 0.0));
    /// assert_eq!(t.transform_vector(Vec3::Y), Vec3::Y);
    /// ```
    #[inline]
    pub fn transform_vector(self, v: Vec3) -> Vec3 {
        self.rotation * (self.scale * v)
    }
}

impl Default for Transform {
    /// [`Transform::IDENTITY`].
    #[inline]
    fn default() -> Self {
        Self::IDENTITY
    }
}

impl Mul for Transform {
    type Output = Self;

    /// [`Transform::mul_transform`].
    #[inline]
    fn mul(self, rhs: Self) -> Self {
        self.mul_transform(rhs)
    }
}
