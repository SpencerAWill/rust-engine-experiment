//! Fuzzes `DenseHandleAllocator` against a `BTreeSet` reference model, checking the allocator's
//! internal invariants and its iteration after every operation.

#![no_main]
#![forbid(unsafe_code)]

use std::collections::BTreeSet;

use arbitrary::Arbitrary;
use engine_handle::{DenseHandleAllocator, Handle};
use libfuzzer_sys::fuzz_target;

#[derive(Arbitrary, Debug)]
enum Op {
    Allocate,
    TryAllocate,
    Free(usize),
    IsAlive(usize),
    /// Keeps a handle when `index % (k % 8 + 1) != 0`.
    Retain(u8),
    Clear,
    Iterate,
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
    allocator: DenseHandleAllocator<()>,
    /// `to_bits` of the live handles. Derived only from the ops applied so far.
    model: BTreeSet<u64>,
    /// The fixed capacity, if any.
    limit: Option<usize>,
    /// Every handle the allocator has issued, for modulo selection (stale ones included).
    issued: Vec<Handle<()>>,
    /// `to_bits` of every handle issued so far: no pair may ever be issued twice, not even
    /// across a `clear`.
    ever_issued: BTreeSet<u64>,
}

impl Harness {
    fn new(capacity: Option<u8>) -> Self {
        let limit = capacity.map(|c| usize::from(c % 17));
        Self {
            allocator: limit.map_or_else(
                DenseHandleAllocator::new,
                DenseHandleAllocator::with_fixed_capacity,
            ),
            model: BTreeSet::new(),
            limit,
            issued: Vec::new(),
            ever_issued: BTreeSet::new(),
        }
    }

    fn at_limit(&self) -> bool {
        self.limit.is_some_and(|limit| self.model.len() >= limit)
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

    fn apply(&mut self, op: &Op) {
        match *op {
            Op::Allocate => {
                // `allocate` panics at the limit, as documented; `TryAllocate` covers that path.
                if !self.at_limit() {
                    let handle = self.allocator.allocate();
                    self.register(handle);
                }
            }
            Op::TryAllocate => {
                let was_full = self.at_limit();
                match self.allocator.try_allocate() {
                    Some(handle) => {
                        assert!(!was_full, "allocated from a full allocator");
                        self.register(handle);
                    }
                    None => assert!(was_full, "refused to allocate below the limit"),
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
            Op::Retain(k) => {
                let k = u32::from(k % 8) + 1;
                let mut visited = BTreeSet::new();
                let model = &self.model;
                self.allocator.retain(|handle| {
                    assert!(model.contains(&handle.to_bits()), "{handle:?} is not live");
                    assert!(visited.insert(handle.to_bits()), "{handle:?} visited twice");
                    handle.index() % k != 0
                });
                assert_eq!(visited, self.model, "retain skipped a live handle");
                self.model.retain(|bits| ((bits >> 32) as u32) % k != 0);
            }
            Op::Clear => {
                self.allocator.clear();
                self.model.clear();
            }
            // `check` compares `iter()` with the model after every op; this op only gives the
            // fuzzer a way to put that comparison right after chosen operations.
            Op::Iterate => self.check_iteration(),
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

    /// `iter()` yields exactly the live handles, each once, in an order that is stable between
    /// calls (the dense order itself is an implementation detail).
    fn check_iteration(&self) {
        let first: Vec<u64> = self.allocator.iter().map(Handle::to_bits).collect();
        let second: Vec<u64> = self.allocator.iter().map(Handle::to_bits).collect();
        assert_eq!(first, second, "iteration order changed between calls");
        assert_eq!(self.allocator.iter().len(), first.len());
        let as_set: BTreeSet<u64> = first.iter().copied().collect();
        assert_eq!(as_set.len(), first.len(), "iter() yielded a handle twice");
        assert_eq!(as_set, self.model);
    }

    /// Everything that must hold after every operation.
    fn check(&self) {
        self.allocator
            .check_invariants()
            .expect("the allocator's invariants must hold");
        assert_eq!(self.allocator.len(), self.model.len());
        assert_eq!(self.allocator.is_empty(), self.model.is_empty());
        self.check_iteration();
    }
}

fuzz_target!(|input: Input| {
    let mut harness = Harness::new(input.capacity);
    for op in &input.ops {
        harness.apply(op);
        harness.check();
    }
});
