//! The `sprites` workload on a `hecs` `World`.

use engine::prelude::{Prng, Transform2d};
use hecs::World;
use spike_ecs_harness::sprites::{self as rules, DrawItem, Hidden, Sprite, Sprites, Velocity2d};
use spike_ecs_harness::{Params, Workload, dt};

/// `sprites` on a `hecs` [`World`]: sprites spawn into one of two archetypes (with or without
/// `Hidden`). The move query matches both, the draw query only the visible one.
pub struct HecsSprites {
    world: World,
    draw_list: Vec<DrawItem>,
    dt: f32,
}

impl core::fmt::Debug for HecsSprites {
    /// `hecs::World` has no `Debug`, so this shows the entity count only.
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("HecsSprites")
            .field("entities", &self.world.len())
            .finish_non_exhaustive()
    }
}

impl Workload for HecsSprites {
    type Spec = Sprites;
    const BACKEND: &'static str = "hecs";

    fn setup(params: Params) -> Self {
        let prng = Prng::from_seed(params.seed);
        let mut world = World::new();
        for i in 0..params.count as u64 {
            let (transform, velocity, sprite, hidden) = rules::spawn(&prng, i);
            match hidden {
                Some(hidden) => {
                    world.spawn((transform, velocity, sprite, hidden));
                }
                None => {
                    world.spawn((transform, velocity, sprite));
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
        for (transform, velocity) in self.world.query_mut::<(&mut Transform2d, &Velocity2d)>() {
            rules::update(transform, velocity, dt);
        }
        self.draw_list.clear();
        for (transform, sprite) in self
            .world
            .query_mut::<(&Transform2d, &Sprite)>()
            .without::<&Hidden>()
        {
            self.draw_list.push(rules::draw_item(transform, sprite));
        }
    }

    fn checksum(&self) -> u64 {
        let sum = self
            .world
            .query::<(&Transform2d, &Velocity2d, &Sprite, Option<&Hidden>)>()
            .iter()
            .fold(0_u64, |sum, (transform, velocity, sprite, hidden)| {
                sum.wrapping_add(rules::hash_entity(
                    transform,
                    velocity,
                    sprite,
                    hidden.is_some(),
                ))
            });
        self.draw_list.iter().fold(sum, |sum, item| {
            sum.wrapping_add(rules::hash_draw_item(item))
        })
    }
}
