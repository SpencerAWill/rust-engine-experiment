# M3.1 plan: `engine_collections`: deterministic map and set

- **Status:** Historical: executed in PR #26 (2026-11-13); the spec and the code are authoritative. Deviations are in §4.
- **Spec:** [spec.md](spec.md). This plan argues from the spec; read both.

## 1. Approach

Eight commits on branch `m3.1-collections`, after the spec commit already on it. The crate skeleton and the clippy bans come first, so every later commit is linted against the final ban list. The map is built in two commits: first the operations that define order, with the model test that proves the order rules (§4.3) with both hashers; then the iterators and trait impls. The set follows the map's pattern in one commit. Allocation and platform checks, benchmarks, mutation triage and documents close the chunk.

Choices the spec leaves to implementation:

- **How the tests swap the hasher (R4).** `src/hasher.rs` defines `pub(crate) type Hasher`. In normal builds it is `foldhash::fast::FixedState`. Under `#[cfg(test)]` it is a private enum `TestHasher { Fold(FixedState), Constant }` implementing `BuildHasher`, whose `Constant` variant builds a hasher that ignores its input and returns 0 from `finish`. `IndexMap` and `IndexSet` get a `#[cfg(test)] pub(crate) fn with_test_hasher(hasher: Hasher) -> Self`. So the public types have no hasher parameter, non-test builds contain no test code, and the unit-test model runs against the real public type with both hashers. Integration tests (`tests/`) build the library without `cfg(test)` and always use the real hasher.
- **Where the model test lives.** Spec §8 names `tests/model.rs` for R3 and a unit module for R4. One unit module, `src/tests/map_model.rs` (and `set_model.rs`), serves both, because the constant hasher is only reachable from inside the crate. Recorded as a deviation in §4.
- **`SEED`** is `0x5EED_C011_EC71_0A5D` ("seed collection", arbitrary). It is private and not part of the contract.
- **Module layout.** `src/lib.rs` (crate docs, re-exports, `#[cfg(test)] mod tests;`), `src/hasher.rs`, `src/error.rs`, `src/index_map.rs` with `src/index_map/{iter.rs, entry.rs}`, `src/index_set.rs` with `src/index_set/iter.rs`, `src/tests/{mod.rs, map_model.rs, set_model.rs, golden.rs}`. The backend is imported as `use indexmap::IndexMap as Inner;` (and `IndexSet as InnerSet`), each with `#[expect(clippy::disallowed_types, reason = "the one sanctioned use: wrapped with a fixed hasher")]` on the smallest item that names it.
- **Delegation.** Every method is a one-line call into the `indexmap` method of the same meaning: `sort_keys` calls `sort_unstable_keys`, the set's `sort` calls `sort_unstable`, and `Entry` wraps `indexmap::map::Entry`. Lookups take `&Q` where `K: Borrow<Q>, Q: Hash + Eq + ?Sized`, which satisfies `indexmap`'s blanket `Equivalent` impl, so `Equivalent` never appears in our signatures.
- **Checking the bans fire (R13)** uses a throwaway crate in the session scratchpad, outside the repository: it depends on `indexmap` (default features, so `std` is on) and `foldhash` (default features), names each banned item once, and runs `CLIPPY_CONF_DIR=<repo root> cargo clippy`. Nothing from it is committed; its output goes into the pull request.

**Global constraints**

- Every commit passes `cargo xtask ci --quick`; Steps 1 and 6 also pass `cargo xtask deny`, with `deny.toml` unchanged (R18).
- Dependencies: `indexmap = { version = "2.14", default-features = false }` and `foldhash = { version = "0.2", default-features = false }`, declared in `[workspace.dependencies]` and used by `engine_collections` only (benchmarks may enable features in their own dev-dependencies).
- `engine_collections` is `#![no_std]` with `extern crate alloc` and `#![forbid(unsafe_code)]`. No public signature names an `indexmap` or `foldhash` type, and there is no public hasher type parameter (R2).
- There is no `remove`, no mutable key access, and no `Index<usize>`.
- Commit messages are conventional, end with ` (M3.1)`, and end with the session's attribution line. Stage by explicit path.
- Stop and ask the maintainer before the first push and before opening the pull request.

**Review focus** (inputs the spec implies but no requirement's test pins on its own; each has a test in the step that owns the code)

1. **Empty collections.** `pop`, `first`, `last`, `swap_remove_index(0)`, `shift_remove_index(0)`, `sort_keys` and `retain` on an empty map return `None` or do nothing, without panicking; `Debug` prints `{}`. Step 2: `empty_map_operations_are_noops`; Step 4: the same for the set.
2. **Lookup through `Borrow`.** An `IndexMap<String, u32>` is looked up with `&str` by `get`, `contains_key`, `get_index_of`, `swap_remove` and `map["key"]`. Step 2: `string_keys_are_looked_up_by_str`; Step 3: the `Index` case.
3. **Removing the last entry by swapping.** `swap_remove` of the last entry (nothing moves) and of the only entry leave the others in place. The model covers it statistically; Step 2 pins it: `swap_remove_of_the_last_entry_moves_nothing`.
4. **`Entry::index`** is `len()` for a vacant entry, and `or_insert` puts the key exactly there. Step 2: `vacant_entry_index_is_len`.
5. **`try_reserve(usize::MAX)`** returns `Err` and leaves the map usable, on 64-bit and on 32-bit wasm. Step 1: `try_reserve_overflow_is_an_error`, which also runs under `cargo xtask wasm` from Step 5.

## 2. Steps

### Step 1: crate skeleton, hasher, error and clippy bans (R1, R13, R18)

- **Files:** `Cargo.toml` (workspace dependencies `indexmap`, `foldhash`, and `engine_collections = { path = "libs/engine_collections", version = "0.1.0" }`), `libs/engine_collections/Cargo.toml` (new: description "Deterministic insertion-ordered hash map and set", keywords `["collections", "deterministic", "gamedev", "no-std"]`, categories `["data-structures", "no-std"]`, dev-dependency `proptest = { workspace = true }`, `[lints] workspace = true`), `src/lib.rs`, `src/hasher.rs`, `src/error.rs`, `src/index_map.rs` (new), `clippy.toml`, `Cargo.lock`.
- **Interfaces produced:**
  - `pub(crate) type Hasher` and `pub(crate) const fn default_hasher() -> Hasher` (`FixedState::with_seed(SEED)`, or `TestHasher::Fold(..)` under `cfg(test)`); `#[cfg(test)] pub(crate) const fn constant_hasher() -> Hasher`.
  - `pub struct TryReserveError` (opaque, private unit field; derives `Clone, Debug, PartialEq, Eq`; `Display`: "memory allocation failed: the capacity overflowed or the allocator reported an error"; `core::error::Error`; `pub(crate) fn from_inner(indexmap::TryReserveError) -> Self`).
  - `pub struct IndexMap<K, V> { inner: Inner<K, V, Hasher> }` with `pub const fn new() -> Self`, `pub fn with_capacity(n: usize) -> Self`, `len`, `is_empty`, `capacity`, `reserve(&mut self, additional: usize)` (`#[track_caller]`, `# Panics` naming `try_reserve`, wording as `HandlePool::reserve`), `try_reserve(&mut self, additional: usize) -> Result<(), TryReserveError>`, `shrink_to_fit`, `clear`; `#[cfg(test)] pub(crate) fn with_test_hasher(hasher: Hasher) -> Self`.
  - Crate docs: the ADR 0004 justification paragraph (spec §4.1); the rest of the crate docs grows in later steps.
- **Test first** (inline `#[cfg(test)]` modules):
  - `new_map_is_empty`: `len() == 0`, `is_empty()`, `capacity() == 0`.
  - `with_capacity_reserves`: `IndexMap::<u32, u32>::with_capacity(10).capacity() >= 10`; after `reserve(100)`, `capacity() >= 100`.
  - `try_reserve_overflow_is_an_error` (review focus 5): `try_reserve(usize::MAX)` is `Err(e)` with a non-empty `e.to_string()`, and afterwards `try_reserve(4)` is `Ok` and `len() == 0`.
  - `default_hasher_is_fixed`: two `default_hasher()` values give the same `hash_one(42_u64)`; `constant_hasher().hash_one("a") == constant_hasher().hash_one(7_u64) == 0`.
  - Run `cargo xtask test`: compile failure (the crate's items do not exist).
- **Change:** implement the items above. `clippy.toml`: the std `HashMap` and `HashSet` reasons become "iteration order is randomized per process; use engine_collections::IndexMap" (and `IndexSet`), and the entries of spec §4.8 are added. Write each new path as the spec gives it first.
- **Verify:**
  - `cargo xtask test` passes; `cargo xtask ci --quick` passes; `cargo xtask deny` passes with `deny.toml` unchanged.
  - Ban check: in the scratch crate described in §1, `CLIPPY_CONF_DIR=<repo root> cargo clippy` reports one `disallowed_types` or `disallowed_methods` warning per new entry (9 items). For any entry that does not fire, replace it with the definition path (for example `indexmap::map::IndexMap`, `foldhash::convenience::HashMap`) and rerun until all 9 fire. `foldhash::HashMap` and `HashSet` are type aliases; if no path makes an alias fire, record it in §4 and tell the maintainer (the `RandomState` ban inside the alias may still catch it). Keep the final output for the pull request (R13).
  - `cargo tree -p engine_collections -e normal --depth 1` lists only `indexmap` and `foldhash`.
- **Commit:** `feat(collections): add engine_collections skeleton and hashing bans (M3.1)`

### Step 2: map operations, the order model and the golden order (R3, R4, R5)

- **Files:** `src/index_map.rs`, `src/index_map/entry.rs`, `src/index_map/iter.rs` (new), `src/tests/mod.rs`, `src/tests/map_model.rs`, `src/tests/golden.rs` (new).
- **Interfaces produced** (on `IndexMap<K, V>`; `Q` lookups as in §1):
  - `insert(&mut self, key: K, value: V) -> Option<V>`; `get(&self, &Q) -> Option<&V>`; `get_mut(&mut self, &Q) -> Option<&mut V>`; `contains_key(&self, &Q) -> bool`; `get_index_of(&self, &Q) -> Option<usize>`.
  - `get_index(&self, usize) -> Option<(&K, &V)>`; `get_index_mut(&mut self, usize) -> Option<(&K, &mut V)>`; `first`, `last` -> `Option<(&K, &V)>`; `first_mut`, `last_mut` -> `Option<(&K, &mut V)>`.
  - `swap_remove(&mut self, &Q) -> Option<V>`; `shift_remove(&mut self, &Q) -> Option<V>`; `swap_remove_index(&mut self, usize) -> Option<(K, V)>`; `shift_remove_index(&mut self, usize) -> Option<(K, V)>`; `pop(&mut self) -> Option<(K, V)>`; `retain(&mut self, impl FnMut(&K, &mut V) -> bool)`; `sort_keys(&mut self) where K: Ord`.
  - `entry(&mut self, key: K) -> Entry<'_, K, V>`; `Entry` in `index_map`: `key(&self) -> &K`, `index(&self) -> usize`, `or_insert(self, V) -> &'a mut V`, `or_insert_with(self, impl FnOnce() -> V) -> &'a mut V`, `or_default(self) -> &'a mut V where V: Default`, `and_modify(self, impl FnOnce(&mut V)) -> Self`; implements `Debug` where `K: Debug, V: Debug`.
  - `iter(&self) -> Iter<'_, K, V>` and `keys(&self) -> Keys<'_, K, V>` in `index_map`, each implementing `Iterator`, `DoubleEndedIterator`, `ExactSizeIterator`, `FusedIterator`, `Clone` and `Debug`.
  - Docs: each removal says what happens to positions; `get_index_of` says positions are valid until the next removal or sort (spec §4.3).
- **Test first:**
  - `src/tests/map_model.rs`: an `Op` enum over keys `u8` in `0..16` (small, so re-insertion and removal hits are frequent) and values `u16`: `Insert(k, v)`, `EntryOrInsert(k, v)`, `EntryAndModify(k, v)` (`and_modify(|x| *x = v).or_insert(v)`), `GetMutSet(k, v)`, `SwapRemove(k)`, `ShiftRemove(k)`, `SwapRemoveIndex(i)`, `ShiftRemoveIndex(i)` (`i` in `0..20`, so out-of-range happens), `Pop`, `RetainEvenValues`, `Clear`, `Reserve(n in 0..64)`, `ShrinkToFit`, `SortKeys`. The model is a `Vec<(u8, u16)>` searched linearly, implementing spec §4.3 rules 1 to 9 directly. Proptest `map_matches_model_with_fold_hasher` and `map_matches_model_with_constant_hasher` (`ops` of length `0..200`) apply each op to both and check after every op: the operation's return value, `iter()` against the model's entries, `len`, `first`, `last`, and for every `k` in `0..16`, `get` and `get_index_of`.
  - `src/tests/golden.rs` `map_golden_order` (R5): the sequence and expected list below.
  - Review-focus unit tests in `index_map.rs`: `empty_map_operations_are_noops`, `string_keys_are_looked_up_by_str`, `swap_remove_of_the_last_entry_moves_nothing`, `vacant_entry_index_is_len`, and `insert_existing_keeps_position_and_returns_old` (insert `a`,`b`,`c`, then `insert(b, 9)` returns `Some(old)` and keys stay `a, b, c`).
  - Run `cargo xtask test`: compile failure (methods missing).
- **Golden sequence** (keys `char`, values `u32`): insert `a1 b2 c3 d4 e5`; `insert('b', 20)`; `swap_remove(&'a')`; `insert('f', 6)`; `shift_remove(&'c')`; `entry('g').or_insert(7)`; `entry('b').or_insert(99)`; `pop()`; `retain(|_, v| *v % 2 == 0)`; `insert('a', 1)`; `swap_remove_index(0)`; `shift_remove_index(1)`; `insert('c', 3)`; `insert('b', 2)`; `sort_keys()`; `insert('z', 26)`. Expected `[('a', 1), ('b', 2), ('c', 3), ('f', 6), ('z', 26)]`, written as a literal. Also assert the intermediate keys after `swap_remove(&'a')` are `e b c d`.
- **Change:** implement the interfaces by delegation (§1).
- **Verify:** `cargo xtask test` passes, both model proptests included; `cargo xtask ci --quick` passes.
- **Commit:** `feat(collections): add IndexMap operations and order model tests (M3.1)`

### Step 3: map iterators, trait impls and `Index` (R2, R6, R7, R8, R10)

- **Files:** `src/index_map.rs`, `src/index_map/iter.rs`, `tests/laws.rs`, `tests/panics.rs`, `tests/auto_traits.rs` (new).
- **Interfaces produced:**
  - `iter_mut`, `values`, `values_mut`, `into_keys`, `into_values`, and types `IterMut`, `Values`, `ValuesMut`, `IntoIter`, `IntoKeys`, `IntoValues` in `index_map`, with the iterator traits of spec §4.4 (`Clone` only on `Values` among the new ones).
  - `IntoIterator` for `IndexMap<K, V>` (`IntoIter`), `&IndexMap` (`Iter`) and `&mut IndexMap` (`IterMut`).
  - `Clone`; `Default` (`new()`); `Debug` (`f.debug_map().entries(self.iter())`); `PartialEq` (`len` equal and `iter().eq(other.iter())`); `Eq`; `Hash` (`state.write_usize(len)`, then each key and value in order); `FromIterator<(K, V)>`, `Extend<(K, V)>`, `From<[(K, V); N]>` (all `K: Hash + Eq`).
  - `Index<&Q>` and `IndexMut<&Q>`, `#[track_caller]`, panicking through a private `#[cold] #[track_caller] fn missing_key() -> !` with the message "IndexMap: no entry for this key; use get (or get_mut) to handle a missing key".
- **Test first:**
  - Unit: `from_iter_keeps_first_position_last_value` (`[(1, 'a'), (2, 'b'), (1, 'c')]` collects to `[(1, 'c'), (2, 'b')]`, and the same through `Extend` and `From`); `debug_format` (`{1: 'a', 2: 'b'}` and `{}`); `iterators_are_double_ended_and_exact` (`rev()` order and `len()` for each iterator type); `index_by_str` (review focus 2: `map["walk"]` on `IndexMap<String, f32>`, and `map["walk"] += 1.0`).
  - `tests/laws.rs` (proptest over `Vec<(u8, u16)>` inputs, hashing with `foldhash::fast::FixedState::with_seed(0).hash_one`, so `foldhash` becomes a dev-dependency): `eq_is_reflexive_and_symmetric`; `equal_maps_hash_equal`; `permutation_is_unequal_until_sorted` (a reordering of at least two distinct keys compares unequal; after `sort_keys` on both, equal and equal hashes).
  - `tests/panics.rs` (native only, `#![cfg(not(target_os = "wasi"))]`, the `engine_handle` hook pattern): `index_missing_key_panics_at_caller` and `index_mut_missing_key_panics_at_caller`: the location ends with `tests/panics.rs`; the message contains "no entry for this key" and "use get".
  - `tests/auto_traits.rs`: `fn assert_traits<T: Send + Sync + Unpin>() {}` for `IndexMap<u32, String>` and each of `Iter`, `IterMut`, `Keys`, `Values`, `ValuesMut`, `IntoIter`, `IntoKeys`, `IntoValues`, `Entry`.
  - Run `cargo xtask test`: compile failure.
- **Change:** implement the interfaces. Each public type gets a doctest. The type docs say equality respects order and warn about order-independent keys (spec §4.6).
- **Verify:** `cargo xtask test` passes; `cargo xtask ci --quick` passes; `cargo xtask doc` shows no `indexmap` or `foldhash` type in any public signature (R2).
- **Commit:** `feat(collections): add IndexMap iterators, trait impls and Index (M3.1)`

### Step 4: `IndexSet` (R2 to R8, R10 for the set)

- **Files:** `src/index_set.rs`, `src/index_set/iter.rs` (new), `src/lib.rs` (re-export), `src/tests/set_model.rs` (new), `src/tests/golden.rs`, `tests/laws.rs`, `tests/auto_traits.rs`.
- **Interfaces produced:** `IndexSet<T> { inner: InnerSet<T, Hasher> }` with the API of spec §4.5: `new` (`const`), `with_capacity`, `len`, `is_empty`, `capacity`, `reserve`, `try_reserve`, `shrink_to_fit`, `clear`, `insert(T) -> bool`, `contains`, `get(&Q) -> Option<&T>`, `get_index_of`, `get_index(usize) -> Option<&T>`, `first`, `last`, `swap_remove(&Q) -> bool`, `shift_remove(&Q) -> bool`, `swap_remove_index(usize) -> Option<T>`, `shift_remove_index(usize) -> Option<T>`, `pop() -> Option<T>`, `retain(impl FnMut(&T) -> bool)`, `sort() where T: Ord`, `iter`; `index_set::{Iter, IntoIter}`; `IntoIterator` for `IndexSet` and `&IndexSet`; `Clone`, `Debug` (`debug_set`), `Default`, `PartialEq`, `Eq`, `Hash`, `FromIterator<T>`, `Extend<T>`, `From<[T; N]>`; `#[cfg(test)] with_test_hasher`.
- **Test first:**
  - `set_model.rs`: the map model's ops without values (`Insert(k)`, `SwapRemove(k)`, `ShiftRemove(k)`, `SwapRemoveIndex(i)`, `ShiftRemoveIndex(i)`, `Pop`, `RetainEven`, `Clear`, `Reserve(n)`, `ShrinkToFit`, `Sort`) against a `Vec<u8>` model, with both hashers, checking the same observations.
  - `golden.rs` `set_golden_order`: insert `5 3 8 1`; `swap_remove(&5)`; `insert(3)` (returns `false`, no change); `insert(9)`; `shift_remove(&8)`; expected `[1, 3, 9]`; then `sort()` keeps `[1, 3, 9]`; then `insert(0)` gives `[1, 3, 9, 0]`.
  - Unit: `empty_set_operations_are_noops` (review focus 1); `debug_format` (`{1, 2}`); `insert_existing_returns_false_and_keeps_position`.
  - `tests/laws.rs`: the three laws for `IndexSet<u8>`, including the guide-level case (`[1, 2]` and `[2, 1]` unequal, equal after `sort`).
  - `tests/auto_traits.rs`: `IndexSet<String>`, `index_set::Iter`, `index_set::IntoIter`.
  - Run `cargo xtask test`: compile failure.
- **Change:** implement by delegation to `indexmap::IndexSet` (`sort` calls `sort_unstable`). Doctests on every public type, including the §3 example of sorting a set before using it as a key.
- **Verify:** `cargo xtask test` passes; `cargo xtask ci --quick` passes.
- **Commit:** `feat(collections): add IndexSet (M3.1)`

### Step 5: allocation test, facade and platform builds (R9, R12, R14)

- **Files:** `libs/engine_collections/Cargo.toml` (dev-dependency `dhat = "0.3"`), `libs/engine_collections/tests/allocations.rs` (new), `libs/engine/Cargo.toml` (`engine_collections = { workspace = true }`), `libs/engine/src/lib.rs` (`pub use engine_collections as collections;`, a crate-map row, a doctest using `engine::collections::IndexMap`), `xtask/src/main.rs` (`engine_collections` in `no_std()` and in `wasm()`'s test list).
- **Test first:**
  - `tests/allocations.rs` (dhat in testing mode, `fixed_step_sim` pattern, reading `total_blocks` outside `dhat::assert_eq!`): `map_with_capacity_never_allocates`: `IndexMap::<u64, u64>::with_capacity(1024)`; record blocks; insert 1,024 distinct keys; 1,024 `get`s; one `iter` pass; `get_mut` on every key; `swap_remove` 100 keys; `shift_remove` 100 keys; `retain` even values; `sort_keys`; `pop`; `clear`; insert 1,024 keys again; blocks unchanged. `map_reserve_never_allocates`: `new()`, `reserve(1024)`, record, insert 1,024 keys, unchanged. `set_with_capacity_never_allocates`: the same sequence for `IndexSet<u64>`.
  - The `engine` doctest fails to compile until the re-export exists.
  - Run `cargo xtask test`: the doctest fails; the allocation tests pass or fail (they test `indexmap`'s behaviour, which the change does not alter). **If an allocation test fails, stop and report it to the maintainer**: spec §4.7 is a MUST and the fix is a spec decision, not a test change.
- **Change:** as listed. `engine::prelude` is unchanged (R12).
- **Verify:** `cargo xtask test` passes; `cargo xtask no-std` builds `engine_collections` for `thumbv7em-none-eabihf`; `cargo xtask wasm` runs its tests under wasmtime (32-bit, so review focus 5 runs there); `cargo xtask ci --quick` passes.
- **Commit:** `feat(collections): re-export from the facade and check allocations and no_std (M3.1)`

### Step 6: benchmarks (R15)

- **Files:** `bench-regress/Cargo.toml` (`[[bench]] name = "collections"`, dev-dependency `engine_collections.workspace = true`), `bench-regress/benches/collections.rs` (new), `benches/Cargo.toml` (`[[bench]] name = "collections"`, dev-dependencies `engine_collections.workspace = true`, `indexmap = "2.14"`, `foldhash = "0.2"`), `benches/benches/collections.rs` (new), `Cargo.lock`.
- **Interfaces:** Gungraun, following `bench-regress/benches/handle.rs` exactly (setup functions, a `*_work` function outside each benchmark, seed `0x5EED`, keys shuffled with `Prng::shuffle`): `map_insert_1k` (insert 1,024 shuffled `u64` keys into `with_capacity(1024)`), `map_get_hit_1k`, `map_get_miss_1k` (1,024 keys not present), `map_iter_1k` (sum of values), `map_get_typeid_64` (64 `TypeId`s of a private `struct Tag<const N: usize>;`, built by a `macro_rules!` over the literals `0` to `63`; look each up once). Group `map`, added to `main!`. Criterion: groups `map_insert`, `map_get_hit`, `map_get_miss`, `map_iter` at sizes 64, 1,024 and 65,536, each with four implementations labelled `engine_collections`, `indexmap_foldhash` (`indexmap::IndexMap<u64, u64, FixedState>`), `indexmap_sip` (`indexmap::IndexMap<u64, u64, BuildHasherDefault<DefaultHasher>>`), and `btreemap`.
- **Test first:** `cargo xtask bench-smoke` fails before the files exist only in the sense that the new targets are missing; the check here is that it runs them once, without measurement, after the change.
- **Change:** as listed. Both files carry the existing crate-level `#![allow(clippy::disallowed_types, ...)]` header of their neighbours.
- **Verify:**
  - `cargo xtask bench-smoke` passes; `cargo xtask ci --quick` passes; `cargo xtask deny` passes.
  - In the devcontainer: `cargo xtask bench-regress` passes. The merge-base has no `collections` target, so the base run cannot measure it; if the command fails rather than skipping a target the base lacks, record that in §4 and report it to the maintainer before changing anything in `xtask`.
  - Wall clock, following the benchmark protocol (AC power, `caffeinate`, no builds during the run, check the power log afterwards): `cargo xtask bench --save m3.1-collections` twice, then `cargo xtask bench-report --baseline m3.1-collections`; record the run-to-run noise floor. Expected: `engine_collections` and `indexmap_foldhash` within the noise floor of each other. Numbers go in the pull request, not in `docs/benchmarks.md` (that is M3.8's).
- **Commit:** `bench(collections): add instruction-count and comparison benchmarks (M3.1)`

### Step 7: mutation testing and triage (R16)

- **Files:** `xtask/src/mutants.rs` (`TRIAGED` gains `"engine_collections"`), `.github/workflows/mutants.yml` (`pull_request.paths` gains `libs/engine_collections/src/**`), `.cargo/mutants.toml` (exclusions only for mutants classed *equivalent* or *not worth a test*, each with its reason), tests in the crate for mutants classed *test gap, fixed*, `docs/mutation-triage.md` (new section).
- **Test first:** `cargo xtask mutants -p engine_collections -j 2` before any new test. Every missed mutant and timeout is classified with the classes in CONTRIBUTING.md "Mutation testing".
- **Change:** a test for each *test gap, fixed*; an exclusion with a reason for each *equivalent* or *not worth a test*. Expected likely survivors: the `Hash` impl's `write_usize(len)` (a mutant that drops it is caught only if a law test distinguishes, for example, `[(1, 2)]` from `[]` plus extra writes; add such a test if missed) and `Display` of `TryReserveError`.
- **Verify:** a second `cargo xtask mutants -p engine_collections -j 2` run; the triage section has a row for every remaining missed mutant and timeout, with both runs' summary tables, as the M2.3 section does. `cargo xtask ci --quick` passes.
- **Commit:** `test(collections): triage mutants in engine_collections (M3.1)`

### Step 8: documents (R11, R17)

- **Files:** `libs/engine_collections/src/lib.rs` (crate docs completed), `docs/adr/0004-crate-splitting-rule.md` (table row: "Dependencies (`indexmap`, `foldhash`); platform (`no_std` + `alloc`); layering (foundation for the ECS, registry and persistence)"), `docs/adr/0011-heap-allocation-control.md` (the Consequences line of spec §4.8), `docs/design/m0-m1-foundations/spec.md` (§4.2 note, spec §4.8), `README.md` (crate table row; `engine_collections` added to the `no_std` and wasm lines), `CHANGELOG.md` (an "Added" entry under Unreleased: the crate, its order guarantee, the bans), `docs/roadmap.md` (M3.1 row `In progress`; M5's rough chunks gain the iteration-order note of spec §4.8), this plan's §4.
- **Test first:** the documentation review in CONTRIBUTING.md fails before the change: ADR 0004 has no row for the crate, and the crate docs do not yet state all the consequences R11 lists.
- **Change:** crate docs gain sections "Guarantees" (order is a function of the operation sequence, not of key values or the hasher; positions are not stable; equality respects order), "Your part" (the caller owns a deterministic operation sequence; inserting in thread-completion order breaks it), "Choosing a collection" (`IndexMap`/`IndexSet`, `BTreeMap` for sorted order, a sorted `Vec` for small order-independent keys), and "Known limitation" (hash flooding with a fixed seed; spec §4.6 and §10). Plus the document edits listed.
- **Verify:** `cargo xtask ci` (the full run, including `doc`, `typos`, `deny`); relative links in the changed files resolve.
- **Commit:** `docs: record engine_collections in the ADRs, README and changelog (M3.1)`

## 3. Final verification

- `cargo xtask ci` passes locally on macOS; `cargo xtask bench-regress` passed in the devcontainer (Step 6).
- With the maintainer's go-ahead: push, open the pull request as a draft, and confirm CI is green on Ubuntu, macOS and Windows, and that the `bench-regress` and `mutants-changed` jobs ran.
- Run the reviews in CONTRIBUTING.md ("Reviews before a pull request"): `/code-review`, `/security-review` (two new dependencies), documentation, spec conformance (each requirement against spec §8), public API (the whole new surface, against spec §4.4 and §4.5), determinism (R3 to R5 and the hasher swap), performance (Step 6 numbers with baseline and noise floor), portability (`no-std`, `wasm`, the three CI platforms).
- The pull request says an assistant was used, includes the ban-check clippy output (R13), the benchmark comparison and noise floor (R15) and the mutants summary (R16), and lists all eight review results.
- Walk the spec's §9 "Done when" list.

## 4. Deviations

- **Model test location (planned).** Spec §8 names `tests/model.rs` for R3. The model tests are unit tests in `src/tests/map_model.rs` and `src/tests/set_model.rs`, because R4's constant hasher is only reachable from inside the crate (§1); the same tests cover R3 and R4.
- **No `extern crate alloc` (implementation).** Spec §4.1 lists `extern crate alloc`. The crate never names an `alloc` item (the backend owns every allocation), so the declaration would be an unused crate under `rust_2018_idioms`; it is left out. The crate is still `#![no_std]` and builds for `thumbv7em-none-eabihf`.
- **Single allocation test (Step 5).** The three planned tests are one `#[test]` with three scenarios, because `dhat` allows one profiler per process (the pattern of `engine_handle`'s `fixed_capacity.rs`).
- **Backend alias (Step 1).** The backend is named through `type Inner<K, V> = indexmap::IndexMap<K, V, Hasher>` with the `#[expect]` on the alias, not `use indexmap::IndexMap as Inner`: with an import, every `Inner::...` call site fires the ban.
- **Items pulled forward (Steps 2 and 3).** `Debug` and `IntoIterator for &IndexMap` are in Step 2, because clippy's `iter_without_into_iter` and the empty-map `Debug` check need them.
- **`foldhash::HashMap` and `HashSet` bans (Step 1).** Both fire in a scratch crate with foldhash's default features, but in the workspace (default features off) clippy warns that the two paths "do not refer to a reachable type", because the aliases need foldhash's `std` feature. The warning does not fail CI. The entries stay as a guard in case a crate enables that feature; the maintainer may prefer to drop them.
- **Tests added by triage (Step 7).** `tests/contracts.rs` (8 tests) closes all 36 mutants missed in the first run.
- **Typos (Step 8).** Two short variable names in the guide-level example were flagged by `typos`; they are `forward` and `backward` in the spec's example and the crate's doctests and tests.
- **Not run in this session (Step 6).** `cargo xtask bench-regress` (needs Valgrind; not available on macOS) and the wall-clock comparison with its baseline and noise floor (needs the benchmark protocol on the maintainer's machine). Both are required before the pull request is ready.
- **Benchmark sizes (Step 6).** The first CI run of `bench-regress` measured `map_iter_1k` at 1,921 instructions and `map_get_typeid_64` at 3,486, below the 10,000-instruction minimum of the M2.4 spec §4.2. They are now `map_iter_10k` (10,240 entries, as `pool_iter_10k` does) and `map_get_typeid_64_x8` (the 64 lookups repeated eight times). The other three were 47,514 to 106,594.
