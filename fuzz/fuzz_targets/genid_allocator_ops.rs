//! Fuzzes `GenIdAllocator` against a `BTreeSet` reference model, checking the allocator's
//! internal invariants after every operation.

#![no_main]
#![forbid(unsafe_code)]

use std::collections::BTreeSet;

use arbitrary::Arbitrary;
use engine_genid::{GenId, GenIdAllocator};
use libfuzzer_sys::fuzz_target;

#[derive(Arbitrary, Debug)]
enum Op {
    Allocate,
    TryAllocate,
    /// Allocates `n % 65` IDs at once.
    AllocateMany(u8),
    /// Tries to allocate `n % 65` IDs at once.
    TryAllocateMany(u8),
    Free(usize),
    IsAlive(usize),
    Clear,
    /// An ID built from arbitrary bits, which the allocator probably never issued.
    Probe(u64),
    /// An issued ID's slot with a nearby generation (a stale or never-issued neighbour).
    ProbeNear(usize, i8),
}

#[derive(Arbitrary, Debug)]
struct Input {
    /// `None` is an unbounded allocator; `Some(c)` is a fixed capacity of `c % 17`.
    capacity: Option<u8>,
    ops: Vec<Op>,
}

/// The allocator under test, its model, and the bookkeeping the checks need.
struct Harness {
    allocator: GenIdAllocator<()>,
    /// `to_bits` of the live IDs. Derived only from the ops applied so far.
    model: BTreeSet<u64>,
    /// The fixed capacity, if any.
    limit: Option<usize>,
    /// Every ID the allocator has issued, for modulo selection (stale ones included).
    issued: Vec<GenId<()>>,
    /// `to_bits` of every ID issued so far. A freed slot already holds the generation of its
    /// next occupant, and `clear` bumps only occupied slots, so no pair is ever issued twice,
    /// not even across a `clear`.
    ever_issued: BTreeSet<u64>,
}

impl Harness {
    fn new(capacity: Option<u8>) -> Self {
        let limit = capacity.map(|c| usize::from(c % 17));
        Self {
            allocator: limit.map_or_else(GenIdAllocator::new, GenIdAllocator::with_fixed_capacity),
            model: BTreeSet::new(),
            limit,
            issued: Vec::new(),
            ever_issued: BTreeSet::new(),
        }
    }

    /// How many more IDs the model says can be allocated.
    fn room(&self) -> usize {
        self.limit
            .map_or(usize::MAX, |limit| limit.saturating_sub(self.model.len()))
    }

    /// An issued ID chosen by `i`, or `None` before the first allocation.
    fn pick(&self, i: usize) -> Option<GenId<()>> {
        (!self.issued.is_empty()).then(|| self.issued[i % self.issued.len()])
    }

    /// Records an ID the allocator just issued.
    fn register(&mut self, id: GenId<()>) {
        let bits = id.to_bits();
        assert!(self.ever_issued.insert(bits), "{id:?} was issued twice");
        assert!(self.model.insert(bits), "{id:?} was already live");
        self.issued.push(id);
    }

    /// Records a batch, which must be exactly what `n` single `allocate` calls would issue.
    fn register_batch(&mut self, batch: Vec<GenId<()>>, mut replay: GenIdAllocator<()>) {
        let expected: Vec<GenId<()>> = (0..batch.len()).map(|_| replay.allocate()).collect();
        assert_eq!(batch, expected, "a batch differs from single allocations");
        for id in batch {
            self.register(id);
        }
    }

    fn apply(&mut self, op: &Op) {
        match *op {
            Op::Allocate => {
                // `allocate` panics at the limit, as documented; `TryAllocate` covers that path.
                if self.room() > 0 {
                    let id = self.allocator.allocate();
                    self.register(id);
                }
            }
            Op::TryAllocate => {
                let had_room = self.room() > 0;
                match self.allocator.try_allocate() {
                    Some(id) => {
                        assert!(had_room, "allocated from a full allocator");
                        self.register(id);
                    }
                    None => assert!(!had_room, "refused to allocate below the limit"),
                }
            }
            Op::AllocateMany(n) => {
                let n = usize::from(n % 65);
                // Like `allocate`, `allocate_many` panics when it cannot fit `n` IDs.
                if n <= self.room() {
                    let replay = self.allocator.clone();
                    let batch = self.allocator.allocate_many(n);
                    assert_eq!(batch.len(), n);
                    self.register_batch(batch, replay);
                }
            }
            Op::TryAllocateMany(n) => {
                let n = usize::from(n % 65);
                let fits = n <= self.room();
                let replay = self.allocator.clone();
                match self.allocator.try_allocate_many(n) {
                    Some(batch) => {
                        assert!(fits, "allocated {n} IDs with room for {}", self.room());
                        assert_eq!(batch.len(), n);
                        self.register_batch(batch, replay);
                    }
                    // Nothing may have been allocated. `check` compares `len()` with the model;
                    // the clone taken before the call catches state the model cannot see, such
                    // as a changed free-list order or generation.
                    None => {
                        assert!(!fits, "refused {n} IDs with room for {}", self.room());
                        let mut replay = replay;
                        assert_eq!(
                            self.allocator.clone().try_allocate(),
                            replay.try_allocate(),
                            "a refused batch changed what the next allocation returns"
                        );
                    }
                }
            }
            Op::Free(i) => {
                if let Some(id) = self.pick(i) {
                    let expected = self.model.remove(&id.to_bits());
                    assert_eq!(self.allocator.free(id), expected);
                    assert!(!self.allocator.is_alive(id));
                }
            }
            Op::IsAlive(i) => {
                if let Some(id) = self.pick(i) {
                    assert_eq!(
                        self.allocator.is_alive(id),
                        self.model.contains(&id.to_bits())
                    );
                }
            }
            Op::Clear => {
                self.allocator.clear();
                self.model.clear();
            }
            Op::Probe(bits) => {
                if let Some(id) = GenId::<()>::from_bits(bits) {
                    self.probe(id);
                }
            }
            Op::ProbeNear(i, delta) => {
                if let Some(issued) = self.pick(i) {
                    let generation = issued
                        .generation()
                        .get()
                        .wrapping_add_signed(i32::from(delta));
                    let bits = (u64::from(issued.index()) << 32) | u64::from(generation);
                    if let Some(id) = GenId::<()>::from_bits(bits) {
                        self.probe(id);
                    }
                }
            }
        }
    }

    /// An ID the model does not hold must be reported not alive and must not be freeable,
    /// without panicking and without changing the allocator (`check` verifies the latter).
    fn probe(&mut self, id: GenId<()>) {
        if self.model.contains(&id.to_bits()) {
            return;
        }
        assert!(!self.allocator.is_alive(id), "{id:?} is not live");
        assert!(
            !self.allocator.free(id),
            "freed {id:?}, which is not live"
        );
    }

    /// Everything that must hold after every operation.
    fn check(&self) {
        self.allocator
            .check_invariants()
            .expect("the allocator's invariants must hold");
        assert_eq!(self.allocator.len(), self.model.len());
        assert_eq!(self.allocator.is_empty(), self.model.is_empty());
    }
}

fuzz_target!(|input: Input| {
    let mut harness = Harness::new(input.capacity);
    for op in &input.ops {
        harness.apply(op);
        harness.check();
    }
});
