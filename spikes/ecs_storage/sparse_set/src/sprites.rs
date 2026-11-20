//! The `sprites` workload on the sparse-set `World`, ungrouped and with an owning group.

use engine::prelude::{Prng, Transform2d};
use spike_ecs_harness::sprites::{self as rules, DrawItem, Hidden, Sprite, Sprites, Velocity2d};
use spike_ecs_harness::{Params, Workload, dt};

use crate::{Query, QueryMut, Without, World};

/// `sprites` on a [`World`]; `GROUPED` declares the `(Transform2d, Velocity2d)` owning group.
///
/// The move pass takes the group path when grouped; the draw query excludes `Hidden`, so it
/// takes the view path.
#[derive(Debug)]
pub struct SpritesOn<const GROUPED: bool> {
    world: World,
    draw_list: Vec<DrawItem>,
    dt: f32,
}

/// `sprites` on a sparse-set `World` without groups.
pub type SparseSetSprites = SpritesOn<false>;
/// `sprites` on a sparse-set `World` with the `(Transform2d, Velocity2d)` group.
pub type GroupedSprites = SpritesOn<true>;

/// The move query, shared with the test hook so the two cannot drift.
fn move_query(world: &mut World) -> QueryMut<'_, (&mut Transform2d, &Velocity2d)> {
    world.query_mut()
}

/// The filter of the draw query: every entity without `Hidden`.
type DrawFilter = ((), Without<Hidden>);

/// The draw query, shared with the test hook so the two cannot drift.
fn draw_query(world: &World) -> Query<'_, (&Transform2d, &Sprite), DrawFilter> {
    world.query().without::<Hidden>()
}

#[cfg(test)]
impl<const GROUPED: bool> SpritesOn<GROUPED> {
    /// Whether the move query takes the group path, for tests.
    pub(crate) fn uses_group_for_move(&mut self) -> bool {
        move_query(&mut self.world).uses_group()
    }

    /// Whether the draw query takes the group path, for tests.
    pub(crate) fn uses_group_for_draw(&self) -> bool {
        draw_query(&self.world).uses_group()
    }
}

impl<const GROUPED: bool> Workload for SpritesOn<GROUPED> {
    type Spec = Sprites;
    const BACKEND: &'static str = if GROUPED {
        "sparse_set_grouped"
    } else {
        "sparse_set"
    };

    fn setup(params: Params) -> Self {
        let prng = Prng::from_seed(params.seed);
        let mut world = if GROUPED {
            World::builder()
                .group::<(Transform2d, Velocity2d)>()
                .build()
        } else {
            World::new()
        };
        for i in 0..params.count as u64 {
            let (transform, velocity, sprite, hidden) = rules::spawn(&prng, i);
            let e = world.spawn();
            world.insert::<Transform2d>(e, transform);
            world.insert::<Velocity2d>(e, velocity);
            world.insert::<Sprite>(e, sprite);
            if let Some(hidden) = hidden {
                world.insert::<Hidden>(e, hidden);
            }
        }
        Self {
            world,
            draw_list: Vec::with_capacity(params.count),
            dt: dt(),
        }
    }

    fn step(&mut self) {
        let dt = self.dt;
        move_query(&mut self.world)
            .for_each(|(transform, velocity)| rules::update(transform, velocity, dt));
        self.draw_list.clear();
        let draw_list = &mut self.draw_list;
        draw_query(&self.world)
            .for_each(|(transform, sprite)| draw_list.push(rules::draw_item(transform, sprite)));
    }

    fn checksum(&self) -> u64 {
        let mut sum = 0_u64;
        // `Hidden` is optional: `Option<&Hidden>` reports whether the tag is present.
        self.world
            .query::<(&Transform2d, &Velocity2d, &Sprite, Option<&Hidden>)>()
            .for_each(|(transform, velocity, sprite, hidden)| {
                sum = sum.wrapping_add(rules::hash_entity(
                    transform,
                    velocity,
                    sprite,
                    hidden.is_some(),
                ));
            });
        self.draw_list.iter().fold(sum, |sum, item| {
            sum.wrapping_add(rules::hash_draw_item(item))
        })
    }
}
