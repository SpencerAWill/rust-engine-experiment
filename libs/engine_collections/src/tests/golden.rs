//! A fixed scripted sequence of operations, with the resulting order written out literally.

use std::vec::Vec;

use crate::IndexMap;

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
