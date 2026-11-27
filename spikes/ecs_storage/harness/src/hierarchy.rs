//! `hierarchy`: a forest of trees one to eight levels deep; each step spins every entity's local
//! transform, then propagates world transforms from the roots down, one level at a time (M3.24
//! spec §4.2).
//!
//! The rules here are the M3.24 spec §4.2; the common rules of M3.2 §4.4 apply too, except that
//! the propagation calls `WorldTransform2d::mul_transform`, which calls the engine's `sin_cos`.
//! Conformance compares backends within one build, so no golden checksum exists.

use core::f32::consts::{PI, TAU};

use engine::prelude::{DVec2, Prng, Transform2d, Vec2, WorldTransform2d};

use crate::{Fnv1a, Spec};

/// The `hierarchy` workload's identity.
#[derive(Clone, Copy, Debug)]
pub struct Hierarchy;

impl Spec for Hierarchy {
    const NAME: &'static str = "hierarchy";
    const SIZES: &'static [usize] = &[1_000, 10_000, 100_000];
}

/// The deepest level of any tree; roots are level 1.
pub const LEVELS: u32 = 8;

/// The rotation [`spin`] adds to every local transform each step, in radians.
pub const SPIN: f32 = 0.05;

/// The spawn index the entity was created with.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SpawnId(pub u64);

/// An entity's transform relative to its parent (or to the origin for a root).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LocalTransform(pub Transform2d);

/// An entity's transform in the world, composed down the tree each step.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GlobalTransform(pub WorldTransform2d);

/// The handle of an entity's parent. Roots have none.
///
/// `E` is the backend's entity handle, so every backend stores this type.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Parent<E>(pub E);

macro_rules! depth_types {
    ($($name:ident $level:literal),* $(,)?) => {$(
        #[doc = concat!("Marks an entity at level ", $level, " of its tree; no other depth type is present.")]
        #[derive(Clone, Copy, Debug, PartialEq)]
        pub struct $name;
    )*};
}

depth_types!(
    Depth1 "1 (a root)", Depth2 "2", Depth3 "3", Depth4 "4",
    Depth5 "5", Depth6 "6", Depth7 "7", Depth8 "8",
);

/// One entity of the forest, as [`forest`] yields it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Node {
    /// The spawn index, `0..count` in order.
    pub id: u64,
    /// The level in its tree; roots are 1.
    pub level: u32,
    /// The parent's spawn id, lower than `id`; `None` for a root.
    pub parent: Option<u64>,
    /// The initial local transform.
    pub local: LocalTransform,
}

/// The iterator [`forest`] returns.
#[derive(Debug)]
pub struct Forest<'p> {
    prng: &'p Prng,
    count: u64,
    next: u64,
    /// Pending nodes: `(level, parent, depth of the tree)`.
    stack: Vec<(u32, Option<u64>, u32)>,
}

impl Iterator for Forest<'_> {
    type Item = Node;

    fn next(&mut self) -> Option<Node> {
        if self.next >= self.count {
            return None;
        }
        let id = self.next;
        self.next += 1;
        let (level, parent, mut depth) = self.stack.pop().unwrap_or((1, None, 0));
        let mut r = self.prng.stream(id);
        if level == 1 {
            depth = r.range(1_u32..=LEVELS);
        }
        let translation = if parent.is_none() {
            Vec2::new(r.range(0.0_f32..320.0), r.range(0.0_f32..180.0))
        } else {
            Vec2::new(r.range(-16.0_f32..16.0), r.range(-16.0_f32..16.0))
        };
        let rotation = r.range(-PI..PI);
        let scale = Vec2::new(r.range(0.5_f32..2.0), r.range(0.5_f32..2.0));
        if level < depth {
            for _ in 0..r.range(1_u32..=2) {
                self.stack.push((level + 1, Some(id), depth));
            }
        }
        Some(Node {
            id,
            level,
            parent,
            local: LocalTransform(Transform2d {
                translation,
                rotation,
                scale,
            }),
        })
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let left = usize::try_from(self.count - self.next).unwrap_or(usize::MAX);
        (left, Some(left))
    }
}

/// `size_hint` is exact: the nodes left to yield.
impl ExactSizeIterator for Forest<'_> {}

/// Once `count` nodes are yielded, `next` returns `None` forever.
impl core::iter::FusedIterator for Forest<'_> {}

/// The forest of `count` nodes in spawn order (pre-order: each subtree is spawned in full before
/// its next sibling), drawn from `prng.stream(i)` for node `i` in the order of M3.24 spec §4.2.
/// The last tree is cut off at `count`.
#[must_use]
pub fn forest(prng: &Prng, count: usize) -> Forest<'_> {
    Forest {
        prng,
        count: count as u64,
        next: 0,
        stack: Vec::new(),
    }
}

/// The update rule: `r = rotation + SPIN`, then `r - TAU` if `r > PI`, else `r`. Rotations stay
/// in `(-PI, PI]` with no trigonometry.
#[inline]
pub fn spin(local: &mut LocalTransform) {
    let r = local.0.rotation + SPIN;
    local.0.rotation = if r > PI { r - TAU } else { r };
}

/// The rule for a root's global transform: `local.to_world(DVec2::ZERO)`.
#[inline]
#[must_use]
pub fn root(local: &LocalTransform) -> GlobalTransform {
    GlobalTransform(local.0.to_world(DVec2::ZERO))
}

/// The rule for a child's global transform: the parent's global, as this step wrote it, composed
/// with the child's local via `mul_transform`.
#[inline]
#[must_use]
pub fn place(parent: &GlobalTransform, local: &LocalTransform) -> GlobalTransform {
    GlobalTransform(parent.0.mul_transform(local.0))
}

/// One entity's term of the checksum: `u64(id)`, `u32(level)`, `u64(parent's spawn id)` or
/// `u64(u64::MAX)` for a root, the local's `vec2(translation)`, `f32(rotation)`, `vec2(scale)`,
/// then the global's translation x and y as `u64` of their `f64` bits, `f32(rotation)` and
/// `vec2(scale)`.
#[must_use]
pub fn hash(
    id: &SpawnId,
    level: u32,
    parent: Option<u64>,
    local: &LocalTransform,
    global: &GlobalTransform,
) -> u64 {
    Fnv1a::new()
        .u64(id.0)
        .u32(level)
        .u64(parent.unwrap_or(u64::MAX))
        .vec2(local.0.translation)
        .f32(local.0.rotation)
        .vec2(local.0.scale)
        .u64(global.0.translation.x.to_bits())
        .u64(global.0.translation.y.to_bits())
        .f32(global.0.rotation)
        .vec2(global.0.scale)
        .finish()
}

#[cfg(test)]
mod tests {
    use core::f32::consts::{PI, TAU};

    use engine::prelude::{DVec2, Prng, Transform2d, Vec2, WorldTransform2d};

    use super::{
        GlobalTransform, LEVELS, LocalTransform, SPIN, SpawnId, forest, hash, place, root, spin,
    };

    #[test]
    fn forest_links_parents_first() {
        let prng = Prng::from_seed(42);
        for count in [1_000_usize, 100_000] {
            let nodes: Vec<_> = forest(&prng, count).collect();
            assert_eq!(nodes.len(), count);
            let mut seen = [0_usize; LEVELS as usize + 1];
            for (i, node) in nodes.iter().enumerate() {
                assert_eq!(node.id, i as u64);
                assert!((1..=LEVELS).contains(&node.level), "level {}", node.level);
                seen[node.level as usize] += 1;
                match node.parent {
                    None => assert_eq!(node.level, 1),
                    Some(p) => {
                        assert!(p < node.id);
                        assert_eq!(
                            nodes[usize::try_from(p).expect("fits")].level + 1,
                            node.level
                        );
                    }
                }
            }
            for (level, n) in seen.iter().enumerate().skip(1) {
                assert!(*n > 0, "level {level} is empty at count {count}");
            }
        }
    }

    #[test]
    fn forest_follows_the_draw_order() {
        let prng = Prng::from_seed(42);
        let nodes: Vec<_> = forest(&prng, 1_000).collect();

        let mut r = prng.stream(0);
        let depth = r.range(1_u32..=8);
        let tx = r.range(0.0_f32..320.0);
        let ty = r.range(0.0_f32..180.0);
        let rotation = r.range(-PI..PI);
        let sx = r.range(0.5_f32..2.0);
        let sy = r.range(0.5_f32..2.0);
        let children = if depth > 1 { r.range(1_u32..=2) } else { 0 };
        let node = nodes[0];
        assert_eq!((node.id, node.level, node.parent), (0, 1, None));
        assert_eq!(
            node.local.0,
            Transform2d {
                translation: Vec2::new(tx, ty),
                rotation,
                scale: Vec2::new(sx, sy),
            }
        );
        // Node 0 may be a leaf, so check every node of the first trees: a node below its tree's
        // depth has the drawn number of children. Every tree before the last root is complete;
        // only the last tree can be cut off at `count`.
        let last_root = nodes
            .iter()
            .rposition(|n| n.parent.is_none())
            .expect("a root exists");
        let mut depths = Vec::new();
        for node in &nodes[..last_root] {
            let mut r = prng.stream(node.id);
            let tree_depth = match node.parent {
                None => r.range(1_u32..=LEVELS),
                Some(p) => depths[usize::try_from(p).expect("fits")],
            };
            depths.push(tree_depth);
            for _ in 0..5 {
                let _ = r.range(0.0_f32..1.0);
            }
            let want = if node.level < tree_depth {
                r.range(1_u32..=2) as usize
            } else {
                0
            };
            let got = nodes.iter().filter(|n| n.parent == Some(node.id)).count();
            assert_eq!(got, want, "children of node {}", node.id);
        }
        if children > 0 {
            assert_eq!(nodes[1].parent, Some(0));
            assert_eq!(nodes[1].level, 2);
        } else {
            assert_eq!(nodes[1].parent, None);
        }

        let (i, child) = nodes
            .iter()
            .enumerate()
            .find(|(_, n)| n.parent.is_some())
            .expect("a non-root node exists");
        let mut r = prng.stream(i as u64);
        let tx = r.range(-16.0_f32..16.0);
        let ty = r.range(-16.0_f32..16.0);
        let rotation = r.range(-PI..PI);
        let sx = r.range(0.5_f32..2.0);
        let sy = r.range(0.5_f32..2.0);
        assert_eq!(
            child.local.0,
            Transform2d {
                translation: Vec2::new(tx, ty),
                rotation,
                scale: Vec2::new(sx, sy),
            }
        );
    }

    fn local_with_rotation(rotation: f32) -> LocalTransform {
        LocalTransform(Transform2d::IDENTITY.with_rotation(rotation))
    }

    #[test]
    fn spin_wraps_and_keeps_the_order() {
        let mut l = local_with_rotation(0.0);
        spin(&mut l);
        assert_eq!(l.0.rotation, 0.05_f32);
        assert_eq!(SPIN, 0.05_f32);

        // Wrapping branch: the sum exceeds PI, so TAU is subtracted in f32.
        let mut l = local_with_rotation(PI - 0.01);
        spin(&mut l);
        let sum = (PI - 0.01) + SPIN;
        assert!(sum > PI);
        assert_eq!(l.0.rotation, sum - TAU);

        // Exactly at the compare: written as `r > PI`, so a sum equal to PI is kept.
        let start = PI - SPIN;
        let sum = start + SPIN;
        let mut l = local_with_rotation(start);
        spin(&mut l);
        let expected = if sum > PI { sum - TAU } else { sum };
        assert_eq!(l.0.rotation, expected);

        // Other fields are untouched.
        let mut l = LocalTransform(Transform2d {
            translation: Vec2::new(1.0, 2.0),
            rotation: 0.5,
            scale: Vec2::new(3.0, 4.0),
        });
        spin(&mut l);
        assert_eq!(l.0.translation, Vec2::new(1.0, 2.0));
        assert_eq!(l.0.scale, Vec2::new(3.0, 4.0));
    }

    #[test]
    fn root_and_place_compose() {
        let local = LocalTransform(Transform2d {
            translation: Vec2::new(3.0, -4.0),
            rotation: 1.25,
            scale: Vec2::new(1.5, 0.75),
        });
        assert_eq!(root(&local).0, local.0.to_world(DVec2::ZERO));

        let parent = GlobalTransform(WorldTransform2d {
            translation: DVec2::new(10.0, 20.0),
            rotation: -0.5,
            scale: Vec2::new(2.0, 1.0),
        });
        assert_eq!(place(&parent, &local).0, parent.0.mul_transform(local.0));
    }

    #[test]
    fn hash_sees_every_field() {
        let id = SpawnId(7);
        let local = LocalTransform(Transform2d {
            translation: Vec2::new(1.0, 2.0),
            rotation: 0.5,
            scale: Vec2::new(1.5, 1.25),
        });
        let global = GlobalTransform(WorldTransform2d {
            translation: DVec2::new(10.0, 20.0),
            rotation: 0.25,
            scale: Vec2::new(0.5, 0.75),
        });
        let base = hash(&id, 3, Some(4), &local, &global);

        let mut variants = vec![
            hash(&SpawnId(8), 3, Some(4), &local, &global),
            hash(&id, 4, Some(4), &local, &global),
            hash(&id, 3, Some(5), &local, &global),
            hash(&id, 3, None, &local, &global),
        ];
        for edit in 0..5 {
            let mut l = local;
            match edit {
                0 => l.0.translation.x += 1.0,
                1 => l.0.translation.y += 1.0,
                2 => l.0.rotation += 1.0,
                3 => l.0.scale.x += 1.0,
                _ => l.0.scale.y += 1.0,
            }
            variants.push(hash(&id, 3, Some(4), &l, &global));
        }
        for edit in 0..5 {
            let mut g = global;
            match edit {
                0 => g.0.translation.x += 1.0,
                1 => g.0.translation.y += 1.0,
                2 => g.0.rotation += 1.0,
                3 => g.0.scale.x += 1.0,
                _ => g.0.scale.y += 1.0,
            }
            variants.push(hash(&id, 3, Some(4), &local, &g));
        }
        for (i, v) in variants.iter().enumerate() {
            assert_ne!(*v, base, "variant {i} did not change the hash");
        }
        assert_ne!(
            hash(&id, 3, None, &local, &global),
            hash(&id, 3, Some(u64::MAX - 1), &local, &global)
        );
    }
}
