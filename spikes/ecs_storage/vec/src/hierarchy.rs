//! The `hierarchy` workload on a `Vec` of rows (M3.24 spec §4.4).

use engine::prelude::Prng;
use engine::prelude::WorldTransform2d;
use spike_ecs_harness::hierarchy::{
    self as rules, GlobalTransform, Hierarchy, LocalTransform, SpawnId, forest,
};
use spike_ecs_harness::{Params, Workload};

/// One entity: its spawn id, level, parent's row index, and both transforms.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Row {
    pub(crate) id: SpawnId,
    pub(crate) level: u32,
    /// The parent's index in the `Vec`, which is its spawn id; `None` for a root.
    pub(crate) parent: Option<u32>,
    pub(crate) local: LocalTransform,
    pub(crate) global: GlobalTransform,
}

/// `hierarchy` as one `Vec` of rows in spawn order.
///
/// The step spins every row, then makes one pass in spawn order that sets each global with
/// `root` or with `place` from the parent's row: pre-order puts every parent before its children,
/// so the parent's global is already this tick's. That equals the level passes' result, since
/// `place` reads only the parent's global and the entity's local. It is the reference the
/// storage backends are compared against, not a storage model: it runs one pass and no lookup.
#[derive(Clone, Debug)]
pub struct VecHierarchy {
    pub(crate) rows: Vec<Row>,
}

impl VecHierarchy {
    /// Sets every row's global from its local, parents first.
    fn propagate(&mut self) {
        for i in 0..self.rows.len() {
            let global = match self.rows[i].parent {
                None => rules::root(&self.rows[i].local),
                Some(p) => rules::place(&self.rows[p as usize].global, &self.rows[i].local),
            };
            self.rows[i].global = global;
        }
    }

    /// Every row's `GlobalTransform`, in spawn order.
    #[cfg(test)]
    pub(crate) fn globals(&self) -> Vec<GlobalTransform> {
        self.rows.iter().map(|r| r.global).collect()
    }
}

impl Workload for VecHierarchy {
    type Spec = Hierarchy;
    const BACKEND: &'static str = "vec_aos";

    fn setup(params: Params) -> Self {
        let prng = Prng::from_seed(params.seed);
        let rows = forest(&prng, params.count)
            .map(|node| Row {
                id: SpawnId(node.id),
                level: node.level,
                parent: node
                    .parent
                    .map(|p| u32::try_from(p).expect("count fits in u32")),
                local: node.local,
                global: GlobalTransform(WorldTransform2d::IDENTITY),
            })
            .collect();
        let mut w = Self { rows };
        w.propagate();
        w
    }

    fn step(&mut self) {
        for row in &mut self.rows {
            rules::spin(&mut row.local);
        }
        self.propagate();
    }

    fn checksum(&self) -> u64 {
        self.rows.iter().fold(0, |sum, r| {
            sum.wrapping_add(rules::hash(
                &r.id,
                r.level,
                r.parent.map(u64::from),
                &r.local,
                &r.global,
            ))
        })
    }
}
