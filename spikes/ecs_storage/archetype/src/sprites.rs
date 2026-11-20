//! The `sprites` workload on the archetype `World`.

use engine::prelude::{Prng, Transform2d};
use spike_ecs_harness::sprites::{self as rules, DrawItem, Hidden, Sprite, Sprites, Velocity2d};
use spike_ecs_harness::{Params, Workload, dt};

use crate::{Query, QueryMut, Without, World};

/// `sprites` on an archetype [`World`]: sprites spawn into one of two archetypes (with or
/// without `Hidden`). The move query matches both, the draw query only the visible one.
#[derive(Debug)]
pub struct ArchetypeSprites {
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
impl ArchetypeSprites {
    /// The archetypes the move query matches, for tests.
    pub(crate) fn move_archetypes(&mut self) -> Vec<crate::archetype::ArchetypeId> {
        move_query(&mut self.world).matched_archetypes()
    }

    /// The archetypes the draw query matches, for tests.
    pub(crate) fn draw_archetypes(&self) -> Vec<crate::archetype::ArchetypeId> {
        draw_query(&self.world).matched_archetypes()
    }

    /// The number of archetypes in the world, for tests.
    pub(crate) fn archetype_count(&self) -> usize {
        self.world.archetype_count()
    }

    /// The world, for archetype inspection in tests.
    pub(crate) fn world(&self) -> &World {
        &self.world
    }
}

impl Workload for ArchetypeSprites {
    type Spec = Sprites;
    const BACKEND: &'static str = "archetype";

    fn setup(params: Params) -> Self {
        let prng = Prng::from_seed(params.seed);
        let mut world = World::new();
        for i in 0..params.count as u64 {
            let (transform, velocity, sprite, hidden) = rules::spawn(&prng, i);
            match hidden {
                Some(hidden) => {
                    world.spawn_with((transform, velocity, sprite, hidden));
                }
                None => {
                    world.spawn_with((transform, velocity, sprite));
                }
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
