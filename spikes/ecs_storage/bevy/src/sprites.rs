//! The `sprites` workload on a `bevy_ecs` `World`.

use bevy_ecs::query::{QueryState, Without};
use bevy_ecs::world::World;
use engine::prelude::{Prng, Transform2d};
use spike_ecs_harness::sprites::{self as rules, DrawItem, Hidden, Sprite, Sprites, Velocity2d};
use spike_ecs_harness::{Params, Workload, dt};

use crate::Table;

/// The move query.
type MoveQuery = QueryState<(&'static mut Table<Transform2d>, &'static Table<Velocity2d>)>;

/// The draw query: every sprite without `Hidden`.
type DrawQuery =
    QueryState<(&'static Table<Transform2d>, &'static Table<Sprite>), Without<Table<Hidden>>>;

/// `sprites` on a `bevy_ecs` [`World`]: sprites spawn into one of two archetypes (with or
/// without `Hidden`). The move query matches both, the draw query only the visible one.
#[derive(Debug)]
pub struct BevySprites {
    world: World,
    moves: MoveQuery,
    draw: DrawQuery,
    draw_list: Vec<DrawItem>,
    dt: f32,
}

impl Workload for BevySprites {
    type Spec = Sprites;
    const BACKEND: &'static str = "bevy_ecs";

    fn setup(params: Params) -> Self {
        let prng = Prng::from_seed(params.seed);
        let mut world = World::new();
        for i in 0..params.count as u64 {
            let (transform, velocity, sprite, hidden) = rules::spawn(&prng, i);
            match hidden {
                Some(hidden) => {
                    world.spawn((
                        Table(transform),
                        Table(velocity),
                        Table(sprite),
                        Table(hidden),
                    ));
                }
                None => {
                    world.spawn((Table(transform), Table(velocity), Table(sprite)));
                }
            }
        }
        let moves = world.query();
        let draw = world.query_filtered();
        Self {
            world,
            moves,
            draw,
            draw_list: Vec::with_capacity(params.count),
            dt: dt(),
        }
    }

    fn step(&mut self) {
        let dt = self.dt;
        for (mut transform, velocity) in self.moves.iter_mut(&mut self.world) {
            rules::update(&mut transform.0, &velocity.0, dt);
        }
        self.draw_list.clear();
        for (transform, sprite) in self.draw.iter(&self.world) {
            self.draw_list
                .push(rules::draw_item(&transform.0, &sprite.0));
        }
        self.world.clear_trackers();
    }

    fn checksum(&self) -> u64 {
        let mut sum = 0_u64;
        if let Some(query) = self.world.try_query::<(
            &Table<Transform2d>,
            &Table<Velocity2d>,
            &Table<Sprite>,
            Option<&Table<Hidden>>,
        )>() {
            for (transform, velocity, sprite, hidden) in query.iter_manual(&self.world) {
                sum = sum.wrapping_add(rules::hash_entity(
                    &transform.0,
                    &velocity.0,
                    &sprite.0,
                    hidden.is_some(),
                ));
            }
        }
        self.draw_list.iter().fold(sum, |sum, item| {
            sum.wrapping_add(rules::hash_draw_item(item))
        })
    }
}
