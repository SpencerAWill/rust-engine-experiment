//! Property tests for `Transform2d` and `WorldTransform2d`.
//!
//! Scales are uniform where exactness against matrix composition is expected: the per-component
//! contract matches matrix composition exactly only for a uniform parent scale.

use core::f32::consts::{PI, TAU};

use engine_math::{DVec2, Transform, Transform2d, Vec2, Vec3, WorldTransform2d};
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

fn uniform_transform2d() -> impl Strategy<Value = Transform2d> {
    (
        prop::array::uniform2(-1e3_f32..1e3),
        -3.1_f32..3.1,
        0.1_f32..10.0,
    )
        .prop_map(|(translation, rotation, scale)| Transform2d {
            translation: Vec2::from(translation),
            rotation,
            scale: Vec2::splat(scale),
        })
}

fn close(a: f32, b: f32, relative: f32) -> bool {
    (a - b).abs() <= relative * (1.0 + a.abs().max(b.abs()))
}

/// Rounding slack for results formed by adding terms that can cancel: error scales with the
/// size of the terms (about 5e-7 relative to them, measured), not with the result.
const CANCELLATION: f32 = 2e-6;

fn vec_close_with_slack(a: Vec2, b: Vec2, relative: f32, slack: f32) -> bool {
    (a - b).length() <= relative * (1.0 + a.length().max(b.length())) + slack
}

fn vec_close(a: Vec2, b: Vec2, relative: f32) -> bool {
    close(a.x, b.x, relative) && close(a.y, b.y, relative)
}

/// The signed difference between two angles, wrapped into `[-pi, pi]`.
fn angle_diff(a: f32, b: f32) -> f32 {
    let d = (a - b).rem_euclid(TAU);
    if d > PI { d - TAU } else { d }
}

proptest! {
    #![proptest_config(config())]

    #[test]
    fn composition_matches_affine_composition(a in uniform_transform2d(), b in uniform_transform2d()) {
        let composed = (a * b).to_affine();
        let by_matrix = a.to_affine() * b.to_affine();
        for (x, y) in composed.to_cols_array().iter().zip(by_matrix.to_cols_array()) {
            let slack = CANCELLATION * (1.0 + a.translation.length() + a.scale.x * b.translation.length());
            prop_assert!(close(*x, y, 1e-3) || (x - y).abs() <= slack, "{composed:?} vs {by_matrix:?}");
        }
        prop_assert_eq!(a.mul_transform(b), a * b);
    }

    #[test]
    fn inverse_composes_to_identity(t in uniform_transform2d()) {
        for composed in [t * t.inverse(), t.inverse() * t] {
            prop_assert!(composed.translation.length() <= 1e-3 * (1.0 + t.translation.length() * t.scale.x));
            prop_assert!(angle_diff(composed.rotation, 0.0).abs() <= 1e-4);
            prop_assert!(vec_close(composed.scale, Vec2::ONE, 1e-4));
        }
    }

    #[test]
    fn transform_point_agrees_with_affine_and_mat3(t in uniform_transform2d(), p in prop::array::uniform2(-1e3_f32..1e3)) {
        let p = Vec2::from(p);
        let direct = t.transform_point(p);
        let slack = CANCELLATION * (1.0 + t.translation.length() + t.scale.x * p.length());
        prop_assert!(vec_close_with_slack(direct, t.to_affine().transform_point2(p), 1e-3, slack));
        let h = t.to_mat3() * p.extend(1.0);
        prop_assert!(vec_close_with_slack(direct, Vec2::new(h.x, h.y), 1e-3, slack));
        prop_assert!(close(h.z, 1.0, 1e-6));
        prop_assert!(vec_close(t.transform_vector(p), t.to_affine().transform_vector2(p), 1e-3));
    }

    #[test]
    fn to_3d_then_to_2d_recovers_the_transform(t in uniform_transform2d(), z in -1e3_f32..1e3) {
        let back = t.to_3d(z).to_2d();
        prop_assert!(vec_close(back.translation, t.translation, 1e-5));
        prop_assert!(angle_diff(back.rotation, t.rotation).abs() <= 1e-5, "{} vs {}", back.rotation, t.rotation);
        prop_assert!(vec_close(back.scale, t.scale, 1e-5));
        prop_assert_eq!(t.to_3d(z).translation.z.to_bits(), z.to_bits());
        prop_assert_eq!(t.to_3d(z).scale.z.to_bits(), 1.0_f32.to_bits());
    }

    #[test]
    fn rotation_stays_wrapped_after_composition(a in uniform_transform2d(), b in uniform_transform2d()) {
        let r = (a * b).rotation;
        prop_assert!(r > -PI && r <= PI, "rotation {r} escaped (-pi, pi]");
    }

    /// Same 1 mm guarantee as the 3D version: positions up to 1e9 m, offset within 10 km.
    #[test]
    fn world_roundtrip_within_1mm(
        abs in prop::array::uniform2(-1e9_f64..1e9),
        off in prop::array::uniform2(-1e4_f64..1e4),
    ) {
        let origin = DVec2::from(abs);
        let pos = origin + DVec2::from(off);
        let world = WorldTransform2d::from_translation(pos);
        let back = world.relative_to(origin).to_world(origin);
        prop_assert!((back.translation - pos).length() <= 1e-3);
    }

    #[test]
    fn world_transform_point_matches_daffine(
        abs in prop::array::uniform2(-1e9_f64..1e9),
        rotation in -3.1_f32..3.1,
        scale in 0.1_f32..10.0,
        local in prop::array::uniform2(-1e3_f32..1e3),
    ) {
        let world = WorldTransform2d { translation: DVec2::from(abs), rotation, scale: Vec2::splat(scale) };
        let p = Vec2::from(local);
        let direct = world.transform_point(p);
        let affine = world.to_daffine().transform_point2(p.as_dvec2());
        let tolerance = 1e-4 * (1.0 + f64::from(scale) * f64::from(p.length()));
        prop_assert!((direct - affine).length() <= tolerance, "{direct:?} vs {affine:?}");
    }

    #[test]
    fn world_to_3d_and_back(abs in prop::array::uniform2(-1e9_f64..1e9), rotation in -3.1_f32..3.1, z in -1e6_f64..1e6) {
        let world = WorldTransform2d { translation: DVec2::from(abs), rotation, scale: Vec2::ONE };
        let lifted = world.to_3d(z);
        prop_assert_eq!(lifted.translation.z.to_bits(), z.to_bits());
        let back = lifted.to_2d();
        prop_assert_eq!(back.translation, world.translation);
        prop_assert!(angle_diff(back.rotation, rotation).abs() <= 1e-5);
    }
}

#[test]
fn mul_wraps_the_rotation_into_minus_pi_pi() {
    let a = Transform2d::from_rotation(3.0);
    let b = Transform2d::from_rotation(3.0);
    assert!(((a * b).rotation - (6.0 - TAU)).abs() <= 1e-5);
    // Exactly pi stays pi; exactly -pi wraps to +pi (the interval is (-pi, pi]).
    assert_eq!(
        (Transform2d::from_rotation(PI) * Transform2d::IDENTITY)
            .rotation
            .to_bits(),
        PI.to_bits()
    );
    assert_eq!(
        (Transform2d::from_rotation(-PI) * Transform2d::IDENTITY)
            .rotation
            .to_bits(),
        PI.to_bits()
    );
}

#[test]
fn to_3d_is_a_rotation_about_z() {
    let t = Transform2d::from_rotation(core::f32::consts::FRAC_PI_2)
        .with_translation(Vec2::new(1.0, 2.0));
    let lifted: Transform = t.to_3d(5.0);
    let p = lifted.transform_point(Vec3::X);
    assert!((p - Vec3::new(1.0, 3.0, 5.0)).length() < 1e-5, "{p:?}");
}
