//! Fuzzes `GenIdMap` against a `BTreeMap` reference model, checking the map's internal
//! invariants and its drop accounting after every operation.

#![no_main]
#![forbid(unsafe_code)]

use std::cell::Cell;
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

use arbitrary::Arbitrary;
use engine_genid::{GenId, GenIdMap};
use libfuzzer_sys::fuzz_target;

/// The map's value type: counts its own drops in a counter shared with the harness, so a value
/// that is leaked or dropped twice breaks the accounting equation.
struct Tracked {
    value: u16,
    drops: Rc<Cell<u32>>,
}

impl Drop for Tracked {
    fn drop(&mut self) {
        self.drops.set(self.drops.get() + 1);
    }
}

#[derive(Arbitrary, Debug)]
enum Op {
    Insert(u16),
    TryInsert(u16),
    Remove(usize),
    Get(usize),
    GetMut(usize, u16),
    GetDisjoint(usize, usize),
    /// Keeps a value when `value % (k % 8 + 1) != 0`.
    Retain(u16),
    /// Drains the map. An even `n` consumes every value; an odd `n` takes only `n / 2 % 8`
    /// values and drops the iterator early, which must still empty the map.
    Drain(u8),
    Clear,
    Iterate,
    /// An ID built from arbitrary bits, which the map probably never issued.
    Probe(u64),
    /// An issued ID's slot with a nearby generation (a stale or never-issued neighbour).
    ProbeNear(usize, i8),
}

#[derive(Arbitrary, Debug)]
struct Input {
    /// `None` is an unbounded map; `Some(c)` is a fixed capacity of `c % 17`.
    capacity: Option<u8>,
    ops: Vec<Op>,
}

/// The map under test, its model, and the bookkeeping the checks need.
struct Harness {
    map: GenIdMap<Tracked>,
    /// Live IDs by `to_bits`, with their values. Derived only from the ops applied so far.
    model: BTreeMap<u64, u16>,
    /// The fixed capacity, if any.
    limit: Option<usize>,
    /// Every ID the map has issued, for modulo selection (stale ones included).
    issued: Vec<GenId<Tracked>>,
    /// `to_bits` of every ID issued so far: no pair may ever be issued twice.
    ever_issued: BTreeSet<u64>,
    drops: Rc<Cell<u32>>,
    /// How many `Tracked` values have been constructed, including rejected ones.
    created: u32,
}

impl Harness {
    fn new(capacity: Option<u8>) -> Self {
        let limit = capacity.map(|c| usize::from(c % 17));
        Self {
            map: limit.map_or_else(GenIdMap::new, GenIdMap::with_fixed_capacity),
            model: BTreeMap::new(),
            limit,
            issued: Vec::new(),
            ever_issued: BTreeSet::new(),
            drops: Rc::new(Cell::new(0)),
            created: 0,
        }
    }

    fn make(&mut self, value: u16) -> Tracked {
        self.created += 1;
        Tracked {
            value,
            drops: Rc::clone(&self.drops),
        }
    }

    fn at_limit(&self) -> bool {
        self.limit.is_some_and(|limit| self.model.len() >= limit)
    }

    /// An issued ID chosen by `i`, or `None` before the first insert.
    fn pick(&self, i: usize) -> Option<GenId<Tracked>> {
        (!self.issued.is_empty()).then(|| self.issued[i % self.issued.len()])
    }

    /// Records an ID the map just issued for `value`.
    fn register(&mut self, id: GenId<Tracked>, value: u16) {
        let bits = id.to_bits();
        assert!(self.ever_issued.insert(bits), "{id:?} was issued twice");
        assert!(
            self.model.insert(bits, value).is_none(),
            "{id:?} was already live"
        );
        self.issued.push(id);
    }

    fn apply(&mut self, op: &Op) {
        match *op {
            Op::Insert(value) => {
                // `insert` panics at the limit, as documented; `TryInsert` covers that path.
                if !self.at_limit() {
                    let tracked = self.make(value);
                    let id = self.map.insert(tracked);
                    self.register(id, value);
                }
            }
            Op::TryInsert(value) => {
                let was_full = self.at_limit();
                let tracked = self.make(value);
                match self.map.try_insert(tracked) {
                    Ok(id) => {
                        assert!(!was_full, "inserted into a full map");
                        self.register(id, value);
                    }
                    Err(error) => {
                        assert!(was_full, "rejected an insert below the limit");
                        assert_eq!(Some(error.limit() as usize), self.limit);
                        assert_eq!(error.into_inner().value, value);
                    }
                }
            }
            Op::Remove(i) => {
                if let Some(id) = self.pick(i) {
                    let expected = self.model.remove(&id.to_bits());
                    assert_eq!(self.map.remove(id).map(|t| t.value), expected);
                }
            }
            Op::Get(i) => {
                if let Some(id) = self.pick(i) {
                    let expected = self.model.get(&id.to_bits()).copied();
                    assert_eq!(self.map.get(id).map(|t| t.value), expected);
                    assert_eq!(self.map.contains(id), expected.is_some());
                }
            }
            Op::GetMut(i, value) => {
                if let Some(id) = self.pick(i) {
                    match (
                        self.map.get_mut(id),
                        self.model.get_mut(&id.to_bits()),
                    ) {
                        (Some(tracked), Some(modelled)) => {
                            assert_eq!(tracked.value, *modelled);
                            tracked.value = value;
                            *modelled = value;
                        }
                        (None, None) => {}
                        (got, expected) => panic!(
                            "get_mut disagrees for {id:?}: map {}, model {}",
                            got.is_some(),
                            expected.is_some()
                        ),
                    }
                }
            }
            Op::GetDisjoint(i, j) => {
                if let (Some(a), Some(b)) = (self.pick(i), self.pick(j)) {
                    let (abits, bbits) = (a.to_bits(), b.to_bits());
                    let expect = a != b
                        && self.model.contains_key(&abits)
                        && self.model.contains_key(&bbits);
                    match self.map.get_disjoint_mut([a, b]) {
                        Some([x, y]) => {
                            assert!(expect, "get_disjoint_mut succeeded for {a:?}, {b:?}");
                            std::mem::swap(&mut x.value, &mut y.value);
                            let (va, vb) = (self.model[&abits], self.model[&bbits]);
                            self.model.insert(abits, vb);
                            self.model.insert(bbits, va);
                        }
                        None => assert!(!expect, "get_disjoint_mut failed for {a:?}, {b:?}"),
                    }
                }
            }
            Op::Retain(k) => {
                let k = k % 8 + 1;
                let model = &self.model;
                self.map.retain(|id, tracked| {
                    assert_eq!(model.get(&id.to_bits()), Some(&tracked.value));
                    tracked.value % k != 0
                });
                self.model.retain(|_, value| *value % k != 0);
            }
            Op::Drain(n) => {
                let mut drain = self.map.drain();
                let drained: Vec<(GenId<Tracked>, Tracked)> = if n % 2 == 0 {
                    drain.by_ref().collect()
                } else {
                    drain.by_ref().take(usize::from(n / 2 % 8)).collect()
                };
                // Dropping the iterator drops whatever was not consumed.
                drop(drain);
                let seen: BTreeMap<u64, u16> = drained
                    .iter()
                    .map(|(id, tracked)| (id.to_bits(), tracked.value))
                    .collect();
                assert_eq!(seen.len(), drained.len(), "drain yielded an ID twice");
                for (bits, value) in &seen {
                    assert_eq!(self.model.get(bits), Some(value), "drain invented a value");
                }
                if n % 2 == 0 {
                    assert_eq!(seen, self.model, "a full drain missed values");
                }
                drop(drained);
                self.model.clear();
            }
            Op::Clear => {
                self.map.clear();
                self.model.clear();
            }
            Op::Iterate => {
                // `check` compares `iter()`; this compares the dense value slice too.
                let mut values: Vec<u16> = self.map.values().iter().map(|t| t.value).collect();
                let mut expected: Vec<u16> = self.model.values().copied().collect();
                values.sort_unstable();
                expected.sort_unstable();
                assert_eq!(values, expected);
            }
            Op::Probe(bits) => {
                if let Some(id) = GenId::<Tracked>::from_bits(bits) {
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
                    if let Some(id) = GenId::<Tracked>::from_bits(bits) {
                        self.probe(id);
                    }
                }
            }
        }
    }

    /// An ID the model does not hold must be reported absent everywhere, without panicking
    /// and without changing the map (`check` verifies the latter).
    fn probe(&mut self, id: GenId<Tracked>) {
        if self.model.contains_key(&id.to_bits()) {
            return;
        }
        assert!(!self.map.contains(id), "{id:?} is not live");
        assert!(self.map.get(id).is_none());
        assert!(self.map.get_mut(id).is_none());
        assert!(self.map.remove(id).is_none());
    }

    /// Everything that must hold after every operation.
    fn check(&self) {
        self.map
            .check_invariants()
            .expect("the map's invariants must hold");
        assert_eq!(self.map.len(), self.model.len());
        assert_eq!(self.map.is_empty(), self.model.is_empty());

        let snapshot = |map: &GenIdMap<Tracked>| -> Vec<(u64, u16)> {
            map.iter()
                .map(|(id, tracked)| (id.to_bits(), tracked.value))
                .collect()
        };
        let first = snapshot(&self.map);
        // The dense order is an implementation detail, but it must be stable between calls.
        assert_eq!(first, snapshot(&self.map));
        assert_eq!(self.map.iter().len(), first.len());
        let as_set: BTreeMap<u64, u16> = first.iter().copied().collect();
        assert_eq!(as_set.len(), first.len(), "iter() yielded an ID twice");
        assert_eq!(as_set, self.model);

        // Every value ever built is either live or has been dropped exactly once.
        let live = u32::try_from(self.model.len()).expect("a model this small fits in u32");
        assert_eq!(
            self.created,
            live + self.drops.get(),
            "values leaked or double-dropped"
        );
    }

    /// Drops the map and checks that nothing was leaked.
    fn finish(self) {
        let Self {
            map,
            drops,
            created,
            ..
        } = self;
        drop(map);
        assert_eq!(
            created,
            drops.get(),
            "values leaked when the map was dropped"
        );
    }
}

fuzz_target!(|input: Input| {
    let mut harness = Harness::new(input.capacity);
    for op in &input.ops {
        harness.apply(op);
        harness.check();
    }
    harness.finish();
});
