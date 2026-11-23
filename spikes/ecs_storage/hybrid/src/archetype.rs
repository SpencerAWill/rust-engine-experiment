//! Archetypes: one table per distinct set of table component types, with cached transition
//! edges. Adapted from the archetype prototype's (M3.4); sparse types never appear here (A5).

use crate::Entity;
use crate::table::{ColumnMut, Table};

/// The dense index of a component type: the position of its `ComponentInfo` in the world.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ComponentId(u32);

impl ComponentId {
    pub(crate) const fn new(index: u32) -> Self {
        Self(index)
    }

    pub(crate) fn index(self) -> usize {
        self.0 as usize
    }
}

/// The index of an archetype in the world.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ArchetypeId(u32);

impl ArchetypeId {
    /// The archetype with no components, created by `World::new`.
    pub(crate) const EMPTY: Self = Self(0);

    pub(crate) fn new(index: usize) -> Self {
        Self(u32::try_from(index).expect("fewer than u32::MAX archetypes"))
    }

    pub(crate) fn index(self) -> usize {
        self.0 as usize
    }

    /// The `archetype` of `Location::NONE`.
    pub(crate) const NONE: Self = Self(u32::MAX);
}

/// A set of table component types, its table, and the cached archetypes reached by adding or
/// removing one type.
#[derive(Debug)]
pub struct Archetype {
    /// Strictly ascending.
    pub(crate) components: Box<[ComponentId]>,
    pub(crate) table: Table,
    /// `add[c]`: the archetype reached by adding component `c`, once known.
    add: Vec<Option<ArchetypeId>>,
    /// `remove[c]`: the archetype reached by removing component `c`, once known.
    remove: Vec<Option<ArchetypeId>>,
}

impl Archetype {
    pub(crate) fn new(components: Box<[ComponentId]>, table: Table) -> Self {
        Self {
            components,
            table,
            add: Vec::new(),
            remove: Vec::new(),
        }
    }

    /// The index of `component`'s column, by binary search.
    pub(crate) fn column_of(&self, component: ComponentId) -> Option<usize> {
        column_in(&self.components, component)
    }

    /// A view of this archetype for a mutable query run: its component list, row count,
    /// entities and column handles, and nothing that changes a length (lesson ecs-5).
    pub(crate) fn view_mut(&mut self) -> ArchetypeMut<'_> {
        let Self {
            components, table, ..
        } = self;
        ArchetypeMut {
            components: &components[..],
            table,
        }
    }

    pub(crate) fn add_edge(&self, component: ComponentId) -> Option<ArchetypeId> {
        self.add.get(component.index()).copied().flatten()
    }

    pub(crate) fn remove_edge(&self, component: ComponentId) -> Option<ArchetypeId> {
        self.remove.get(component.index()).copied().flatten()
    }

    pub(crate) fn set_add_edge(&mut self, component: ComponentId, to: ArchetypeId) {
        set_edge(&mut self.add, component, to);
    }

    pub(crate) fn set_remove_edge(&mut self, component: ComponentId, to: ArchetypeId) {
        set_edge(&mut self.remove, component, to);
    }

    /// Every recorded add edge as `(component, target)`.
    pub(crate) fn add_edges(&self) -> impl Iterator<Item = (ComponentId, ArchetypeId)> + '_ {
        edges(&self.add)
    }

    /// Every recorded remove edge as `(component, target)`.
    pub(crate) fn remove_edges(&self) -> impl Iterator<Item = (ComponentId, ArchetypeId)> + '_ {
        edges(&self.remove)
    }
}

/// The index of `component`'s column in an archetype whose ascending component list is
/// `components`, by binary search.
pub(crate) fn column_in(components: &[ComponentId], component: ComponentId) -> Option<usize> {
    components.binary_search(&component).ok()
}

/// One archetype, borrowed exclusively for a mutable query run. It offers the component list,
/// the row count, the entities and the column handles, so a query cannot add or remove a row,
/// replace the component list or reach an edge (lesson ecs-5).
#[derive(Debug)]
pub struct ArchetypeMut<'w> {
    components: &'w [ComponentId],
    table: &'w mut Table,
}

impl<'w> ArchetypeMut<'w> {
    /// The archetype's component ids, strictly ascending.
    pub(crate) fn components(&self) -> &'w [ComponentId] {
        self.components
    }

    /// The row count.
    pub(crate) fn len(&self) -> usize {
        self.table.len()
    }

    /// The entities, and one handle per column in component order.
    pub(crate) fn into_parts(self) -> (&'w [Entity], impl Iterator<Item = ColumnMut<'w>>) {
        self.table.columns_mut()
    }
}

fn set_edge(edges: &mut Vec<Option<ArchetypeId>>, component: ComponentId, to: ArchetypeId) {
    if edges.len() <= component.index() {
        edges.resize(component.index() + 1, None);
    }
    edges[component.index()] = Some(to);
}

fn edges(edges: &[Option<ArchetypeId>]) -> impl Iterator<Item = (ComponentId, ArchetypeId)> + '_ {
    edges.iter().enumerate().filter_map(|(c, to)| {
        let c = ComponentId::new(u32::try_from(c).expect("fewer than u32::MAX components"));
        to.map(|to| (c, to))
    })
}
