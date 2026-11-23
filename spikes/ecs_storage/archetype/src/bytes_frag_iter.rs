//! The `frag_iter` workload on runtime-registered components in byte columns, through the two
//! access paths of the M3.6 spec §4.4: [`BytesCopyFragIter`] decodes and encodes each value
//! through its [`Codec`], [`BytesCastFragIter`] casts each column to `[f32]` (the letters and
//! `Data` are `f32` newtypes, so no mirror type is needed).
//!
//! Setup registers the 26 letters in `LETTERS` order, then `Data`, and spawns each entity with
//! one `spawn_raw`, so the world has the typed backend's 27 archetypes (the empty one and one per
//! letter, created in the same order because the entities are spawned in the same order).

use engine::prelude::Prng;
use spike_ecs_harness::frag_iter::{
    self as rules, A, B, C, D, Data, E, F, FragIter, G, H, I, J, K, L, LETTERS, Letter, M, N, O, P,
    Q, R, S, T, U, V, W, X, Y, Z,
};
use spike_ecs_harness::{Params, Workload};

use crate::codec::{Codec, layout, read_f32s, write_f32s};
use crate::raw::RawLayout;
use crate::{ComponentId, RawQuery, RawTable, World};

const F32_LAYOUT: RawLayout = layout(4, 4);

/// Implements [`Codec`] for a newtype over one `f32`, and defines the two functions that need
/// one arm per letter type.
macro_rules! per_letter {
    ($($name:ident = $index:literal),+ $(,)?) => {
        impl Codec for Data {
            const LAYOUT: RawLayout = F32_LAYOUT;

            fn encode(&self, out: &mut [u8]) {
                write_f32s(&[self.0], out);
            }

            fn decode(bytes: &[u8]) -> Self {
                let [value] = read_f32s(bytes);
                Self(value)
            }
        }

        $(
            impl Codec for $name {
                const LAYOUT: RawLayout = F32_LAYOUT;

                fn encode(&self, out: &mut [u8]) {
                    write_f32s(&[self.0], out);
                }

                fn decode(bytes: &[u8]) -> Self {
                    let [value] = read_f32s(bytes);
                    Self(value)
                }
            }
        )+

        /// The letter's payload encoded through its [`Codec`].
        fn encode_letter(letter: &Letter) -> [u8; 4] {
            let mut bytes = [0; 4];
            match letter {
                $(Letter::$name(x) => x.encode(&mut bytes),)+
            }
            bytes
        }

        /// The letter with the given index, decoded through its [`Codec`].
        fn decode_letter(index: u8, bytes: &[u8]) -> Letter {
            match index {
                $($index => Letter::$name($name::decode(bytes)),)+
                _ => panic!("letter index {index} is not below {LETTERS}"),
            }
        }
    };
}

per_letter!(
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

/// The step's query, shared with the test hook so the two cannot drift.
fn step_query(data: ComponentId) -> RawQuery {
    RawQuery::new().write(data)
}

/// The checksum's query for one letter.
fn checksum_query(letter: ComponentId, data: ComponentId) -> RawQuery {
    RawQuery::new().read(letter).read(data)
}

/// The world and the queries both backends share: the step's, and one checksum query per letter
/// (all built once, at setup).
#[derive(Debug)]
struct Base {
    world: World,
    step: RawQuery,
    checksum: Vec<RawQuery>,
}

impl Base {
    /// Registers the 26 letters and `Data`, and spawns entities `0..26 * count`; `bytes` turns
    /// one entity's letter and data into the two byte values.
    fn new(params: Params, bytes: impl Fn(&Letter, &Data) -> ([u8; 4], [u8; 4])) -> Self {
        let mut world = World::new();
        let letters: Vec<ComponentId> = (0..LETTERS)
            .map(|i| world.register_raw(letter_name(i), F32_LAYOUT))
            .collect();
        let data = world.register_raw("Data", Data::LAYOUT);
        let prng = Prng::from_seed(params.seed);
        let population = u64::from(LETTERS) * params.count as u64;
        for i in 0..population {
            let (letter, value) = rules::spawn(&prng, i, params.count);
            let (letter_bytes, data_bytes) = bytes(&letter, &value);
            let id = letters[usize::from(letter.index())];
            world
                .spawn_raw(&[(id, &letter_bytes), (data, &data_bytes)])
                .expect("the ids are registered raw and listed once, with their layouts' sizes");
        }
        Self {
            world,
            step: step_query(data),
            checksum: letters.iter().map(|&l| checksum_query(l, data)).collect(),
        }
    }

    /// Runs `f` on each table of the step's query.
    fn step(&mut self, f: impl FnMut(RawTable<'_, '_>)) {
        self.world
            .query_raw_mut(&self.step)
            .expect("the step's id is registered raw")
            .for_each_table(f);
    }

    /// Runs `f` with the letter's index on each table of that letter's checksum query.
    fn checksum(&self, mut f: impl FnMut(u8, RawTable<'_, '_>)) {
        for (index, query) in (0..LETTERS).zip(&self.checksum) {
            self.world
                .query_raw(query)
                .expect("the checksum's ids are registered raw")
                .for_each_table(|table| f(index, table));
        }
    }
}

/// The registered name of a letter's component: `"A"` to `"Z"`.
fn letter_name(index: u8) -> &'static str {
    const NAMES: [&str; 26] = [
        "A", "B", "C", "D", "E", "F", "G", "H", "I", "J", "K", "L", "M", "N", "O", "P", "Q", "R",
        "S", "T", "U", "V", "W", "X", "Y", "Z",
    ];
    NAMES[usize::from(index)]
}

/// `frag_iter` on byte columns, copying each value in and out through its [`Codec`].
#[derive(Debug)]
pub struct BytesCopyFragIter {
    base: Base,
}

/// `frag_iter` on byte columns, casting each column to `[f32]`.
#[derive(Debug)]
pub struct BytesCastFragIter {
    base: Base,
}

impl Workload for BytesCopyFragIter {
    type Spec = FragIter;
    const BACKEND: &'static str = "archetype_bytes_copy";

    fn setup(params: Params) -> Self {
        let base = Base::new(params, |letter, data| {
            let mut data_bytes = [0; 4];
            data.encode(&mut data_bytes);
            (encode_letter(letter), data_bytes)
        });
        Self { base }
    }

    fn step(&mut self) {
        self.base.step(|mut table| {
            for row in table.column_mut(0).rows_mut() {
                let mut data = Data::decode(row);
                rules::update(&mut data);
                data.encode(row);
            }
        });
    }

    fn checksum(&self) -> u64 {
        let mut sum = 0_u64;
        self.base.checksum(|index, table| {
            let letters = table.column(0);
            let data = table.column(1);
            for (l, d) in letters.rows().zip(data.rows()) {
                let letter = decode_letter(index, l);
                sum = sum.wrapping_add(rules::hash(&letter, &Data::decode(d)));
            }
        });
        sum
    }
}

impl Workload for BytesCastFragIter {
    type Spec = FragIter;
    const BACKEND: &'static str = "archetype_bytes_cast";

    fn setup(params: Params) -> Self {
        let base = Base::new(params, |letter, data| {
            (
                bytemuck::bytes_of(&letter.value())
                    .try_into()
                    .expect("an f32 is 4 bytes"),
                bytemuck::bytes_of(&data.0)
                    .try_into()
                    .expect("an f32 is 4 bytes"),
            )
        });
        Self { base }
    }

    fn step(&mut self) {
        self.base.step(|mut table| {
            let data = table
                .column_mut(0)
                .cast_mut::<f32>()
                .expect("Data's layout matches f32");
            for x in data {
                let mut value = Data(*x);
                rules::update(&mut value);
                *x = value.0;
            }
        });
    }

    fn checksum(&self) -> u64 {
        let mut sum = 0_u64;
        self.base.checksum(|index, table| {
            let letters = table
                .column(0)
                .cast::<f32>()
                .expect("a letter's layout matches f32");
            let data = table
                .column(1)
                .cast::<f32>()
                .expect("Data's layout matches f32");
            for (l, d) in letters.iter().zip(data) {
                let letter = Letter::from_index(index, *l);
                sum = sum.wrapping_add(rules::hash(&letter, &Data(*d)));
            }
        });
        sum
    }
}

/// The two backends' hooks for tests: the same query function the step uses (ecs-1).
macro_rules! test_hooks {
    ($($backend:ty),*) => {$(
        #[cfg(test)]
        impl $backend {
            /// The archetypes the step's query matches, for tests.
            pub(crate) fn step_archetypes(&mut self) -> Vec<crate::archetype::ArchetypeId> {
                self.base
                    .world
                    .query_raw_mut(&self.base.step)
                    .expect("the step's id is registered raw")
                    .matched_archetypes()
            }

            /// The number of archetypes in the world, for tests.
            pub(crate) fn archetype_count(&self) -> usize {
                self.base.world.archetype_count()
            }

            /// The world, for invariant checks in tests.
            pub(crate) fn world(&self) -> &World {
                &self.base.world
            }
        }
    )*};
}

test_hooks!(BytesCopyFragIter, BytesCastFragIter);
