//! `heavy_compute`: a step dominated by arithmetic, so storage overhead barely shows
//! (spec §4.4.3).

use engine::prelude::{Mat4, Prng, Vec4};

use crate::simple_iter::draw_vectors;
pub use crate::simple_iter::{Matrix, Position, Rotation, Velocity};
use crate::{Fnv1a, Spec};

/// The `heavy_compute` workload's identity.
#[derive(Clone, Copy, Debug)]
pub struct HeavyCompute;

impl Spec for HeavyCompute {
    const NAME: &'static str = "heavy_compute";
    const SIZES: &'static [usize] = &[1_000];
}

/// How many times the step inverts each matrix.
pub const INVERSIONS: u32 = 100;

/// Entity `i`'s components: position, rotation and velocity as in `simple_iter`, then the matrix.
///
/// The matrix draws are, for column `c` in 0 to 2 and row `r` in 0 to 2, one value in `2..4` if
/// `r == c` and in `-0.5..0.5` otherwise, then the translation as 3 values in `-10..10`. Row 3 is
/// `(0, 0, 0, 1)`. The 3 by 3 part is strictly diagonally dominant, so the matrix is invertible.
#[must_use]
pub fn spawn(prng: &Prng, i: u64) -> (Matrix, Position, Rotation, Velocity) {
    let mut r = prng.stream(i);
    let (position, rotation, velocity) = draw_vectors(&mut r);
    let mut cols = [Vec4::ZERO, Vec4::ZERO, Vec4::ZERO, Vec4::W];
    for (c, col) in cols.iter_mut().take(3).enumerate() {
        for row in 0..3 {
            col[row] = if row == c {
                r.range(2.0_f32..4.0)
            } else {
                r.range(-0.5_f32..0.5)
            };
        }
    }
    for row in 0..3 {
        cols[3][row] = r.range(-10.0_f32..10.0);
    }
    let matrix = Mat4::from_cols(cols[0], cols[1], cols[2], cols[3]);
    (Matrix(matrix), position, rotation, velocity)
}

/// The rule for one entity, in this order: invert the matrix [`INVERSIONS`] times (written back
/// each time); then move the matrix's translation (column 3) by `velocity * dt()`, per row a
/// multiply and then an add, never fused; then `position = matrix.transform_point3(rotation)`.
///
/// The inversions alone would leave the matrix where it started (an even count), so the
/// translation step is what makes the state change every tick. Every backend calls this
/// function, so the arithmetic is written once.
#[inline]
pub fn update(
    matrix: &mut Matrix,
    position: &mut Position,
    rotation: &Rotation,
    velocity: &Velocity,
) {
    for _ in 0..INVERSIONS {
        matrix.0 = matrix.0.inverse();
    }
    let dt = crate::dt();
    for row in 0..3 {
        let moved = velocity.0[row] * dt;
        matrix.0.w_axis[row] += moved;
    }
    position.0 = matrix.0.transform_point3(rotation.0);
}

/// One entity's term of the checksum: `vec3(position)`, then the 16 matrix elements as `f32` in
/// column-major order.
#[must_use]
pub fn hash(position: &Position, matrix: &Matrix) -> u64 {
    matrix
        .0
        .to_cols_array()
        .into_iter()
        .fold(Fnv1a::new().vec3(position.0), Fnv1a::f32)
        .finish()
}
