//! `frag_iter`: 26 groups of entities that differ only in one component type, and a step that
//! updates a component they all share (spec §4.4.2).

use engine::prelude::Prng;

use crate::{Fnv1a, Spec};

/// The `frag_iter` workload's identity. `Params::count` is the number of entities per letter.
#[derive(Clone, Copy, Debug)]
pub struct FragIter;

impl Spec for FragIter {
    const NAME: &'static str = "frag_iter";
    const SIZES: &'static [usize] = &[20];
}

/// The number of letter types.
pub const LETTERS: u8 = 26;

macro_rules! letters {
    ($($name:ident = $index:literal),+ $(,)?) => {
        $(
            #[doc = concat!("The letter component `", stringify!($name), "`.")]
            #[derive(Clone, Copy, Debug, PartialEq)]
            pub struct $name(pub f32);
        )+

        /// One of the 26 letter components. A backend with a type per letter stores each
        /// variant's payload in that letter's own group; the `Vec` backend stores the enum.
        #[derive(Clone, Copy, Debug, PartialEq)]
        pub enum Letter {
            $(
                #[doc = concat!("The letter `", stringify!($name), "`.")]
                $name($name),
            )+
        }

        impl Letter {
            /// The letter's index: 0 for `A`, 25 for `Z`.
            #[must_use]
            pub fn index(&self) -> u8 {
                match self { $(Self::$name(_) => $index,)+ }
            }

            /// The letter component's value.
            #[must_use]
            pub fn value(&self) -> f32 {
                match self { $(Self::$name(x) => x.0,)+ }
            }

            /// The letter with the given index and value.
            ///
            /// # Panics
            ///
            /// If `index` is not below [`LETTERS`].
            #[must_use]
            pub fn from_index(index: u8, value: f32) -> Self {
                match index {
                    $($index => Self::$name($name(value)),)+
                    _ => panic!("letter index {index} is not below {LETTERS}"),
                }
            }
        }
    };
}

letters!(
    A = 0,
    B = 1,
    C = 2,
    D = 3,
    E = 4,
    F = 5,
    G = 6,
    H = 7,
    I = 8,
    J = 9,
    K = 10,
    L = 11,
    M = 12,
    N = 13,
    O = 14,
    P = 15,
    Q = 16,
    R = 17,
    S = 18,
    T = 19,
    U = 20,
    V = 21,
    W = 22,
    X = 23,
    Y = 24,
    Z = 25,
);

/// The component every entity shares, and the step updates.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Data(pub f32);

/// Entity `i`'s components: the letter `i / count` with one draw in `0..1`, then `Data` with one
/// draw in `0..1`. `count` is the number of entities per letter.
///
/// # Panics
///
/// If `count` is 0 or `i` is not below `26 * count`, with a message that says which.
#[must_use]
pub fn spawn(prng: &Prng, i: u64, count: usize) -> (Letter, Data) {
    assert!(count >= 1, "frag_iter: count must be at least 1");
    let index = i / count as u64;
    assert!(
        index < u64::from(LETTERS),
        "frag_iter: entity index {i} is not below 26 * count ({})",
        u64::from(LETTERS) * count as u64
    );
    let index = u8::try_from(index).expect("index is below 26, checked above");
    let mut r = prng.stream(i);
    let value = r.range(0.0_f32..1.0);
    let data = r.range(0.0_f32..1.0);
    (Letter::from_index(index, value), Data(data))
}

/// The rule for one entity: `x = data + 0.618_034`, then `data = x - 1.0` if `x >= 1.0`, else
/// `x`. Values stay in `0.0..1.0`. A rotation by a constant near the golden ratio's conjugate, so
/// no state repeats an earlier tick within the conformance run.
#[inline]
pub fn update(data: &mut Data) {
    let x = data.0 + 0.618_034;
    data.0 = if x >= 1.0 { x - 1.0 } else { x };
}

/// One entity's term of the checksum: `u8(letter index)`, `f32(letter value)`, `f32(data)`.
#[must_use]
pub fn hash(letter: &Letter, data: &Data) -> u64 {
    Fnv1a::new()
        .u8(letter.index())
        .f32(letter.value())
        .f32(data.0)
        .finish()
}
