//! `Component` impls for the harness's types: the foreign types the nine workloads store.
//!
//! One module, because `sprites` and `agents` share `Transform2d`. The sparse types are
//! `add_remove::B` and `agents::Seeker` (spec section 4.9); every other type, `Hidden`
//! included, is a table type. The agents' own newtypes are implemented in `agents.rs`.

use engine::prelude::Transform2d;
use spike_ecs_harness::{
    add_remove, agents, entity_churn, frag_iter, many_queries, random_access, simple_iter, sprites,
};

use crate::component::components;

components!(SparseStorage: add_remove::B, agents::Seeker);

components!(
    TableStorage:
    add_remove::A,
    simple_iter::Matrix,
    simple_iter::Position,
    simple_iter::Rotation,
    simple_iter::Velocity,
    entity_churn::SpawnId,
    entity_churn::Position,
    entity_churn::Velocity,
    entity_churn::Lifetime,
    sprites::Velocity2d,
    sprites::Sprite,
    sprites::Hidden,
    frag_iter::Data,
    Transform2d,
    frag_iter::A,
    frag_iter::B,
    frag_iter::C,
    frag_iter::D,
    frag_iter::E,
    frag_iter::F,
    frag_iter::G,
    frag_iter::H,
    frag_iter::I,
    frag_iter::J,
    frag_iter::K,
    frag_iter::L,
    frag_iter::M,
    frag_iter::N,
    frag_iter::O,
    frag_iter::P,
    frag_iter::Q,
    frag_iter::R,
    frag_iter::S,
    frag_iter::T,
    frag_iter::U,
    frag_iter::V,
    frag_iter::W,
    frag_iter::X,
    frag_iter::Y,
    frag_iter::Z,
    random_access::SpawnId,
    random_access::Slot,
    random_access::Position,
    random_access::Velocity,
    random_access::Lifetime,
    random_access::Target<crate::Entity>,
    many_queries::C0,
    many_queries::C1,
    many_queries::C2,
    many_queries::C3,
    many_queries::C4,
    many_queries::C5,
    many_queries::C6,
    many_queries::C7,
);
