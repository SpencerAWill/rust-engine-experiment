//! Property tests for `Transform`: composition, inversion, and agreement with matrices.
//!
//! Scales are uniform: the per-component composition contract matches matrix composition
//! exactly only then (non-uniform parent scale drops shear; see the `Transform` docs).

use engine_math::{Mat4, Quat, Transform, Vec3};
use proptest::prelude::*;

/// WASI has no writable directory for proptest's failure persistence.
fn config() -> ProptestConfig {
    if cfg!(target_os = "wasi") {
        ProptestConfig {
            failure_persistence: None,
            ..ProptestConfig::default()
        }
    } else {
        ProptestConfig::default()
    }
}

fn unit_quat() -> impl Strategy<Value = Quat> {
    (
        prop::array::uniform3(-1.0_f32..1.0),
        -std::f32::consts::PI..std::f32::consts::PI,
    )
        .prop_filter_map("axis too short", |(axis, angle)| {
            Vec3::from(axis)
                .try_normalize()
                .map(|axis| Quat::from_axis_angle(axis, angle))
        })
}

fn uniform_transform() -> impl Strategy<Value = Transform> {
    (
        prop::array::uniform3(-1e3_f32..1e3),
        unit_quat(),
        0.1_f32..10.0,
    )
        .prop_map(|(translation, rotation, scale)| Transform {
            translation: Vec3::from(translation),
            rotation,
            scale: Vec3::splat(scale),
        })
}

fn close(a: f32, b: f32, relative: f32) -> bool {
    (a - b).abs() <= relative * (1.0 + a.abs().max(b.abs()))
}

fn mat_close(a: Mat4, b: Mat4, relative: f32) -> bool {
    a.to_cols_array()
        .iter()
        .zip(b.to_cols_array())
        .all(|(x, y)| close(*x, y, relative))
}

fn vec_close(a: Vec3, b: Vec3, relative: f32) -> bool {
    close(a.x, b.x, relative) && close(a.y, b.y, relative) && close(a.z, b.z, relative)
}

proptest! {
    #![proptest_config(config())]

    #[test]
    fn composition_matches_matrix_composition(a in uniform_transform(), b in uniform_transform()) {
        prop_assert!(mat_close((a * b).to_matrix(), a.to_matrix() * b.to_matrix(), 1e-3));
        prop_assert_eq!(a.mul_transform(b), a * b);
    }

    #[test]
    fn inverse_composes_to_identity(t in uniform_transform()) {
        let identity = (t * t.inverse()).to_matrix();
        prop_assert!(mat_close(identity, Mat4::IDENTITY, 1e-3), "{identity:?}");
        let identity = (t.inverse() * t).to_matrix();
        prop_assert!(mat_close(identity, Mat4::IDENTITY, 1e-3), "{identity:?}");
    }

    #[test]
    fn transform_point_matches_the_matrix(t in uniform_transform(), p in prop::array::uniform3(-1e3_f32..1e3)) {
        let p = Vec3::from(p);
        prop_assert!(vec_close(t.transform_point(p), t.to_matrix().transform_point3(p), 1e-3));
        prop_assert!(vec_close(t.transform_point(p), t.to_affine().transform_point3(p), 1e-3));
        prop_assert!(vec_close(t.transform_vector(p), t.to_matrix().transform_vector3(p), 1e-3));
    }

    #[test]
    fn identity_is_neutral(t in uniform_transform()) {
        prop_assert_eq!(Transform::IDENTITY * t, t);
        prop_assert!(mat_close((t * Transform::IDENTITY).to_matrix(), t.to_matrix(), 1e-6));
    }
}
