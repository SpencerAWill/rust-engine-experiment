//! Fuzzes `HandleAllocator` against a `BTreeSet` reference model, checking the allocator's
//! internal invariants after every operation.

#![no_main]
#![forbid(unsafe_code)]

use std::collections::BTreeSet;

use arbitrary::Arbitrary;
use engine_handle::{Handle, HandleAllocator};
use libfuzzer_sys::fuzz_target;

#[derive(Arbitrary, Debug)]
enum Op {
    Allocate,
    TryAllocate,
    /// Allocates `n % 65` handles at once.
    AllocateMany(u8),
    /// Tries to allocate `n % 65` handles at once.
    TryAllocateMany(u8),
    Free(usize),
    IsAlive(usize),
    Clear,
    /// A handle built from arbitrary bits, which the allocator probably never issued.
    Probe(u64),
    /// An issued handle's slot with a nearby generation (a stale or never-issued neighbour).
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
    allocator: HandleAllocator<()>,
    /// `to_bits` of the live handles. Derived only from the ops applied so far.
    model: BTreeSet<u64>,
    /// The fixed capacity, if any.
    limit: Option<usize>,
    /// Every handle the allocator has issued, for modulo selection (stale ones included).
    issued: Vec<Handle<()>>,
    /// `to_bits` of every handle issued so far. A freed slot already holds the generation of its
    /// next occupant, and `clear` bumps only occupied slots, so no pair is ever issued twice,
    /// not even across a `clear`.
    ever_issued: BTreeSet<u64>,
}

impl Harness {
    fn new(capacity: Option<u8>) -> Self {
        let limit = capacity.map(|c| usize::from(c % 17));
        Self {
            allocator: limit
                .map_or_else(HandleAllocator::new, HandleAllocator::with_fixed_capacity),
            model: BTreeSet::new(),
            limit,
            issued: Vec::new(),
            ever_issued: BTreeSet::new(),
        }
    }

    /// How many more handles the model says can be allocated.
    fn room(&self) -> usize {
        self.limit
            .map_or(usize::MAX, |limit| limit.saturating_sub(self.model.len()))
    }

    /// An issued handle chosen by `i`, or `None` before the first allocation.
    fn pick(&self, i: usize) -> Option<Handle<()>> {
        (!self.issued.is_empty()).then(|| self.issued[i % self.issued.len()])
    }

    /// Records a handle the allocator just issued.
    fn register(&mut self, handle: Handle<()>) {
        let bits = handle.to_bits();
        assert!(self.ever_issued.insert(bits), "{handle:?} was issued twice");
        assert!(self.model.insert(bits), "{handle:?} was already live");
        self.issued.push(handle);
    }

    /// Records a batch, which must be exactly what `n` single `allocate` calls would issue.
    fn register_batch(&mut self, batch: Vec<Handle<()>>, mut replay: HandleAllocator<()>) {
        let expected: Vec<Handle<()>> = (0..batch.len()).map(|_| replay.allocate()).collect();
        assert_eq!(batch, expected, "a batch differs from single allocations");
        for handle in batch {
            self.register(handle);
        }
    }

    fn apply(&mut self, op: &Op) {
        match *op {
            Op::Allocate => {
                // `allocate` panics at the limit, as documented; `TryAllocate` covers that path.
                if self.room() > 0 {
                    let handle = self.allocator.allocate();
                    self.register(handle);
                }
            }
            Op::TryAllocate => {
                let had_room = self.room() > 0;
                match self.allocator.try_allocate() {
                    Some(handle) => {
                        assert!(had_room, "allocated from a full allocator");
                        self.register(handle);
                    }
                    None => assert!(!had_room, "refused to allocate below the limit"),
                }
            }
            Op::AllocateMany(n) => {
                let n = usize::from(n % 65);
                // Like `allocate`, `allocate_many` panics when it cannot fit `n` handles.
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
                        assert!(fits, "allocated {n} handles with room for {}", self.room());
                        assert_eq!(batch.len(), n);
                        self.register_batch(batch, replay);
                    }
                    // Nothing may have been allocated; `check` compares `len()` with the model.
                    None => assert!(!fits, "refused {n} handles with room for {}", self.room()),
                }
            }
            Op::Free(i) => {
                if let Some(handle) = self.pick(i) {
                    let expected = self.model.remove(&handle.to_bits());
                    assert_eq!(self.allocator.free(handle), expected);
                    assert!(!self.allocator.is_alive(handle));
                }
            }
            Op::IsAlive(i) => {
                if let Some(handle) = self.pick(i) {
                    assert_eq!(
                        self.allocator.is_alive(handle),
                        self.model.contains(&handle.to_bits())
                    );
                }
            }
            Op::Clear => {
                self.allocator.clear();
                self.model.clear();
            }
            Op::Probe(bits) => {
                if let Some(handle) = Handle::<()>::from_bits(bits) {
                    self.probe(handle);
                }
            }
            Op::ProbeNear(i, delta) => {
                if let Some(issued) = self.pick(i) {
                    let generation = issued
                        .generation()
                        .get()
                        .wrapping_add_signed(i32::from(delta));
                    let bits = (u64::from(issued.index()) << 32) | u64::from(generation);
                    if let Some(handle) = Handle::<()>::from_bits(bits) {
                        self.probe(handle);
                    }
                }
            }
        }
    }

    /// A handle the model does not hold must be reported not alive and must not be freeable,
    /// without panicking and without changing the allocator (`check` verifies the latter).
    fn probe(&mut self, handle: Handle<()>) {
        if self.model.contains(&handle.to_bits()) {
            return;
        }
        assert!(!self.allocator.is_alive(handle), "{handle:?} is not live");
        assert!(
            !self.allocator.free(handle),
            "freed {handle:?}, which is not live"
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
