//! A fixed scripted sequence of operations, with the resulting order written out literally.

use std::vec::Vec;

use crate::{IndexMap, IndexSet};

#[test]
fn map_golden_order() {
    let mut m = IndexMap::new();
    for (k, v) in [('a', 1_u32), ('b', 2), ('c', 3), ('d', 4), ('e', 5)] {
        m.insert(k, v);
    }
    m.insert('b', 20);
    m.swap_remove(&'a');
    assert_eq!(m.keys().copied().collect::<Vec<_>>(), ['e', 'b', 'c', 'd']);
    m.insert('f', 6);
    m.shift_remove(&'c');
    m.entry('g').or_insert(7);
    m.entry('b').or_insert(99);
    m.pop();
    m.retain(|_, v| *v % 2 == 0);
    m.insert('a', 1);
    m.swap_remove_index(0);
    m.shift_remove_index(1);
    m.insert('c', 3);
    m.insert('b', 2);
    m.sort_keys();
    m.insert('z', 26);

    let got: Vec<(char, u32)> = m.iter().map(|(k, v)| (*k, *v)).collect();
    assert_eq!(got, [('a', 1), ('b', 2), ('c', 3), ('f', 6), ('z', 26)]);
}

#[test]
fn set_golden_order() {
    let mut s = IndexSet::new();
    for k in [5, 3, 8, 1] {
        s.insert(k);
    }
    s.swap_remove(&5);
    assert!(!s.insert(3));
    s.insert(9);
    s.shift_remove(&8);
    assert_eq!(s.iter().copied().collect::<Vec<_>>(), [1, 3, 9]);
    s.sort();
    assert_eq!(s.iter().copied().collect::<Vec<_>>(), [1, 3, 9]);
    s.insert(0);
    assert_eq!(s.iter().copied().collect::<Vec<_>>(), [1, 3, 9, 0]);
}
