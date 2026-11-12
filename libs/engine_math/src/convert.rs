//! Explicit conversion from other coordinate conventions, for import/export edges only.
//!
//! A source convention is described purely as a basis: which signed *source* axis corresponds
//! to each *engine* axis, where engine +Y is "up" and engine +Z is the asset-front direction
//! (see [`axis`](crate::axis)). No semantic "forward" is involved, so there is no
//! camera-versus-asset ambiguity.
//!
//! When the source is left-handed ([`CoordinateSystem::is_right_handed`] is `false`), converting
//! positions and rotations is not enough: the importer must also **reverse triangle winding**, or
//! faces will point inward. This module converts vectors, rotations, scales, and transforms;
//! mesh data is the importer's job.
//!
//! The presets cite their sources in ADR 0005. Only presets that could be verified against
//! vendor documentation are provided; any other convention can be built with
//! [`CoordinateSystem::new`].

use glam::{DVec3, Mat3, Quat, Vec3};

use crate::Transform;

/// A signed coordinate axis of the *source* convention.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SignedAxis {
    /// +X.
    PosX,
    /// -X.
    NegX,
    /// +Y.
    PosY,
    /// -Y.
    NegY,
    /// +Z.
    PosZ,
    /// -Z.
    NegZ,
}

impl SignedAxis {
    /// The axis index: 0 for X, 1 for Y, 2 for Z.
    const fn index(self) -> usize {
        match self {
            Self::PosX | Self::NegX => 0,
            Self::PosY | Self::NegY => 1,
            Self::PosZ | Self::NegZ => 2,
        }
    }

    const fn is_negative(self) -> bool {
        matches!(self, Self::NegX | Self::NegY | Self::NegZ)
    }

    fn sign(self) -> f32 {
        if self.is_negative() { -1.0 } else { 1.0 }
    }

    fn sign_f64(self) -> f64 {
        if self.is_negative() { -1.0 } else { 1.0 }
    }
}

/// A source convention, described as a basis: which signed *source* axis corresponds to each
/// engine axis.
///
/// # Examples
///
/// ```
/// use engine_math::convert::{CoordinateSystem, vec3_to_engine};
/// use engine_math::{Vec3, axis};
///
/// // Unity is left-handed with +Y up and +Z forward; its +Y is the engine's up.
/// let unity = CoordinateSystem::UNITY;
/// assert_eq!(vec3_to_engine(Vec3::Y, unity), axis::UP);
/// assert!(!unity.is_right_handed());
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct CoordinateSystem {
    /// The source axis that corresponds to engine +X.
    pub x: SignedAxis,
    /// The source axis that corresponds to engine +Y (up).
    pub y: SignedAxis,
    /// The source axis that corresponds to engine +Z (asset front).
    pub z: SignedAxis,
}

impl CoordinateSystem {
    /// The engine's own convention: the identity mapping.
    pub const ENGINE: Self = Self {
        x: SignedAxis::PosX,
        y: SignedAxis::PosY,
        z: SignedAxis::PosZ,
    };

    /// glTF 2.0: right-handed, +Y up, asset front +Z, asset right -X. The engine adopts it, so
    /// this is the identity mapping (a test asserts `ENGINE == GLTF`).
    pub const GLTF: Self = Self::ENGINE;

    /// Unity: left-handed, +X right, +Y up, +Z forward. Up and asset front land on engine +Y and
    /// +Z, and the handedness flip negates the asset-right axis (Unity's +X is engine -X), which
    /// is also what the `UnityGLTF` importer does. See ADR 0005.
    pub const UNITY: Self = Self {
        x: SignedAxis::NegX,
        y: SignedAxis::PosY,
        z: SignedAxis::PosZ,
    };

    /// Builds a convention from its three axes.
    ///
    /// Returns `None` if two fields name the same source axis (ignoring sign), since the three
    /// axes must form a basis.
    ///
    /// # Examples
    ///
    /// ```
    /// use engine_math::convert::{CoordinateSystem, SignedAxis};
    ///
    /// assert!(CoordinateSystem::new(SignedAxis::PosX, SignedAxis::PosZ, SignedAxis::NegY).is_some());
    /// assert!(CoordinateSystem::new(SignedAxis::PosX, SignedAxis::NegX, SignedAxis::PosZ).is_none());
    /// ```
    pub const fn new(x: SignedAxis, y: SignedAxis, z: SignedAxis) -> Option<Self> {
        if x.index() == y.index() || x.index() == z.index() || y.index() == z.index() {
            None
        } else {
            Some(Self { x, y, z })
        }
    }

    /// Whether this convention has the same handedness as the engine (right-handed).
    ///
    /// # Examples
    ///
    /// ```
    /// # use engine_math::convert::CoordinateSystem;
    /// assert!(CoordinateSystem::GLTF.is_right_handed());
    /// assert!(!CoordinateSystem::UNITY.is_right_handed());
    /// ```
    pub const fn is_right_handed(self) -> bool {
        let even_permutation = matches!(
            (self.x.index(), self.y.index(), self.z.index()),
            (0, 1, 2) | (1, 2, 0) | (2, 0, 1)
        );
        let negatives = negative(self.x) + negative(self.y) + negative(self.z);
        // det = (permutation parity) * (product of signs); it is +1 iff the two parities agree.
        even_permutation == (negatives.is_multiple_of(2))
    }

    /// The signed permutation matrix mapping vectors in this convention to engine space.
    ///
    /// Column `i` is the engine axis that source axis `i` maps to.
    ///
    /// # Examples
    ///
    /// ```
    /// # use engine_math::convert::CoordinateSystem;
    /// # use engine_math::{Mat3, Vec3};
    /// let m = CoordinateSystem::UNITY.to_engine_matrix();
    /// assert_eq!(m * Vec3::X, Vec3::NEG_X);
    /// assert_eq!(CoordinateSystem::ENGINE.to_engine_matrix(), Mat3::IDENTITY);
    /// ```
    pub fn to_engine_matrix(self) -> Mat3 {
        let axes = [self.x, self.y, self.z];
        let entry = |row: usize, col: usize| {
            if axes[row].index() == col {
                axes[row].sign()
            } else {
                0.0
            }
        };
        Mat3::from_cols(
            Vec3::new(entry(0, 0), entry(1, 0), entry(2, 0)),
            Vec3::new(entry(0, 1), entry(1, 1), entry(2, 1)),
            Vec3::new(entry(0, 2), entry(1, 2), entry(2, 2)),
        )
    }

    /// `+1.0` if right-handed, else `-1.0`: the determinant of the engine matrix.
    fn handedness(self) -> f32 {
        if self.is_right_handed() { 1.0 } else { -1.0 }
    }
}

const fn negative(axis: SignedAxis) -> usize {
    if axis.is_negative() { 1 } else { 0 }
}

/// Converts a vector (position, direction, or offset) from `from` to engine space.
///
/// # Examples
///
/// ```
/// use engine_math::convert::{CoordinateSystem, vec3_to_engine};
/// use engine_math::Vec3;
///
/// assert_eq!(vec3_to_engine(Vec3::new(1.0, 2.0, 3.0), CoordinateSystem::UNITY), Vec3::new(-1.0, 2.0, 3.0));
/// ```
pub fn vec3_to_engine(v: Vec3, from: CoordinateSystem) -> Vec3 {
    Vec3::new(
        from.x.sign() * v[from.x.index()],
        from.y.sign() * v[from.y.index()],
        from.z.sign() * v[from.z.index()],
    )
}

/// Inverse of [`vec3_to_engine`]: engine space to the convention `to`.
///
/// # Examples
///
/// ```
/// use engine_math::convert::{CoordinateSystem, vec3_from_engine, vec3_to_engine};
/// use engine_math::Vec3;
///
/// let v = Vec3::new(1.0, 2.0, 3.0);
/// assert_eq!(vec3_from_engine(vec3_to_engine(v, CoordinateSystem::UNITY), CoordinateSystem::UNITY), v);
/// ```
pub fn vec3_from_engine(v: Vec3, to: CoordinateSystem) -> Vec3 {
    let mut out = Vec3::ZERO;
    out[to.x.index()] = to.x.sign() * v.x;
    out[to.y.index()] = to.y.sign() * v.y;
    out[to.z.index()] = to.z.sign() * v.z;
    out
}

/// Double-precision [`vec3_to_engine`].
///
/// # Examples
///
/// ```
/// use engine_math::convert::{CoordinateSystem, dvec3_to_engine};
/// use engine_math::DVec3;
///
/// assert_eq!(dvec3_to_engine(DVec3::X, CoordinateSystem::UNITY), DVec3::NEG_X);
/// ```
pub fn dvec3_to_engine(v: DVec3, from: CoordinateSystem) -> DVec3 {
    DVec3::new(
        from.x.sign_f64() * v[from.x.index()],
        from.y.sign_f64() * v[from.y.index()],
        from.z.sign_f64() * v[from.z.index()],
    )
}

/// Double-precision [`vec3_from_engine`].
///
/// # Examples
///
/// ```
/// use engine_math::convert::{CoordinateSystem, dvec3_from_engine};
/// use engine_math::DVec3;
///
/// assert_eq!(dvec3_from_engine(DVec3::NEG_X, CoordinateSystem::UNITY), DVec3::X);
/// ```
pub fn dvec3_from_engine(v: DVec3, to: CoordinateSystem) -> DVec3 {
    let mut out = DVec3::ZERO;
    out[to.x.index()] = to.x.sign_f64() * v.x;
    out[to.y.index()] = to.y.sign_f64() * v.y;
    out[to.z.index()] = to.z.sign_f64() * v.z;
    out
}

/// Converts a rotation from `from` to engine space: the engine rotation is `M * R * Mᵀ`, where
/// `M` is [`CoordinateSystem::to_engine_matrix`] and `R` is the source rotation matrix. It is a
/// proper rotation even when handedness flips.
///
/// For a signed permutation `M` this equals a rotation about `det(M) * (M * axis)` by the same
/// angle, so it is computed by permuting and negating the quaternion's vector part (no trig, no
/// matrices), which makes the conversion exact and exactly invertible.
///
/// # Examples
///
/// ```
/// use engine_math::convert::{CoordinateSystem, quat_to_engine};
/// use engine_math::Quat;
///
/// // Mirroring X reverses the sense of a rotation about Z.
/// let q = quat_to_engine(Quat::from_rotation_z(0.5), CoordinateSystem::UNITY);
/// assert!(q.abs_diff_eq(Quat::from_rotation_z(-0.5), 1e-6));
/// ```
pub fn quat_to_engine(q: Quat, from: CoordinateSystem) -> Quat {
    let v = from.to_engine_matrix() * Vec3::new(q.x, q.y, q.z) * from.handedness();
    Quat::from_xyzw(v.x, v.y, v.z, q.w)
}

/// Inverse of [`quat_to_engine`].
///
/// # Examples
///
/// ```
/// use engine_math::convert::{CoordinateSystem, quat_from_engine, quat_to_engine};
/// use engine_math::Quat;
///
/// let q = Quat::from_rotation_y(1.0);
/// let unity = CoordinateSystem::UNITY;
/// assert_eq!(quat_from_engine(quat_to_engine(q, unity), unity), q);
/// ```
pub fn quat_from_engine(q: Quat, to: CoordinateSystem) -> Quat {
    let v = to.to_engine_matrix().transpose() * Vec3::new(q.x, q.y, q.z) * to.handedness();
    Quat::from_xyzw(v.x, v.y, v.z, q.w)
}

/// Converts per-axis scale from `from` to engine space by permuting its components. Signs are
/// not applied: a scale's axes are unsigned magnitudes.
///
/// # Examples
///
/// ```
/// use engine_math::convert::{CoordinateSystem, SignedAxis, scale_to_engine};
/// use engine_math::Vec3;
///
/// // A Z-up source (x: +X, y: +Z, z: -Y), as many DCC tools use: engine Y takes the source Z scale.
/// let z_up = CoordinateSystem::new(SignedAxis::PosX, SignedAxis::PosZ, SignedAxis::NegY).unwrap();
/// assert_eq!(scale_to_engine(Vec3::new(1.0, 2.0, 3.0), z_up), Vec3::new(1.0, 3.0, 2.0));
/// ```
pub fn scale_to_engine(s: Vec3, from: CoordinateSystem) -> Vec3 {
    Vec3::new(s[from.x.index()], s[from.y.index()], s[from.z.index()])
}

/// Inverse of [`scale_to_engine`].
///
/// # Examples
///
/// ```
/// use engine_math::convert::{CoordinateSystem, scale_from_engine, scale_to_engine};
/// use engine_math::Vec3;
///
/// let s = Vec3::new(1.0, 2.0, 3.0);
/// assert_eq!(scale_from_engine(scale_to_engine(s, CoordinateSystem::UNITY), CoordinateSystem::UNITY), s);
/// ```
pub fn scale_from_engine(s: Vec3, to: CoordinateSystem) -> Vec3 {
    let mut out = Vec3::ZERO;
    out[to.x.index()] = s.x;
    out[to.y.index()] = s.y;
    out[to.z.index()] = s.z;
    out
}

/// Converts a whole transform from `from` to engine space.
///
/// # Examples
///
/// ```
/// use engine_math::convert::{CoordinateSystem, transform_to_engine};
/// use engine_math::{Transform, Vec3};
///
/// let t = Transform::from_translation(Vec3::new(1.0, 2.0, 3.0));
/// assert_eq!(transform_to_engine(t, CoordinateSystem::UNITY).translation, Vec3::new(-1.0, 2.0, 3.0));
/// ```
pub fn transform_to_engine(t: Transform, from: CoordinateSystem) -> Transform {
    Transform {
        translation: vec3_to_engine(t.translation, from),
        rotation: quat_to_engine(t.rotation, from),
        scale: scale_to_engine(t.scale, from),
    }
}

/// Inverse of [`transform_to_engine`].
///
/// # Examples
///
/// ```
/// use engine_math::convert::{CoordinateSystem, transform_from_engine, transform_to_engine};
/// use engine_math::{Transform, Vec3};
///
/// let t = Transform::from_translation(Vec3::new(1.0, 2.0, 3.0));
/// let unity = CoordinateSystem::UNITY;
/// assert_eq!(transform_from_engine(transform_to_engine(t, unity), unity), t);
/// ```
pub fn transform_from_engine(t: Transform, to: CoordinateSystem) -> Transform {
    Transform {
        translation: vec3_from_engine(t.translation, to),
        rotation: quat_from_engine(t.rotation, to),
        scale: scale_from_engine(t.scale, to),
    }
}
