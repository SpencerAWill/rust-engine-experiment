//! Coordinate-system conversion: presets, round trips, and algebraic consistency.
#![forbid(unsafe_code)]

use engine_math::convert::{
    CoordinateSystem, SignedAxis, dvec3_from_engine, dvec3_to_engine, quat_from_engine,
    quat_to_engine, scale_from_engine, scale_to_engine, transform_from_engine, transform_to_engine,
    vec3_from_engine, vec3_to_engine,
};
use engine_math::{DVec3, Mat3, Quat, Transform, Vec3, axis};
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

const AXES: [SignedAxis; 6] = [
    SignedAxis::PosX,
    SignedAxis::NegX,
    SignedAxis::PosY,
    SignedAxis::NegY,
    SignedAxis::PosZ,
    SignedAxis::NegZ,
];

/// All 48 valid coordinate systems (the signed permutations), found by brute force.
fn all_systems() -> Vec<CoordinateSystem> {
    let mut systems = Vec::new();
    for x in AXES {
        for y in AXES {
            for z in AXES {
                if let Some(system) = CoordinateSystem::new(x, y, z) {
                    systems.push(system);
                }
            }
        }
    }
    systems
}

fn any_system() -> impl Strategy<Value = CoordinateSystem> {
    prop::sample::select(all_systems())
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

fn vec_close_with_slack(a: Vec3, b: Vec3, eps: f32, slack: f32) -> bool {
    (a - b).length() <= eps * (1.0 + a.length().max(b.length())) + slack
}

fn vec_close(a: Vec3, b: Vec3, eps: f32) -> bool {
    (a - b).length() <= eps * (1.0 + a.length().max(b.length()))
}

#[test]
fn engine_is_gltf_and_identity() {
    assert_eq!(CoordinateSystem::ENGINE, CoordinateSystem::GLTF);
    assert_eq!(
        vec3_to_engine(Vec3::new(1.0, 2.0, 3.0), CoordinateSystem::GLTF),
        Vec3::new(1.0, 2.0, 3.0)
    );
    assert_eq!(
        vec3_to_engine(Vec3::Z, CoordinateSystem::GLTF),
        axis::ASSET_FRONT
    );
    assert_eq!(vec3_to_engine(Vec3::Y, CoordinateSystem::GLTF), axis::UP);
    assert_eq!(CoordinateSystem::ENGINE.to_engine_matrix(), Mat3::IDENTITY);
    assert!(CoordinateSystem::ENGINE.is_right_handed());
}

#[test]
fn new_rejects_repeated_axis() {
    assert!(CoordinateSystem::new(SignedAxis::PosX, SignedAxis::NegX, SignedAxis::PosZ).is_none());
    assert!(CoordinateSystem::new(SignedAxis::PosX, SignedAxis::PosY, SignedAxis::PosY).is_none());
    assert!(CoordinateSystem::new(SignedAxis::NegZ, SignedAxis::PosX, SignedAxis::PosY).is_some());
    assert_eq!(
        all_systems().len(),
        48,
        "3! permutations times 2^3 sign choices"
    );
}

#[test]
fn exactly_half_of_all_systems_are_right_handed() {
    let systems = all_systems();
    let right = systems.iter().filter(|s| s.is_right_handed()).count();
    assert_eq!(right, 24);
    for system in systems {
        let det = system.to_engine_matrix().determinant();
        assert_eq!(
            system.is_right_handed(),
            det > 0.0,
            "{system:?} has determinant {det}"
        );
        assert!((det.abs() - 1.0).abs() < 1e-6);
    }
}

/// Unity: left-handed, +X right, +Y up, +Z forward (Unity manual, "Quaternions and Euler
/// rotations"); its glTF importers negate X (`UnityGLTF`), so the asset-right axis flips.
#[test]
fn unity_preset_maps_up_and_front_and_flips_handedness() {
    let unity = CoordinateSystem::UNITY;
    assert_eq!(vec3_to_engine(Vec3::Y, unity), axis::UP);
    assert_eq!(vec3_to_engine(Vec3::Z, unity), axis::ASSET_FRONT);
    assert!(!unity.is_right_handed(), "Unity is left-handed");
    // Unity's +X is the asset's right, which is -X in the engine (glTF: -X is right).
    assert_eq!(vec3_to_engine(Vec3::X, unity), axis::LEFT);
}

#[test]
fn directional_constants_are_consistent() {
    assert_eq!(axis::RIGHT, -axis::LEFT);
    assert_eq!(axis::UP, -axis::DOWN);
    assert_eq!(axis::VIEW_FORWARD, -axis::ASSET_FRONT);
    // Right-handed: right x up = toward the viewer (+Z), which is opposite view-forward.
    assert_eq!(axis::RIGHT.cross(axis::UP), -axis::VIEW_FORWARD);
    assert_eq!(axis::RIGHT_2D, engine_math::Vec2::X);
    assert_eq!(axis::UP_2D, engine_math::Vec2::Y);
}

proptest! {
    #![proptest_config(config())]

    #[test]
    fn vector_roundtrips_for_every_system(system in any_system(), v in prop::array::uniform3(-1e3_f32..1e3), d in prop::array::uniform3(-1e9_f64..1e9)) {
        let v = Vec3::from(v);
        prop_assert_eq!(vec3_from_engine(vec3_to_engine(v, system), system), v);
        let d = DVec3::from(d);
        prop_assert_eq!(dvec3_from_engine(dvec3_to_engine(d, system), system), d);
        // Conversion never changes length.
        // Components are only permuted and negated, so the length is unchanged (up to the
        // order of the floating-point additions).
        let (a, b) = (vec3_to_engine(v, system).length_squared(), v.length_squared());
        prop_assert!((a - b).abs() <= 1e-6 * b.max(1.0), "{a} vs {b}");
    }

    #[test]
    fn scale_roundtrips_and_permutes(system in any_system(), s in prop::array::uniform3(0.1_f32..10.0)) {
        let s = Vec3::from(s);
        let engine = scale_to_engine(s, system);
        prop_assert_eq!(scale_from_engine(engine, system), s);
        prop_assert!(engine.min_element() > 0.0, "scale stays positive: signs are not applied");
        let (a, b) = (engine.x * engine.y * engine.z, s.x * s.y * s.z);
        prop_assert!((a - b).abs() <= 1e-6 * b, "{a} vs {b}");
    }

    #[test]
    fn quaternion_stays_a_proper_rotation_and_roundtrips(system in any_system(), q in unit_quat()) {
        let engine = quat_to_engine(q, system);
        prop_assert!((engine.length() - 1.0).abs() < 1e-5, "length {}", engine.length());
        let det = Mat3::from_quat(engine).determinant();
        prop_assert!((det - 1.0).abs() < 1e-4, "determinant {det}");
        let back = quat_from_engine(engine, system);
        prop_assert!(back.dot(q).abs() > 1.0 - 1e-5, "{back:?} vs {q:?}");
    }

    /// The closed form used by `quat_to_engine` equals the spec's definition, `M * R * M^T`.
    #[test]
    fn quaternion_matches_the_matrix_definition(system in any_system(), q in unit_quat()) {
        let m = system.to_engine_matrix();
        let by_matrix = Quat::from_mat3(&(m * Mat3::from_quat(q) * m.transpose()));
        prop_assert!(quat_to_engine(q, system).dot(by_matrix).abs() > 1.0 - 1e-5);
    }

    #[test]
    fn rotation_conversion_is_algebraically_consistent(system in any_system(), q in unit_quat(), v in prop::array::uniform3(-1e2_f32..1e2)) {
        let v = Vec3::from(v);
        let lhs = quat_to_engine(q, system) * vec3_to_engine(v, system);
        let rhs = vec3_to_engine(q * v, system);
        prop_assert!(vec_close(lhs, rhs, 1e-4), "{lhs:?} vs {rhs:?}");
    }

    #[test]
    fn transform_conversion_is_consistent_and_roundtrips(
        system in any_system(),
        t in prop::array::uniform3(-1e3_f32..1e3),
        q in unit_quat(),
        s in prop::array::uniform3(0.1_f32..10.0),
        p in prop::array::uniform3(-1e2_f32..1e2),
    ) {
        let transform = Transform { translation: Vec3::from(t), rotation: q, scale: Vec3::from(s) };
        let p = Vec3::from(p);
        let engine = transform_to_engine(transform, system);
        let lhs = engine.transform_point(vec3_to_engine(p, system));
        let rhs = vec3_to_engine(transform.transform_point(p), system);
        // translation + rotated(scale * p) can cancel: slack proportional to the size of both terms.
        let slack = 2e-6 * (1.0 + transform.translation.length() + transform.scale.max_element() * p.length());
        prop_assert!(vec_close_with_slack(lhs, rhs, 1e-3, slack), "{lhs:?} vs {rhs:?}");

        let back = transform_from_engine(engine, system);
        prop_assert_eq!(back.translation, transform.translation);
        prop_assert_eq!(back.scale, transform.scale);
        prop_assert!(back.rotation.dot(transform.rotation).abs() > 1.0 - 1e-5);
    }
}
