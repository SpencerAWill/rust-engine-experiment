//! The `sprites` workload on a `Vec` of structs.

use engine::prelude::{Prng, Transform2d};
use spike_ecs_harness::sprites::{self as rules, DrawItem, Hidden, Sprite, Sprites, Velocity2d};
use spike_ecs_harness::{Params, Workload, dt};

/// One entity: all components in one struct, the `Hidden` tag as an `Option`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct SpriteEntity {
    pub(crate) transform: Transform2d,
    pub(crate) velocity: Velocity2d,
    pub(crate) sprite: Sprite,
    pub(crate) hidden: Option<Hidden>,
}

/// `sprites` as an array of structs, with the draw list as a separate `Vec`.
#[derive(Clone, Debug)]
pub struct VecSprites {
    pub(crate) entities: Vec<SpriteEntity>,
    pub(crate) draw_list: Vec<DrawItem>,
    dt: f32,
}

impl Workload for VecSprites {
    type Spec = Sprites;
    const BACKEND: &'static str = "vec_aos";

    fn setup(params: Params) -> Self {
        let prng = Prng::from_seed(params.seed);
        let mut entities = Vec::with_capacity(params.count);
        for i in 0..params.count as u64 {
            let (transform, velocity, sprite, hidden) = rules::spawn(&prng, i);
            entities.push(SpriteEntity {
                transform,
                velocity,
                sprite,
                hidden,
            });
        }
        Self {
            entities,
            draw_list: Vec::with_capacity(params.count),
            dt: dt(),
        }
    }

    fn step(&mut self) {
        for e in &mut self.entities {
            rules::update(&mut e.transform, &e.velocity, self.dt);
        }
        self.draw_list.clear();
        for e in &self.entities {
            if e.hidden.is_none() {
                self.draw_list
                    .push(rules::draw_item(&e.transform, &e.sprite));
            }
        }
    }

    fn checksum(&self) -> u64 {
        let entities = self.entities.iter().fold(0_u64, |sum, e| {
            sum.wrapping_add(rules::hash_entity(
                &e.transform,
                &e.velocity,
                &e.sprite,
                e.hidden.is_some(),
            ))
        });
        self.draw_list.iter().fold(entities, |sum, item| {
            sum.wrapping_add(rules::hash_draw_item(item))
        })
    }
}
