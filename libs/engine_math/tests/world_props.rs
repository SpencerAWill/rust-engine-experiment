//! Property tests for `WorldTransform`: the large-world precision guarantee.

use engine_math::{DVec3, Quat, Transform, Vec3, WorldTransform};
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

proptest! {
    #![proptest_config(config())]

    /// Spec §5.4: for absolute positions up to 1e9 m and a local offset within 10 km of the
    /// chosen origin, `relative_to` then `to_world` round-trips within 1 mm.
    #[test]
    fn relative_roundtrip_within_1mm(
        abs in prop::array::uniform3(-1e9_f64..1e9),
        off in prop::array::uniform3(-1e4_f64..1e4),
    ) {
        let origin = DVec3::from(abs);
        let pos = origin + DVec3::from(off);
        let world = WorldTransform::from_translation(pos);
        let back = world.relative_to(origin).to_world(origin);
        prop_assert!((back.translation - pos).length() <= 1e-3);
    }

    #[test]
    fn rotation_and_scale_survive_the_roundtrip_unchanged(
        abs in prop::array::uniform3(-1e9_f64..1e9),
        rotation in unit_quat(),
        scale in 0.1_f32..10.0,
    ) {
        let world = WorldTransform {
            translation: DVec3::from(abs),
            rotation,
            scale: Vec3::splat(scale),
        };
        let back = world.relative_to(world.translation).to_world(world.translation);
        prop_assert_eq!(back.rotation, world.rotation);
        prop_assert_eq!(back.scale, world.scale);
    }

    #[test]
    fn transform_point_matches_daffine(
        abs in prop::array::uniform3(-1e9_f64..1e9),
        rotation in unit_quat(),
        scale in 0.1_f32..10.0,
        local in prop::array::uniform3(-1e3_f32..1e3),
    ) {
        let world = WorldTransform {
            translation: DVec3::from(abs),
            rotation,
            scale: Vec3::splat(scale),
        };
        let p = Vec3::from(local);
        let direct = world.transform_point(p);
        let affine = world.to_daffine().transform_point3(p.as_dvec3());
        // The f32 rotation is only unit to ~6e-8, so allow an error proportional to the size of
        // the local contribution (translation is added exactly in f64 on both sides).
        let tolerance = 1e-5 * (1.0 + f64::from(scale) * f64::from(p.length()));
        prop_assert!((direct - affine).length() <= tolerance, "{direct:?} vs {affine:?}");
    }

    #[test]
    fn mul_transform_places_a_local_child(
        abs in prop::array::uniform3(-1e9_f64..1e9),
        rotation in unit_quat(),
        child in prop::array::uniform3(-1e3_f32..1e3),
    ) {
        let parent = WorldTransform {
            translation: DVec3::from(abs),
            rotation,
            scale: Vec3::ONE,
        };
        let child = Transform::from_translation(Vec3::from(child));
        let placed = parent.mul_transform(child);
        let expected = parent.transform_point(child.translation);
        prop_assert!((placed.translation - expected).length() <= 1e-9 * (1.0 + expected.length()));
    }
}
