//! The `simple_iter` workload on runtime-registered components in byte columns, through the two
//! access paths of the M3.6 spec §4.4: [`BytesCopySimpleIter`] copies each value through its
//! [`Codec`], [`BytesCastSimpleIter`] casts each column to a slice of a `Pod` mirror type.

use bytemuck::{Pod, Zeroable};
use engine::prelude::{Mat4, Prng, Vec3};
use spike_ecs_harness::simple_iter::{
    self as rules, Matrix, Position, Rotation, SimpleIter, Velocity,
};
use spike_ecs_harness::{Params, Workload};

use crate::codec::{Codec, layout, read_f32s, write_f32s};
use crate::raw::RawLayout;
use crate::{ComponentId, RawQuery, RawTable, World};

/// A `Vec3` as three `f32`s, the cast path's view of a position, rotation or velocity column.
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
struct RawVec3 {
    x: f32,
    y: f32,
    z: f32,
}

/// A `Mat4` as sixteen column-major `f32`s, the cast path's view of a matrix column.
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
struct RawMat4([f32; 16]);

impl From<Vec3> for RawVec3 {
    fn from(v: Vec3) -> Self {
        Self {
            x: v.x,
            y: v.y,
            z: v.z,
        }
    }
}

impl From<RawVec3> for Vec3 {
    fn from(v: RawVec3) -> Self {
        Self::new(v.x, v.y, v.z)
    }
}

impl From<Mat4> for RawMat4 {
    fn from(m: Mat4) -> Self {
        Self(m.to_cols_array())
    }
}

const VEC3_LAYOUT: RawLayout = layout(12, 4);

/// Implements [`Codec`] for a newtype over a `Vec3`.
macro_rules! vec3_codec {
    ($($name:ident),*) => {$(
        impl Codec for $name {
            const LAYOUT: RawLayout = VEC3_LAYOUT;

            fn encode(&self, out: &mut [u8]) {
                write_f32s(&self.0.to_array(), out);
            }

            fn decode(bytes: &[u8]) -> Self {
                Self(Vec3::from_array(read_f32s(bytes)))
            }
        }
    )*};
}

vec3_codec!(Position, Rotation, Velocity);

impl Codec for Matrix {
    const LAYOUT: RawLayout = layout(64, 4);

    fn encode(&self, out: &mut [u8]) {
        write_f32s(&self.0.to_cols_array(), out);
    }

    fn decode(bytes: &[u8]) -> Self {
        Self(Mat4::from_cols_array(&read_f32s(bytes)))
    }
}

/// The world, the four component ids and the two queries both backends share.
#[derive(Debug)]
struct Base {
    world: World,
    step: RawQuery,
    checksum: RawQuery,
}

/// The step's query, shared with the test hook so the two cannot drift.
fn step_query(position: ComponentId, velocity: ComponentId) -> RawQuery {
    RawQuery::new().write(position).read(velocity)
}

/// The checksum's query.
fn checksum_query(position: ComponentId, velocity: ComponentId) -> RawQuery {
    RawQuery::new().read(position).read(velocity)
}

impl Base {
    /// Registers the four components and spawns entities `0..count`; `values` turns one
    /// entity's components into the four byte values, in `[matrix, position, rotation,
    /// velocity]` order.
    fn new(params: Params, values: impl Fn(u64, &Prng) -> [Vec<u8>; 4]) -> Self {
        let mut world = World::new();
        let matrix = world.register_raw("Matrix", Matrix::LAYOUT);
        let position = world.register_raw("Position", Position::LAYOUT);
        let rotation = world.register_raw("Rotation", Rotation::LAYOUT);
        let velocity = world.register_raw("Velocity", Velocity::LAYOUT);
        let prng = Prng::from_seed(params.seed);
        for i in 0..params.count as u64 {
            let [m, p, r, v] = values(i, &prng);
            world
                .spawn_raw(&[(matrix, &m), (position, &p), (rotation, &r), (velocity, &v)])
                .expect("the ids are registered raw and listed once, with their layouts' sizes");
        }
        Self {
            world,
            step: step_query(position, velocity),
            checksum: checksum_query(position, velocity),
        }
    }

    /// Runs `f` on each table of the step's query.
    fn step(&mut self, f: impl FnMut(RawTable<'_, '_>)) {
        self.world
            .query_raw_mut(&self.step)
            .expect("the step's ids are registered raw")
            .for_each_table(f);
    }

    /// Runs `f` on each table of the checksum's query.
    fn checksum(&self, f: impl FnMut(RawTable<'_, '_>)) {
        self.world
            .query_raw(&self.checksum)
            .expect("the checksum's ids are registered raw")
            .for_each_table(f);
    }
}

/// `simple_iter` on byte columns, copying each value in and out through its [`Codec`].
#[derive(Debug)]
pub struct BytesCopySimpleIter {
    base: Base,
}

/// `simple_iter` on byte columns, casting each column to a slice of a `Pod` mirror type.
#[derive(Debug)]
pub struct BytesCastSimpleIter {
    base: Base,
}

/// Encodes a value into a fresh buffer of its layout's size.
fn encoded<T: Codec>(value: &T) -> Vec<u8> {
    let mut bytes = vec![0; T::LAYOUT.size()];
    value.encode(&mut bytes);
    bytes
}

impl Workload for BytesCopySimpleIter {
    type Spec = SimpleIter;
    const BACKEND: &'static str = "archetype_bytes_copy";

    fn setup(params: Params) -> Self {
        let base = Base::new(params, |i, prng| {
            let (m, p, r, v) = rules::spawn(prng, i);
            [encoded(&m), encoded(&p), encoded(&r), encoded(&v)]
        });
        Self { base }
    }

    fn step(&mut self) {
        self.base.step(|mut table| {
            let position = table.column_mut(0);
            let velocity = table.column(1);
            for (p, v) in position.rows_mut().zip(velocity.rows()) {
                let mut value = Position::decode(p);
                rules::update(&mut value, &Velocity::decode(v));
                value.encode(p);
            }
        });
    }

    fn checksum(&self) -> u64 {
        let mut sum = 0_u64;
        self.base.checksum(|table| {
            let position = table.column(0);
            let velocity = table.column(1);
            for (p, v) in position.rows().zip(velocity.rows()) {
                sum = sum.wrapping_add(rules::hash(&Position::decode(p), &Velocity::decode(v)));
            }
        });
        sum
    }
}

impl Workload for BytesCastSimpleIter {
    type Spec = SimpleIter;
    const BACKEND: &'static str = "archetype_bytes_cast";

    fn setup(params: Params) -> Self {
        let base = Base::new(params, |i, prng| {
            let (m, p, r, v) = rules::spawn(prng, i);
            [
                bytemuck::bytes_of(&RawMat4::from(m.0)).to_vec(),
                bytemuck::bytes_of(&RawVec3::from(p.0)).to_vec(),
                bytemuck::bytes_of(&RawVec3::from(r.0)).to_vec(),
                bytemuck::bytes_of(&RawVec3::from(v.0)).to_vec(),
            ]
        });
        Self { base }
    }

    fn step(&mut self) {
        self.base.step(|mut table| {
            let position = table
                .column_mut(0)
                .cast_mut::<RawVec3>()
                .expect("Position's layout matches RawVec3");
            let velocity = table
                .column(1)
                .cast::<RawVec3>()
                .expect("Velocity's layout matches RawVec3");
            for (p, v) in position.iter_mut().zip(velocity) {
                let mut value = Position(Vec3::from(*p));
                rules::update(&mut value, &Velocity(Vec3::from(*v)));
                *p = RawVec3::from(value.0);
            }
        });
    }

    fn checksum(&self) -> u64 {
        let mut sum = 0_u64;
        self.base.checksum(|table| {
            let position = table
                .column(0)
                .cast::<RawVec3>()
                .expect("Position's layout matches RawVec3");
            let velocity = table
                .column(1)
                .cast::<RawVec3>()
                .expect("Velocity's layout matches RawVec3");
            for (p, v) in position.iter().zip(velocity) {
                let (p, v) = (Position(Vec3::from(*p)), Velocity(Vec3::from(*v)));
                sum = sum.wrapping_add(rules::hash(&p, &v));
            }
        });
        sum
    }
}

/// The two backends' hooks for tests: the same query function the step uses (ecs-1).
macro_rules! test_hooks {
    ($($backend:ty),*) => {$(
        #[cfg(test)]
        impl $backend {
            /// The archetypes the step's query matches, for tests.
            pub(crate) fn step_archetypes(&mut self) -> Vec<crate::archetype::ArchetypeId> {
                self.base
                    .world
                    .query_raw_mut(&self.base.step)
                    .expect("the step's ids are registered raw")
                    .matched_archetypes()
            }

            /// The number of archetypes in the world, for tests.
            pub(crate) fn archetype_count(&self) -> usize {
                self.base.world.archetype_count()
            }
        }
    )*};
}

test_hooks!(BytesCopySimpleIter, BytesCastSimpleIter);
