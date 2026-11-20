//! The `sprites` workload on the dense `World`.

use engine::prelude::{Prng, Transform2d};
use spike_ecs_harness::sprites::{self as rules, DrawItem, Hidden, Sprite, Sprites, Velocity2d};
use spike_ecs_harness::{Params, Workload, dt};

use crate::{Query, QueryMut, Without, World};

/// `sprites` on a dense-array [`World`].
///
/// The move pass joins `Transform2d` and `Velocity2d`; the draw pass joins `Transform2d` and
/// `Sprite` and excludes `Hidden`. The draw list is owned and reused across ticks.
#[derive(Debug)]
pub struct DenseSprites {
    world: World,
    draw_list: Vec<DrawItem>,
    dt: f32,
}

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
impl DenseSprites {
    /// The world, for tests.
    pub(crate) fn world(&self) -> &World {
        &self.world
    }

    /// The sets the move query joins (required, excluded), for tests.
    pub(crate) fn move_joined(&mut self) -> crate::query::JoinedSets {
        move_query(&mut self.world).joined()
    }

    /// The sets the draw query joins (required, excluded), for tests.
    pub(crate) fn draw_joined(&self) -> crate::query::JoinedSets {
        draw_query(&self.world).joined()
    }
}

impl Workload for DenseSprites {
    type Spec = Sprites;
    const BACKEND: &'static str = "dense";

    fn setup(params: Params) -> Self {
        let prng = Prng::from_seed(params.seed);
        let mut world = World::new();
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
