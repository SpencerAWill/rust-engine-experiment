//! A buffer of structural changes, recorded while a query runs and applied afterwards.

use core::fmt;

use crate::{Entity, World};

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
/// ```
/// use spike_ecs_sparse_set::{Commands, World};
///
/// struct Health(u32);
///
/// let mut world = World::new();
/// let e = world.spawn();
/// let mut commands = Commands::new();
/// commands.insert(e, Health(3));
/// world.apply(&mut commands);
/// assert!(world.get::<Health>(e).is_some());
/// ```
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
    pub fn insert<T: 'static>(&mut self, entity: Entity, value: T) {
        self.queue
            .push(Command::Deferred(Box::new(move |world: &mut World| {
                if world.is_alive(entity) {
                    world.insert(entity, value);
                }
            })));
    }

    /// Records a removal of the `T` component of `entity`. Skipped if the entity is dead when
    /// the command runs.
    pub fn remove<T: 'static>(&mut self, entity: Entity) {
        self.queue
            .push(Command::Deferred(Box::new(move |world: &mut World| {
                if world.is_alive(entity) {
                    world.remove::<T>(entity);
                }
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
    use crate::{Commands, World};

    #[derive(Debug, PartialEq)]
    struct A(u32);
    #[derive(Debug, PartialEq)]
    struct B(u32);

    #[test]
    fn commands_run_in_recorded_order() {
        let mut w = World::new();
        let e = w.spawn();
        let mut c = Commands::new();
        c.insert(e, A(1));
        c.remove::<A>(e);
        assert_eq!(c.len(), 2);
        w.apply(&mut c);
        assert_eq!(w.get::<A>(e), None);
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
        c.remove::<A>(dead);
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
    fn commands_maintain_groups() {
        let mut w = World::builder().group::<(A, B)>().build();
        let e = w.spawn();
        w.insert(e, A(1));
        let mut c = Commands::new();
        c.insert(e, B(2));
        w.apply(&mut c);
        assert_eq!(w.query::<(&A, &B)>().iter().count(), 1);
        assert!(w.query::<(&A, &B)>().uses_group());
        assert_eq!(w.check_invariants(), Ok(()));
        c.remove::<B>(e);
        w.apply(&mut c);
        assert_eq!(w.query::<(&A, &B)>().iter().count(), 0);
        assert_eq!(w.check_invariants(), Ok(()));
    }
}
