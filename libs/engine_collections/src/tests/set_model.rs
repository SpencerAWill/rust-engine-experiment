//! Random operation sequences against a linear-search model of the order rules
//! (spec §4.3, with `()` values), with the real hasher and with one that maps every key to 0.

use std::vec::Vec;

use proptest::prelude::*;

use crate::IndexSet;
use crate::hasher::{Hasher, constant_hasher, default_hasher};

const KEYS: u8 = 16;

#[derive(Clone, Debug)]
enum Op {
    Insert(u8),
    SwapRemove(u8),
    ShiftRemove(u8),
    SwapRemoveIndex(usize),
    ShiftRemoveIndex(usize),
    Pop,
    RetainEven,
    Clear,
    Reserve(usize),
    ShrinkToFit,
    Sort,
}

/// What an operation returned, in a form both sides can produce.
#[derive(Debug, PartialEq, Eq)]
enum Ret {
    Flag(bool),
    Item(Option<u8>),
    Unit,
}

fn op() -> impl Strategy<Value = Op> {
    let key = 0..KEYS;
    prop_oneof![
        key.clone().prop_map(Op::Insert),
        key.clone().prop_map(Op::SwapRemove),
        key.prop_map(Op::ShiftRemove),
        (0..20_usize).prop_map(Op::SwapRemoveIndex),
        (0..20_usize).prop_map(Op::ShiftRemoveIndex),
        Just(Op::Pop),
        Just(Op::RetainEven),
        Just(Op::Clear),
        (0..64_usize).prop_map(Op::Reserve),
        Just(Op::ShrinkToFit),
        Just(Op::Sort),
    ]
}

fn apply_set(set: &mut IndexSet<u8>, op: &Op) -> Ret {
    match *op {
        Op::Insert(k) => Ret::Flag(set.insert(k)),
        Op::SwapRemove(k) => Ret::Flag(set.swap_remove(&k)),
        Op::ShiftRemove(k) => Ret::Flag(set.shift_remove(&k)),
        Op::SwapRemoveIndex(i) => Ret::Item(set.swap_remove_index(i)),
        Op::ShiftRemoveIndex(i) => Ret::Item(set.shift_remove_index(i)),
        Op::Pop => Ret::Item(set.pop()),
        Op::RetainEven => {
            set.retain(|k| k % 2 == 0);
            Ret::Unit
        }
        Op::Clear => {
            set.clear();
            Ret::Unit
        }
        Op::Reserve(n) => {
            set.reserve(n);
            Ret::Unit
        }
        Op::ShrinkToFit => {
            set.shrink_to_fit();
            Ret::Unit
        }
        Op::Sort => {
            set.sort();
            Ret::Unit
        }
    }
}

fn apply_model(model: &mut Vec<u8>, op: &Op) -> Ret {
    match *op {
        Op::Insert(k) => {
            let new = !model.contains(&k);
            if new {
                model.push(k);
            }
            Ret::Flag(new)
        }
        Op::SwapRemove(k) => Ret::Flag(
            model
                .iter()
                .position(|&x| x == k)
                .map(|i| model.swap_remove(i))
                .is_some(),
        ),
        Op::ShiftRemove(k) => Ret::Flag(
            model
                .iter()
                .position(|&x| x == k)
                .map(|i| model.remove(i))
                .is_some(),
        ),
        Op::SwapRemoveIndex(i) => Ret::Item((i < model.len()).then(|| model.swap_remove(i))),
        Op::ShiftRemoveIndex(i) => Ret::Item((i < model.len()).then(|| model.remove(i))),
        Op::Pop => Ret::Item(model.pop()),
        Op::RetainEven => {
            model.retain(|k| k % 2 == 0);
            Ret::Unit
        }
        Op::Clear => {
            model.clear();
            Ret::Unit
        }
        Op::Reserve(_) | Op::ShrinkToFit => Ret::Unit,
        Op::Sort => {
            model.sort_unstable();
            Ret::Unit
        }
    }
}

fn check_matches(set: &IndexSet<u8>, model: &[u8]) {
    assert_eq!(set.iter().copied().collect::<Vec<_>>(), model);
    assert_eq!(set.len(), model.len());
    assert_eq!(set.is_empty(), model.is_empty());
    assert_eq!(set.first(), model.first());
    assert_eq!(set.last(), model.last());
    for k in 0..KEYS {
        let position = model.iter().position(|&x| x == k);
        assert_eq!(set.get_index_of(&k), position, "position of {k}");
        assert_eq!(set.contains(&k), position.is_some());
        assert_eq!(set.get(&k), position.map(|i| &model[i]));
    }
    for (i, k) in model.iter().enumerate() {
        assert_eq!(set.get_index(i), Some(k));
    }
    assert_eq!(set.get_index(model.len()), None);
}

fn run(ops: &[Op], hasher: Hasher) {
    let mut set = IndexSet::with_test_hasher(hasher);
    let mut model = Vec::new();
    for op in ops {
        let got = apply_set(&mut set, op);
        let want = apply_model(&mut model, op);
        assert_eq!(got, want, "return value of {op:?}");
        check_matches(&set, &model);
    }
}

proptest! {
    #[test]
    fn set_matches_model_with_fold_hasher(ops in prop::collection::vec(op(), 0..200)) {
        run(&ops, default_hasher());
    }

    #[test]
    fn set_matches_model_with_constant_hasher(ops in prop::collection::vec(op(), 0..200)) {
        run(&ops, constant_hasher());
    }
}
