//! A buffer of structural changes, recorded while a query runs and applied afterwards.

use core::fmt;

use crate::{Component, Entity, World};

/// One recorded change.
enum Command {
    /// Despawn the entity. Not boxed.
    Despawn(Entity),
    /// Any other change: a closure that checks the entity is alive before it acts.
    Deferred(Box<dyn FnOnce(&mut World)>),
}

/// Structural changes (despawn, insert, remove) recorded now and run later by
/// [`World::apply`], in the order they were recorded.
///
/// A command whose entity is dead when it runs is skipped, so an insert recorded through
/// `Commands` never panics the way [`World::insert`] does. A rule can therefore despawn and
/// change the same entity in one query.
///
/// A recorded command reaches the storage its type uses (a table or a sparse set) when it is
/// applied; the buffer knows nothing of storage kinds.
#[derive(Default)]
pub struct Commands {
    queue: Vec<Command>,
}

impl Commands {
    /// An empty buffer.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Records a despawn of `entity`.
    pub fn despawn(&mut self, entity: Entity) {
        self.queue.push(Command::Despawn(entity));
    }

    /// Records an insert of `value` as the `T` component of `entity`. Skipped if the entity is
    /// dead when the command runs.
    pub fn insert<T: Component>(&mut self, entity: Entity, value: T) {
        self.queue
            .push(Command::Deferred(Box::new(move |world: &mut World| {
                if world.is_alive(entity) {
                    world.insert(entity, value);
                }
            })));
    }

    /// Records a removal of the `T` component of `entity`. Skipped if the entity is dead when
    /// the command runs.
    pub fn remove<T: Component>(&mut self, entity: Entity) {
        self.queue
            .push(Command::Deferred(Box::new(move |world: &mut World| {
                // `World::remove` already returns `None` for a dead entity.
                world.remove::<T>(entity);
            })));
    }

    /// The number of recorded commands.
    #[must_use]
    pub fn len(&self) -> usize {
        self.queue.len()
    }

    /// Whether no command is recorded.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    #[cfg(test)]
    fn capacity(&self) -> usize {
        self.queue.capacity()
    }
}

impl fmt::Debug for Commands {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Commands")
            .field("len", &self.queue.len())
            .finish()
    }
}

impl World {
    /// Runs the recorded commands in order, then leaves `commands` empty with its capacity
    /// kept. A command whose entity is dead when it runs is skipped.
    pub fn apply(&mut self, commands: &mut Commands) {
        for command in commands.queue.drain(..) {
            match command {
                Command::Despawn(entity) => {
                    self.despawn(entity);
                }
                Command::Deferred(run) => run(self),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::component::components;
    use crate::{Commands, World};

    #[derive(Debug, PartialEq)]
    struct A(u32);
    #[derive(Debug, PartialEq)]
    struct B(u32);
    #[derive(Debug, PartialEq)]
    struct S(u32);
    components!(TableStorage: A, B);
    components!(SparseStorage: S);

    #[test]
    fn commands_run_in_recorded_order() {
        let mut w = World::new();
        let e = w.spawn();
        let mut c = Commands::new();
        c.insert(e, A(1));
        c.remove::<A>(e);
        c.insert(e, S(1));
        c.remove::<S>(e);
        assert_eq!(c.len(), 4);
        w.apply(&mut c);
        assert_eq!(w.get::<A>(e), None);
        assert_eq!(w.get::<S>(e), None);
        assert_eq!(w.check_invariants(), Ok(()));
    }

    #[test]
    fn command_on_a_dead_entity_is_skipped() {
        let mut w = World::new();
        let live = w.spawn();
        let dead = w.spawn();
        w.despawn(dead);
        let mut c = Commands::new();
        c.insert(dead, A(1));
        c.insert(dead, S(1));
        c.remove::<A>(dead);
        c.remove::<S>(dead);
        c.despawn(dead);
        c.insert(live, A(2));
        w.apply(&mut c);
        assert_eq!(w.len(), 1);
        assert_eq!(w.get::<A>(live), Some(&A(2)));
        assert_eq!(w.check_invariants(), Ok(()));
    }

    #[test]
    fn apply_empties_the_buffer_and_keeps_its_capacity() {
        let mut w = World::new();
        let e = w.spawn();
        let mut c = Commands::default();
        assert!(c.is_empty());
        for i in 0..10 {
            c.insert(e, A(i));
        }
        assert_eq!(format!("{c:?}"), "Commands { len: 10 }");
        let cap = c.capacity();
        assert!(cap >= 10);
        w.apply(&mut c);
        assert!(c.is_empty());
        assert_eq!(c.capacity(), cap);
        assert_eq!(w.get::<A>(e), Some(&A(9)));
    }

    #[test]
    fn commands_reach_each_storage() {
        let mut w = World::new();
        let e = w.spawn_with((A(1),));
        let before = w.archetype_count();
        let mut c = Commands::new();
        c.insert(e, S(7));
        w.apply(&mut c);
        assert_eq!(w.archetype_count(), before, "a sparse insert moves nothing");
        assert_eq!(w.get::<S>(e), Some(&S(7)));
        c.insert(e, B(2));
        w.apply(&mut c);
        assert_eq!(
            w.archetype_count(),
            before + 1,
            "a table insert moves the row"
        );
        assert_eq!(w.get::<B>(e), Some(&B(2)));
        assert_eq!(w.get::<S>(e), Some(&S(7)));
        c.remove::<S>(e);
        c.remove::<B>(e);
        w.apply(&mut c);
        assert_eq!(w.get::<S>(e), None);
        assert_eq!(w.get::<B>(e), None);
        assert_eq!(w.get::<A>(e), Some(&A(1)));
        assert_eq!(w.check_invariants(), Ok(()));
    }
}
