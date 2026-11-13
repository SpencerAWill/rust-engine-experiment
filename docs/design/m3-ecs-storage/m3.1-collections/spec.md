# M3.1: `engine_collections`: deterministic map and set

- **Status:** Implemented (PR #26, 2026-11-13). Changed during implementation: the backend is named through a type alias and `extern crate alloc` is left out (§4.1); the model tests are unit tests so the constant hasher is reachable (§8); two benchmarks were renamed and enlarged to reach the gate's 10,000-instruction minimum (§4.8). See the plan's §4.
- **Milestone:** M3 (ECS storage decision) · **Depends on:** none · **Decides:** no ADR
- **Estimated size:** about 850 changed lines, excluding tests, lockfiles and generated files (`IndexMap` about 330, `IndexSet` about 170, iterator, entry and error wrappers about 170, benchmarks about 80, configuration and documents about 100)

Requirement keywords (MUST, MUST NOT, SHOULD, MAY) follow [RFC 2119](https://www.rfc-editor.org/rfc/rfc2119) as clarified by [RFC 8174](https://www.rfc-editor.org/rfc/rfc8174): they are normative only in uppercase.

## 1. Summary

This chunk adds `libs/engine_collections`, a `no_std` crate with two types, `IndexMap<K, V>` and `IndexSet<T>`: hash-indexed collections whose iteration order is insertion order, so it is a pure function of the operations applied and never depends on hash values, seeds, capacity, platform or crate versions. It comes now because `clippy.toml` bans std's `HashMap` and `HashSet` (their iteration order is randomized per process, [ADR 0003](../../../adr/0003-same-build-determinism.md)), which leaves engine code only `BTreeMap`; the M3 storage prototypes need hashed lookups (`TypeId` to component id, component set to archetype, archetype edges) on the paths M3 measures, and the bevy_ecs and hecs baselines they are compared against use hashed maps there. At the end, `use engine::collections::IndexMap;` works in engine code, and the model tests show that iteration order is identical with the real hasher and with a hasher that maps every key to the same value.

## 2. Goals and non-goals

**Goals**

- A deterministic map and set for engine code, with lookup speed comparable to hashbrown-based maps, so M3.2 to M3.7 measure storage models and not the map.
- A contract in which the hasher is unobservable: the hasher and its seed can change in any later chunk without changing any program's output.
- Removal that names its effect on order (`swap_remove` or `shift_remove`), because order is the part that matters for determinism.
- Equality and `Hash` that respect order, so equal collections behave identically.
- A tool to put entries into a canonical order (`sort_keys`, `sort`) before hashing or saving.
- Heap behaviour in line with [ADR 0011](../../../adr/0011-heap-allocation-control.md): capacity reserved up front is used without further allocation.
- Clippy bans for the randomly seeded items of every hashing crate this chunk adds, checked to actually fire.

**Non-goals**

- No implementation of a hash table. The backend is the `indexmap` crate; this chunk owns the API and its contract, not the probing.
- No resistance to hash flooding (§4.6). Nothing in M3 or M4 hashes untrusted keys.
- No set algebra (`union`, `intersection`, ...), custom-order sorting (`sort_by`), `drain`, `splice`, `get_disjoint_mut`, or `Occupied` and `Vacant` entry types. Each is added when a prototype or later chunk needs it (§10).
- No serde or reflection support (M5), no debugger visualizers (M4), no allocator type parameter (ADR 0011).
- No public `BuildHasher` for other hashing needs.
- No new ADR, no frozen format.

## 3. Guide-level explanation

A map remembers the order keys were first inserted in, and iterates in that order:

```rust
use engine_collections::IndexMap;

let mut speeds = IndexMap::new();
speeds.insert("walk", 1.4);
speeds.insert("run", 4.0);
speeds.insert("crawl", 0.3);
speeds.insert("walk", 1.5); // existing key: value replaced, position kept

assert_eq!(speeds.keys().copied().collect::<Vec<_>>(), ["walk", "run", "crawl"]);
assert_eq!(speeds["walk"], 1.5);
assert_eq!(speeds.get_index_of("run"), Some(1));
```

There is no `remove`. Each removal says what it does to the order:

```rust
use engine_collections::IndexMap;

let mut m: IndexMap<char, u32> = ('a'..='e').zip(0..).collect();

m.swap_remove(&'b');  // O(1): the last entry ('e') moves into the hole
assert_eq!(m.keys().collect::<String>(), "aecd");

m.shift_remove(&'a'); // O(n): later entries move up one, order kept
assert_eq!(m.keys().collect::<String>(), "ecd");
```

Two collections are equal only if they hold the same entries **in the same order**, because order changes what a simulation does with them:

```rust
use engine_collections::IndexSet;

let forward: IndexSet<u32> = [1, 2].into();
let backward: IndexSet<u32> = [2, 1].into();
assert_ne!(forward, backward);

// To use a set as an order-independent key (for example, a set of component ids
// naming an archetype), sort it first.
let (mut forward, mut backward) = (forward, backward);
forward.sort();
backward.sort();
assert_eq!(forward, backward);
```

The order is deterministic only if the sequence of operations is. Inserting from several threads in completion order gives a different order from run to run, and no hasher can fix that; merge such work in a fixed order first (M0+M1 spec §4.2).

## 4. Reference-level design

### 4.1 Crate

`libs/engine_collections` is a workspace member with `#![no_std]`, `extern crate alloc`, and `#![forbid(unsafe_code)]`. Its dependencies are:

| Crate | Version | Features | Licence |
|---|---|---|---|
| `indexmap` | 2 (2.14.2 is already in the lockfile, through `winit`) | `default-features = false` | MIT OR Apache-2.0 |
| `foldhash` | 0.2 | `default-features = false` | Zlib |

Its crate documentation MUST open with its [ADR 0004](../../../adr/0004-crate-splitting-rule.md) justification: dependencies (`indexmap` and `foldhash`, which other crates should not carry), platform (`no_std` with `alloc`), and layering (a foundation for the ECS, the registry and persistence). The same justification MUST be added to ADR 0004's table.

The `engine` facade MUST re-export the crate as `engine::collections`. `IndexMap` and `IndexSet` MUST NOT be in `engine::prelude`: a prelude `IndexMap` would clash with the `indexmap` crate's name in code that uses both.

Layout: `IndexMap` and its iterator and entry types live in the public module `index_map`; `IndexSet` and its iterators in `index_set`; `TryReserveError` at the root. `IndexMap` and `IndexSet` are re-exported at the crate root. This mirrors `std::collections::hash_map`.

### 4.2 Representation and hasher

```rust
pub struct IndexMap<K, V> { inner: indexmap::IndexMap<K, V, Hasher> }
pub struct IndexSet<T>    { inner: indexmap::IndexSet<T, Hasher> }
```

- `Hasher` is private. It is `foldhash::fast::FixedState` built with `FixedState::with_seed(SEED)`, where `SEED` is a private constant. The crate documentation MUST say that the algorithm and the seed are not part of the contract and may change in any release.
- There is no hasher type parameter in the public API. Internally the implementation MAY be generic over the hasher (and MUST be, to the extent the tests in §8 need to substitute one).
- No public item's signature may name an `indexmap` or `foldhash` type. Iterators, the entry type and the error type are this crate's own.
- `new` SHOULD be a `const fn`.

### 4.3 Order semantics

These rules define the iteration order of both types. The set follows the map rules with `()` values.

1. **Insertion of a new key** appends it at the end.
2. **Insertion of an existing key** into a map replaces the value and returns the old one; the entry keeps its position and its original key object. Into a set, it changes nothing and returns `false`.
3. **`swap_remove` and `swap_remove_index`** run in O(1): the last entry moves into the removed entry's position, and every other entry keeps its position.
4. **`shift_remove` and `shift_remove_index`** run in O(n): every entry after the removed one moves up one position, and relative order is kept.
5. **`pop`** removes the last entry. **`retain`** keeps the surviving entries in their relative order. **`clear`** removes every entry and keeps the capacity.
6. **`entry(key)`** followed by `or_insert`, `or_insert_with` or `or_default` follows rule 1 when the key is absent, and changes nothing when it is present.
7. **Construction from several entries** (`FromIterator`, `Extend`, `From<[_; N]>`) inserts them one by one in iterator order, so a repeated key keeps its first position and, in a map, the last value.
8. **`sort_keys`** (map) and **`sort`** (set) reorder entries by `K: Ord`. Keys are unique, so the result is fully determined; it is computed with an unstable sort, which does not allocate.
9. Nothing else changes the order. In particular, `reserve`, `try_reserve`, `shrink_to_fit`, growth during insertion, lookups, `get_mut`, `values_mut` and `iter_mut` do not.

Consequences, which the crate documentation MUST state:

- Iteration order is a pure function of the sequence of operations. It does not depend on hash values, the seed, capacity, pointer width, platform, or the versions of `indexmap` and `foldhash`.
- It also does not depend on the values of the keys. Keys whose values differ between runs or builds (pointer addresses under ASLR, `TypeId` across builds) still iterate in a deterministic order. `BTreeMap` does not have this property.
- Positions are not stable. `swap_remove` and `shift_remove` change the positions of other entries; a position from `get_index_of` is valid only until the next removal or sort.
- Determinism requires a deterministic operation sequence; the caller is responsible for that (§3).

### 4.4 `IndexMap<K, V>` API

Methods that look up a key take `&Q` where `K: Borrow<Q>` and `Q: Hash + Eq + ?Sized`, as std's `HashMap` does. Methods that hash require `K: Hash + Eq`.

| Group | Items |
|---|---|
| Construction | `new`, `with_capacity(n)`, `Default`, `FromIterator<(K, V)>`, `Extend<(K, V)>`, `From<[(K, V); N]>` |
| Size and capacity | `len`, `is_empty`, `capacity`, `reserve(n)`, `try_reserve(n) -> Result<(), TryReserveError>`, `shrink_to_fit`, `clear` |
| Lookup by key | `get(&Q) -> Option<&V>`, `get_mut(&Q) -> Option<&mut V>`, `contains_key(&Q) -> bool`, `get_index_of(&Q) -> Option<usize>`, `Index<&Q>`, `IndexMut<&Q>` |
| Lookup by position | `get_index(usize) -> Option<(&K, &V)>`, `get_index_mut(usize) -> Option<(&K, &mut V)>`, `first`, `first_mut`, `last`, `last_mut` |
| Insertion | `insert(K, V) -> Option<V>`, `entry(K) -> Entry<'_, K, V>` |
| Removal | `swap_remove(&Q) -> Option<V>`, `shift_remove(&Q) -> Option<V>`, `swap_remove_index(usize) -> Option<(K, V)>`, `shift_remove_index(usize) -> Option<(K, V)>`, `pop() -> Option<(K, V)>`, `retain(FnMut(&K, &mut V) -> bool)` |
| Order | `sort_keys()` where `K: Ord` |
| Iteration | `iter`, `iter_mut`, `keys`, `values`, `values_mut`, `into_keys`, `into_values`; `IntoIterator` for `IndexMap`, `&IndexMap` and `&mut IndexMap` |
| Traits | `Clone`, `Debug`, `Default`, `PartialEq`, `Eq`, `Hash` (§4.5) |

- No method gives mutable access to a key, so a key cannot change while it is in the map.
- There is no `remove`.
- `Entry<'_, K, V>` is an opaque struct with `key() -> &K`, `index() -> usize` (the position the entry has, or would have once inserted), `or_insert(V) -> &mut V`, `or_insert_with(FnOnce() -> V) -> &mut V`, `or_default() -> &mut V` where `V: Default`, and `and_modify(FnOnce(&mut V)) -> Self`.
- Iterator types live in `index_map`: `Iter`, `IterMut`, `Keys`, `Values`, `ValuesMut`, `IntoIter`, `IntoKeys`, `IntoValues`. Each implements `Iterator`, `DoubleEndedIterator`, `ExactSizeIterator`, `FusedIterator` and `Debug`; `Iter`, `Keys` and `Values` also implement `Clone`.

### 4.5 `IndexSet<T>` API

| Group | Items |
|---|---|
| Construction | `new`, `with_capacity(n)`, `Default`, `FromIterator<T>`, `Extend<T>`, `From<[T; N]>` |
| Size and capacity | `len`, `is_empty`, `capacity`, `reserve(n)`, `try_reserve(n)`, `shrink_to_fit`, `clear` |
| Lookup | `contains(&Q) -> bool`, `get(&Q) -> Option<&T>`, `get_index_of(&Q) -> Option<usize>`, `get_index(usize) -> Option<&T>`, `first`, `last` |
| Insertion | `insert(T) -> bool` |
| Removal | `swap_remove(&Q) -> bool`, `shift_remove(&Q) -> bool`, `swap_remove_index(usize) -> Option<T>`, `shift_remove_index(usize) -> Option<T>`, `pop() -> Option<T>`, `retain(FnMut(&T) -> bool)` |
| Order | `sort()` where `T: Ord` |
| Iteration | `iter`; `IntoIterator` for `IndexSet` and `&IndexSet` |
| Traits | `Clone`, `Debug`, `Default`, `PartialEq`, `Eq`, `Hash` |

Iterator types live in `index_set`: `Iter` (also `Clone`) and `IntoIter`, with the same iterator traits as the map's.

### 4.6 Equality, hashing, formatting and auto traits

- **Equality.** `a == b` MUST hold exactly when `a.len() == b.len()` and the entries compare equal pairwise in iteration order. For maps this requires `K: PartialEq, V: PartialEq`; `Eq` is implemented when both are `Eq`. This differs from `indexmap`, std and `BTreeMap`, which ignore order; the type documentation MUST say so and MUST warn that a set used as an order-independent key has to be sorted first (§3).
- **Hashing.** `Hash` writes the length with `write_usize`, then each entry in iteration order (key then value for maps). So `a == b` implies equal hashes.
- **`Debug`** prints `{k: v, ...}` for maps and `{t, ...}` for sets, in iteration order.
- **Auto traits.** `IndexMap<K, V>` and `IndexSet<T>` MUST be `Send`, `Sync` and `Unpin` whenever their element types are.
- **Inconsistent impls.** If `Hash`, `Eq` or `Ord` on a key is inconsistent or panics, results are unspecified but memory-safe, as with std's collections. This is documented, not tested.
- **Hash flooding.** The seed is fixed and public, so keys chosen by an attacker could make every lookup O(n). The crate documentation MUST state this. The deterministic remedy, a per-world seed drawn from the world's `Prng`, is deferred (§10).

### 4.7 Heap allocation and panics

- After `with_capacity(n)` or `reserve(n)`, the next `n` insertions of new keys MUST NOT allocate.
- Lookups, iteration, `get_mut`, all removals, `pop`, `retain`, `clear` and `sort_keys`/`sort` MUST NOT allocate.
- `reserve` panics if the new capacity overflows or the allocator fails, exactly like `Vec::reserve`. It MUST be `#[track_caller]`, document this in `# Panics`, and name `try_reserve` as the fallible twin. `TryReserveError` is this crate's own opaque type, implementing `Debug`, `Display`, `Clone`, `PartialEq`, `Eq` and `core::error::Error`.
- Growth during `insert`, `entry(..).or_insert*`, `Extend` and `FromIterator` allocates like `Vec::push`. As for `HandlePool::insert`, allocation failure is not a documented panic; callers that must not allocate call `try_reserve` first.
- `Index` and `IndexMut` on a missing key MUST panic through a `#[track_caller]` path, with a `# Panics` section naming `get` and `get_mut` as the twins, and a message that says the key was not in the map and suggests `get` (for example: *IndexMap: no entry for this key; use get to handle a missing key*). The key is not printed: `K: Debug` is not required.
- Nothing else panics. A position out of range returns `None`.

### 4.8 Lints, tooling and documents

**`clippy.toml`.** The two std ban messages change to "iteration order is randomized per process; use `engine_collections::IndexMap`" (and `IndexSet`). The following are added; `engine_collections` uses item-level `#[expect(clippy::disallowed_types, reason = "...")]` where it names them:

| Kind | Path | Reason |
|---|---|---|
| type | `indexmap::IndexMap`, `indexmap::IndexSet` | with `indexmap`'s `std` feature (enabled in the workspace by other dependencies), their default hasher is std's `RandomState`, which the `RandomState` ban does not catch through a defaulted type parameter; use `engine_collections` |
| type | `foldhash::fast::RandomState`, `foldhash::quality::RandomState`, `foldhash::HashMap`, `foldhash::HashSet` | randomly seeded |
| method | `foldhash::fast::SeedableRandomState::random`, `foldhash::quality::SeedableRandomState::random`, `foldhash::SharedSeed::global_random` | randomly seeded |

Clippy matches resolved definition paths, so a path written through a re-export may be silently inert. Each entry MUST be checked to fire by triggering it once in a scratch crate; where the re-export path does not fire, the definition path (for example `indexmap::map::IndexMap`) is used instead.

**xtask.** `engine_collections` is added to the `no_std` leaf list, to the `wasm32-wasip1` test list, and to `TRIAGED` in `xtask/src/mutants.rs`.

**Benchmarks.**

- `bench-regress/benches/collections.rs` (Gungraun, gated at 5% like the existing benchmarks): inserting 1,024 `u64` keys into a map made `with_capacity(1024)`, hit and miss lookups in that map, iterating it, and hit lookups with `TypeId` keys in a map of 64 entries.
- `benches/benches/collections.rs` (wall clock, report only): the same operations on `engine_collections::IndexMap`, raw `indexmap::IndexMap` with the same hasher, `indexmap::IndexMap` with `BuildHasherDefault<DefaultHasher>`, and `BTreeMap`. It is for the numbers in the pull request, not a gate.

**Documents.**

- ADR 0004: a table row for `engine_collections`.
- ADR 0011: the Consequences line "Other crates do not allocate, so they need none of this" is replaced by one saying that `engine_collections` also allocates and follows the same capacity controls, without a fixed-capacity mode.
- M0+M1 spec §4.2, "Deterministic hash collections": a note that the fixed-seed design was replaced by insertion order in M3.1, linking this spec. The text above the note stays as a record.
- `docs/mutation-triage.md`: a section for `engine_collections`.
- `docs/roadmap.md`: the M3.1 row, and on M5's rough-chunk list, "maps and sets are saved in iteration order and loaded by inserting in that order (order is state)".
- `README.md` lists the crate; `CHANGELOG.md` records it.

## 5. Requirements

- **M3.1-R1** `libs/engine_collections` MUST exist as in §4.1: a workspace member with `#![no_std]` and `#![forbid(unsafe_code)]`, depending only on `indexmap` and `foldhash` with default features off.
- **M3.1-R2** The public API MUST provide the items of §4.4 and §4.5 with the stated signatures and trait impls, and no public signature may name an `indexmap` or `foldhash` type or a hasher type parameter.
- **M3.1-R3** Both types MUST follow the order rules 1 to 9 of §4.3.
- **M3.1-R4** No observable behaviour may depend on the hasher: every test of R3 MUST also pass with a hasher that returns the same value for every key.
- **M3.1-R5** A fixed scripted sequence of operations MUST produce a golden iteration order written out literally in the test.
- **M3.1-R6** `PartialEq`, `Eq` and `Hash` MUST respect order as in §4.6, and `a == b` MUST imply `hash(a) == hash(b)`.
- **M3.1-R7** `Debug` output MUST follow §4.6, and every public type MUST implement `Debug`.
- **M3.1-R8** Both types MUST be `Send`, `Sync` and `Unpin` whenever their element types are.
- **M3.1-R9** Heap allocation MUST follow §4.7.
- **M3.1-R10** `Index` and `IndexMut` MUST panic on a missing key at the caller's location with the message of §4.7; `reserve` MUST document its panic and name `try_reserve`; no other public item may panic, apart from allocation failure during growth (§4.7).
- **M3.1-R11** Every public item MUST be documented, every public type MUST have a runnable doctest, and the crate documentation MUST state the ADR 0004 justification and the consequences listed in §4.3 and §4.6 (order is a function of the operation sequence and not of key values, positions are not stable, the caller owns the operation sequence, equality respects order, the hasher is not part of the contract, and the hash-flooding limitation).
- **M3.1-R12** `engine::collections` MUST re-export the crate, and `engine::prelude` MUST NOT contain `IndexMap` or `IndexSet`.
- **M3.1-R13** `clippy.toml` MUST contain the changes of §4.8, and each new ban MUST be shown to fire.
- **M3.1-R14** The crate MUST build in `cargo xtask no-std`, and its tests MUST pass in `cargo xtask wasm`.
- **M3.1-R15** The two benchmark files of §4.8 MUST exist, and `cargo xtask bench-regress` MUST pass.
- **M3.1-R16** `cargo xtask mutants -p engine_collections` MUST have been run and every missed mutant and timeout triaged in `docs/mutation-triage.md`.
- **M3.1-R17** The document changes of §4.8 MUST be made.
- **M3.1-R18** `cargo xtask deny` MUST pass without changes to `deny.toml`, and `cargo xtask ci` MUST pass.

## 6. Cross-cutting concerns

- **Determinism:** the reason this crate exists. Iteration order depends only on the operation sequence (R3), proved independent of the hasher (R4) and pinned by a golden test (R5). Hash values never reach any output. Callers remain responsible for a deterministic operation sequence (§3, §4.3).
- **`unsafe`:** none written. `indexmap` and its dependency `hashbrown` use `unsafe` internally; like `glam`, they are third-party dependencies outside `unsafe-allowlist.toml`'s scope, which covers workspace crates.
- **Concurrency:** n/a, because the types have no interior mutability; they are `Send` and `Sync` when their contents are (R8), and sharing follows ordinary borrowing.
- **Heap allocation:** §4.7. Checked with `dhat` (R9), and ADR 0011's Consequences are updated (§4.8).
- **`no_std` and wasm:** `no_std` with `alloc`; both dependencies support it with default features off (R14). `foldhash` produces different hash values on 32-bit and 64-bit targets, which is unobservable (R4).
- **Frozen formats:** none. The seed and hash algorithm are explicitly not part of the contract; the golden order test pins behaviour, not a format.
- **Public API changes:** a new crate and a new `engine::collections` module; no existing item changes.
- **FFI exposure:** n/a, because the collections are internal building blocks; nothing crosses the C ABI in M3.
- **Performance:** `cargo xtask bench-regress` gates the new Gungraun benchmarks; the wall-clock comparison in `benches` reports the wrapper's cost against raw `indexmap` (expected to be within noise), the hasher choice against `DefaultHasher`, and the gap to `BTreeMap`, with a saved baseline and the noise floor in the pull request (`cargo xtask bench-report`).
- **Documentation:** item docs and doctests (R11), the crate's ADR 0004 justification, this spec and its plan, and the document changes of §4.8.
- **Policy changes:** `clippy.toml` gains the bans of §4.8, as the M0+M1 spec §4.2 requires for any added hashing crate. ADR 0011's Consequences gain one sentence. No policy is relaxed.

## 7. Alternatives considered

- **Hashed iteration order with a fixed seed** (what the M0+M1 spec planned). Rejected: order is deterministic within a build but depends on hash values and on the insert, remove and growth history, so the hasher and the table layout become a frozen format, and a dependency update could change golden values.
- **Lookup only, with no iteration.** Rejected: registries, debug output and persistence all need to iterate, and would have to keep a second structure.
- **`BTreeMap` only, deferring this crate** until a profile shows the map matters. Rejected: the baselines use hashed maps, so the M3 comparison would partly measure the map; prototypes would each work around it, and swapping later would change measured code.
- **Our own hash table in safe Rust.** Rejected for now: high learning value, but about twice the code plus fuzzing, and a slower map would skew M3. The newtype keeps it possible later, measured against the benchmarks of §4.8.
- **Our own ordering over `hashbrown::HashTable`.** Rejected: more code than wrapping `indexmap`, and still a dependency with `unsafe` inside.
- **Other hashers.** `rustc-hash` (fastest on small integer keys, weaker on strings and aligned pointers), a hasher of our own (more to verify, little benefit when the hasher is unobservable), and std's `DefaultHasher` with zero keys (no dependency, but slowest). `foldhash` with a fixed seed was chosen: it is what `hashbrown` defaults to, small, and fast on integers and `TypeId`.
- **A public hasher type parameter.** Rejected: it would make the hasher part of the API and invite `RandomState`; it can be added later if a measured need appears.
- **Other names.** `HashMap`/`HashSet` (readers would assume std's semantics), `Map`/`Set` (generic), `OrderedMap` (reads as "sorted"), `KeyedVec`/`UniqueVec` (unfamiliar), `StableMap` (suggests stable addresses), `DetMap` (abbreviated, says nothing about the order). `IndexMap`/`IndexSet` is the Rust ecosystem's name for exactly this behaviour; inside the crate the backend is imported under another name.
- **Order-insensitive equality** (as `indexmap`, std and `BTreeMap` do). Rejected: two "equal" maps could drive a simulation differently. **No `PartialEq` at all** was rejected because it blocks `derive(PartialEq)` on structs that contain a map.
- **A bare `remove`.** Rejected: it hides whether the order changes. `indexmap` itself deprecated it for this reason.
- **Both `sort_keys` and `sort_unstable_keys`.** Rejected: keys are unique, so both give the same order; one method using the non-allocating unstable sort is enough.
- **`IndexMap` in `engine::prelude`.** Rejected: it would clash with `indexmap::IndexMap` in code that imports both.
- **A fuzz target.** Rejected: the crate is a thin wrapper, and the proptest model (R3, R4) already drives random operation sequences against a reference.
- **A `check_invariants` feature** like `engine_handle`'s. Rejected: the internal invariants belong to `indexmap`; the model test checks everything visible through this API.

## 8. Verification

| Requirement | Verified by |
|---|---|
| R1 | `Cargo.toml` of the crate; `cargo xtask check-unsafe`; `cargo tree -p engine_collections -e normal --depth 1` |
| R2 | doctests and unit tests exercising every item; review that `cargo doc` shows no `indexmap` or `foldhash` type in any public signature |
| R3 | `tests/model.rs`: proptest of random operation sequences (insert of new and existing keys, every entry method, the four removals, `pop`, `retain`, `clear`, `reserve`, `shrink_to_fit`, `sort_keys`/`sort`) against a model `Vec<(K, V)>` searched linearly; after every operation, the iteration order, `len`, every key's lookup and `get_index_of`, and `first`/`last` must match the model. Same for `IndexSet`. Unit tests for `FromIterator`, `Extend` and `From` with repeated keys |
| R4 | the same model tests instantiated with a private constant hasher (every key hashes to 0), in a unit-test module inside the crate |
| R5 | unit test: a fixed sequence of about 20 operations, iteration order compared with a literal list |
| R6 | proptests: equality is reflexive and symmetric; equal collections hash equally; a permutation of distinct entries compares unequal and becomes equal after `sort_keys`/`sort` |
| R7 | unit tests of the `Debug` output; the `missing_debug_implementations` lint |
| R8 | compile-time assertions in a unit test (`fn assert_send_sync_unpin<T: Send + Sync + Unpin>()`) for `IndexMap<u32, u32>`, `IndexSet<u32>` and every iterator |
| R9 | `tests/allocations.rs` (`dhat`): after `with_capacity(n)`, `n` insertions, lookups, iteration, every removal, `retain`, `sort_keys`, `pop` and `clear` allocate zero blocks; likewise after `reserve(n)` |
| R10 | `#[should_panic(expected = ...)]` tests for `Index` and `IndexMut`; a test reading the panic location through a panic hook to confirm it is the caller's line; review of `# Panics` sections |
| R11 | documentation review (CONTRIBUTING.md "Reviews before a pull request"); `cargo xtask doc` |
| R12 | a doctest in `engine` using `engine::collections::IndexMap`; review of `engine::prelude` |
| R13 | `clippy.toml` diff; each new ban triggered once in a scratch crate, with the clippy output pasted in the pull request |
| R14 | `cargo xtask no-std`; `cargo xtask wasm` |
| R15 | `cargo xtask bench-regress` in the devcontainer; `cargo xtask bench-report` output with a saved baseline and the noise floor in the pull request |
| R16 | `docs/mutation-triage.md` section; the `cargo xtask mutants -p engine_collections` summary in the pull request |
| R17 | documentation review; relative links resolve |
| R18 | `cargo xtask deny`; `cargo xtask ci` locally; the pull request's CI run |

## 9. Done when

- [x] `cargo xtask ci` passes locally and CI is green on all three platforms.
- [x] The model tests pass with the real hasher and with the constant hasher.
- [x] `cargo xtask bench-regress` passes (in CI; no devcontainer run was made), and the wall-clock comparison is in the pull request with its baseline and noise floor.
- [x] Every new clippy ban is shown to fire.
- [x] The mutation triage is recorded.
- [x] The eight reviews in CONTRIBUTING.md are recorded in the pull request.
- [x] The spec is `Implemented` and the plan `Historical`, with Deviations filled in.

## 10. Open questions

None blocking. Deferred, each to a named place:

- **Hash flooding** (§4.6): a per-world seed drawn from the world's `Prng`. Deferred to M8 (mods), or to the first earlier chunk that hashes keys from untrusted input.
- **Set algebra, `sort_by`, `drain`, `splice`, `get_disjoint_mut`, and `Occupied`/`Vacant` entry types:** added by the M3.3 to M3.6 chunk that first needs one, or in M4.
- **Serialization and reflection** of the collections: M5, which saves them in iteration order (§4.8).
- **Debugger visualizers:** M4, with the storage types.
- **An allocator type parameter:** revisited under ADR 0011's conditions.

## 11. Prior art and sources

- [`indexmap` 2.14 documentation](https://docs.rs/indexmap/2.14.2/indexmap/): the entries-vector-plus-hash-index design, the `swap_remove` and `shift_remove` split, and the deprecation of `remove`.
- Raymond Hettinger, [More compact dictionaries with faster iteration](https://mail.python.org/pipermail/python-dev/2012-December/123028.html) (python-dev, 2012): the dense-entries design that made Python's `dict` insertion-ordered.
- [ECMAScript `Map` objects](https://tc39.es/ecma262/#sec-map-objects): insertion-ordered iteration as a language-level guarantee.
- Orson Peters, [foldhash](https://github.com/orlp/foldhash): the hasher, its fixed and random states, and its stated non-goal of resistance to hash flooding.
- [std `HashMap` documentation](https://doc.rust-lang.org/std/collections/struct.HashMap.html): why the default hasher is randomly seeded (hash flooding), which is why the std types are banned here.
