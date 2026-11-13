//! Random operation sequences against a linear-search model of the order rules
//! (spec §4.3), with the real hasher and with one that maps every key to 0.

use std::vec::Vec;

use proptest::prelude::*;

use crate::IndexMap;
use crate::hasher::{Hasher, constant_hasher, default_hasher};

const KEYS: u8 = 16;

#[derive(Clone, Debug)]
enum Op {
    Insert(u8, u16),
    EntryOrInsert(u8, u16),
    EntryAndModify(u8, u16),
    GetMutSet(u8, u16),
    SwapRemove(u8),
    ShiftRemove(u8),
    SwapRemoveIndex(usize),
    ShiftRemoveIndex(usize),
    Pop,
    RetainEvenValues,
    Clear,
    Reserve(usize),
    ShrinkToFit,
    SortKeys,
}

/// What an operation returned, in a form both sides can produce.
#[derive(Debug, PartialEq, Eq)]
enum Ret {
    Value(Option<u16>),
    Pair(Option<(u8, u16)>),
    Unit,
}

fn op() -> impl Strategy<Value = Op> {
    let key = 0..KEYS;
    prop_oneof![
        (key.clone(), any::<u16>()).prop_map(|(k, v)| Op::Insert(k, v)),
        (key.clone(), any::<u16>()).prop_map(|(k, v)| Op::EntryOrInsert(k, v)),
        (key.clone(), any::<u16>()).prop_map(|(k, v)| Op::EntryAndModify(k, v)),
        (key.clone(), any::<u16>()).prop_map(|(k, v)| Op::GetMutSet(k, v)),
        key.clone().prop_map(Op::SwapRemove),
        key.prop_map(Op::ShiftRemove),
        (0..20_usize).prop_map(Op::SwapRemoveIndex),
        (0..20_usize).prop_map(Op::ShiftRemoveIndex),
        Just(Op::Pop),
        Just(Op::RetainEvenValues),
        Just(Op::Clear),
        (0..64_usize).prop_map(Op::Reserve),
        Just(Op::ShrinkToFit),
        Just(Op::SortKeys),
    ]
}

fn apply_map(map: &mut IndexMap<u8, u16>, op: &Op) -> Ret {
    match *op {
        Op::Insert(k, v) => Ret::Value(map.insert(k, v)),
        Op::EntryOrInsert(k, v) => Ret::Value(Some(*map.entry(k).or_insert(v))),
        Op::EntryAndModify(k, v) => {
            Ret::Value(Some(*map.entry(k).and_modify(|x| *x = v).or_insert(v)))
        }
        Op::GetMutSet(k, v) => Ret::Value(map.get_mut(&k).map(|x| core::mem::replace(x, v))),
        Op::SwapRemove(k) => Ret::Value(map.swap_remove(&k)),
        Op::ShiftRemove(k) => Ret::Value(map.shift_remove(&k)),
        Op::SwapRemoveIndex(i) => Ret::Pair(map.swap_remove_index(i)),
        Op::ShiftRemoveIndex(i) => Ret::Pair(map.shift_remove_index(i)),
        Op::Pop => Ret::Pair(map.pop()),
        Op::RetainEvenValues => {
            map.retain(|_, v| *v % 2 == 0);
            Ret::Unit
        }
        Op::Clear => {
            map.clear();
            Ret::Unit
        }
        Op::Reserve(n) => {
            map.reserve(n);
            Ret::Unit
        }
        Op::ShrinkToFit => {
            map.shrink_to_fit();
            Ret::Unit
        }
        Op::SortKeys => {
            map.sort_keys();
            Ret::Unit
        }
    }
}

/// The order rules of spec §4.3, written as directly as possible.
fn apply_model(model: &mut Vec<(u8, u16)>, op: &Op) -> Ret {
    let find = |m: &Vec<(u8, u16)>, k: u8| m.iter().position(|&(key, _)| key == k);
    match *op {
        Op::Insert(k, v) => {
            if let Some(i) = find(model, k) {
                Ret::Value(Some(core::mem::replace(&mut model[i].1, v)))
            } else {
                model.push((k, v));
                Ret::Value(None)
            }
        }
        Op::EntryOrInsert(k, v) => {
            if let Some(i) = find(model, k) {
                Ret::Value(Some(model[i].1))
            } else {
                model.push((k, v));
                Ret::Value(Some(v))
            }
        }
        Op::EntryAndModify(k, v) => {
            if let Some(i) = find(model, k) {
                model[i].1 = v;
            } else {
                model.push((k, v));
            }
            Ret::Value(Some(v))
        }
        Op::GetMutSet(k, v) => {
            Ret::Value(find(model, k).map(|i| core::mem::replace(&mut model[i].1, v)))
        }
        Op::SwapRemove(k) => Ret::Value(find(model, k).map(|i| model.swap_remove(i).1)),
        Op::ShiftRemove(k) => Ret::Value(find(model, k).map(|i| model.remove(i).1)),
        Op::SwapRemoveIndex(i) => Ret::Pair((i < model.len()).then(|| model.swap_remove(i))),
        Op::ShiftRemoveIndex(i) => Ret::Pair((i < model.len()).then(|| model.remove(i))),
        Op::Pop => Ret::Pair(model.pop()),
        Op::RetainEvenValues => {
            model.retain(|&(_, v)| v % 2 == 0);
            Ret::Unit
        }
        Op::Clear => {
            model.clear();
            Ret::Unit
        }
        Op::Reserve(_) | Op::ShrinkToFit => Ret::Unit,
        Op::SortKeys => {
            model.sort_by_key(|&(k, _)| k);
            Ret::Unit
        }
    }
}

fn check_matches(map: &IndexMap<u8, u16>, model: &[(u8, u16)]) {
    let entries: Vec<(u8, u16)> = map.iter().map(|(k, v)| (*k, *v)).collect();
    assert_eq!(entries, model);
    assert_eq!(map.len(), model.len());
    assert_eq!(map.is_empty(), model.is_empty());
    assert_eq!(map.first().map(|(k, v)| (*k, *v)), model.first().copied());
    assert_eq!(map.last().map(|(k, v)| (*k, *v)), model.last().copied());
    for k in 0..KEYS {
        let position = model.iter().position(|&(key, _)| key == k);
        assert_eq!(map.get_index_of(&k), position, "position of {k}");
        assert_eq!(
            map.get(&k).copied(),
            position.map(|i| model[i].1),
            "value of {k}"
        );
        assert_eq!(map.contains_key(&k), position.is_some());
    }
    for (i, &(k, v)) in model.iter().enumerate() {
        assert_eq!(map.get_index(i), Some((&k, &v)));
    }
    assert_eq!(map.get_index(model.len()), None);
}

fn run(ops: &[Op], hasher: Hasher) {
    let mut map = IndexMap::with_test_hasher(hasher);
    let mut model = Vec::new();
    for op in ops {
        let got = apply_map(&mut map, op);
        let want = apply_model(&mut model, op);
        assert_eq!(got, want, "return value of {op:?}");
        check_matches(&map, &model);
    }
}

proptest! {
    #[test]
    fn map_matches_model_with_fold_hasher(ops in prop::collection::vec(op(), 0..200)) {
        run(&ops, default_hasher());
    }

    #[test]
    fn map_matches_model_with_constant_hasher(ops in prop::collection::vec(op(), 0..200)) {
        run(&ops, constant_hasher());
    }
}
