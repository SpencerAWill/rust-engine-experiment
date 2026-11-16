# M3.17: Hierarchical bitset

- **Status:** Draft (2026-11-16)
- **Milestone:** M3 (scheduled during M3, not part of its exit criteria; M4's queries build on it) · **Depends on:** M3.14 · **Decides:** no ADR
- **Issues:** #44 (sub-issue of #38). Related: #41 / M3.12 (space metrics; §2), the sparse variant (a follow-up issue opened at close-out; §10).
- **Estimated size:** about 1,000 changed lines, excluding tests, lockfiles and generated files (the set about 450, the join about 150, the error type about 40, benchmarks and the space report about 200, fuzz target about 80, documents about 80)

Requirement keywords (MUST, MUST NOT, SHOULD, MAY) follow [RFC 2119](https://www.rfc-editor.org/rfc/rfc2119) as clarified by [RFC 8174](https://www.rfc-editor.org/rfc/rfc8174): they are normative only in uppercase.

## 1. Summary

This chunk adds `engine::collections::HierarchicalBitSet`, a set of indices stored as six layers of `u64` words, where each bit of an upper layer records whether a word of the layer below is non-zero, and `Join`, which iterates the indices present in every one of a list of sets and absent from another list, skipping any range an upper layer shows to be empty. It comes now because M4's queries ("entities with A and B, without C") are joins over per-component sets, and M3.14 fixed the word layout it is built on. At the end, the model tests show the set and the join behave like `BTreeSet<usize>` operations, and `bench/compare` reports time and heap bytes against `BitSet`, `hibitset` and `hi_sparse_bitset` at several occupancies.

## 2. Goals and non-goals

**Goals**

- A set whose layout, limits and iteration order are the same on every target: `u64` words, six layers, and `MAX_BITS = u32::MAX as usize` as on `BitSet`.
- Iteration and joins whose cost follows the non-empty words, not the largest index.
- A join over a number of sets known only at run time (runtime-registered components, M3.6; C# queries, M8), with no allocation.
- Time and heap bytes measured against the dense and sparse alternatives, so the follow-up decision on a sparse variant has numbers.

**Non-goals**

- No dependency. The implementation is our own (§7); `hibitset` and `hi_sparse_bitset` are dev-dependencies of `bench/compare` only.
- No sparse variant (storing only occupied blocks). It is measured through `hi_sparse_bitset` here and decided in a follow-up issue (§10).
- No public size API. Space is measured with `dhat` in `bench/compare` (§4.8); `HeapSize` is M3.12's to design.
- No lazy union (OR) join, no composable join views (`a.and(&b)`), no backward iteration, no parallel iteration, no atomic variant. Each is added when a chunk needs it (§10).
- No mutable access to the layers, no serde or reflection (M5), no debugger visualizers (M4).
- No new ADR, no frozen format.

## 3. Guide-level explanation

A `HierarchicalBitSet` is used like a `BitSet`:

```rust
use engine::collections::HierarchicalBitSet;

let mut positions = HierarchicalBitSet::new();
positions.insert(3);
positions.insert(1_000_000);
assert!(positions.contains(1_000_000));
assert_eq!(positions.iter().collect::<Vec<_>>(), [3, 1_000_000]);
```

Iterating it skips empty ranges 64, 4,096, 262,144 indices at a time, so a set with a few members spread over a large range iterates in time proportional to its members, not its range. A `Join` does the same across several sets:

```rust
use engine::collections::{HierarchicalBitSet, Join};

let position: HierarchicalBitSet = [1, 5, 9, 700].into_iter().collect();
let velocity: HierarchicalBitSet = [5, 9, 700, 800].into_iter().collect();
let frozen: HierarchicalBitSet = [9].into_iter().collect();

// Entities with Position and Velocity, without Frozen.
let moving: Vec<usize> = Join::new(&[&position, &velocity], &[&frozen]).collect();
assert_eq!(moving, [5, 700]);
```

The lists are slices, so a query built at run time passes a `Vec` of references.

## 4. Reference-level design

### 4.1 Module and layout

- `engine::collections::bit_set` MUST gain `HierarchicalBitSet`, its iterator `HierarchicalIter`, and `Join`. `HierarchicalBitSet` and `Join` MUST be re-exported at `engine::collections`, and not in `engine::prelude` (as M3.14 §4.1).
- Word routines MUST come from the existing private `bit_set/words.rs` where one fits (counting, first, trimming, equality and hashing over a trimmed slice); new routines the layers need go there too.
- The set needs `alloc`; it builds in `cargo xtask no-std` and runs in `cargo xtask wasm`. No `unsafe` and no new normal dependency.

### 4.2 Layers

```rust
pub struct HierarchicalBitSet { layers: [Vec<u64>; 5], top: u64 }
```

(The exact fields are the builder's; the layout below is normative.)

- Layer 0 holds the members in the layout of M3.14 §4.2: bit `i` is bit `i % 64` of word `i / 64`.
- For `k` in 1 to 5, bit `b` of word `j` of layer `k` is set if and only if word `j × 64 + b` of layer `k − 1` exists and is non-zero. Layer 5 is a single word (`top`): `u32::MAX >> 30` is 3, so only its bits 0 to 3 are ever set.
- Each of layers 0 to 4 is as long as needed for the highest index it has had to hold, and MAY end in zero words, as `BitSet` does. Nothing is sized from `usize::BITS`.
- `pub const MAX_BITS: usize = u32::MAX as usize;` valid indices are `0..MAX_BITS` on every target.
- `as_words()` returns layer 0 and `as_trimmed_words()` returns it without trailing zero words, with the meaning of M3.14 §4.2.
- `check_invariants(&self) -> Result<(), BitSetInvariantError>` checks the rule above for every layer and that no bit at or above `MAX_BITS` is set, as `GenIdMap::check_invariants` does for its type. The tests and the fuzz target call it after each operation.

### 4.3 `HierarchicalBitSet`

| Group | Items |
|---|---|
| Construction | `const fn new()`, `with_capacity(bits)`, `try_with_capacity(bits)`, `Default`, `FromIterator<usize>`, `Extend<usize>`, `Extend<&usize>`, `From<[usize; N]>`, `try_extend(impl IntoIterator<Item = usize>)`, `From<&BitSet>`, and `From<&HierarchicalBitSet> for BitSet` |
| Size and capacity | `len()` (O(words of layer 0)), `is_empty()` (O(1): `top == 0`), `capacity()` (`64 ×` the words allocated in layer 0), `ensure_capacity(bits)`, `try_ensure_capacity(bits)`, `shrink_to_fit()`, `clear()` (keeps the allocations) |
| Bits | `insert(usize) -> bool`, `try_insert(usize) -> Result<bool, TryReserveError>`, `remove(usize) -> bool` (never allocates), `contains(usize) -> bool` |
| Order | `first()`, `last()` (each O(layers): descend from `top`), `iter() -> HierarchicalIter<'_>`; `IntoIterator` for `&HierarchicalBitSet` |
| Set algebra, in place | `union_with(&Self)`, `intersect_with(&Self)`, `difference_with(&Self)` |
| Tests | `is_subset(&Self)`, `is_superset(&Self)`, `is_disjoint(&Self)` |
| Words and checks | `as_words()`, `as_trimmed_words()`, `check_invariants()` (§4.2) |
| Traits | `Clone`, `Debug`, `Default`, `PartialEq`, `Eq`, `Hash` |

- `insert` sets the bit in layer 0 and, only if that word was zero before, sets the parent bits upwards, stopping at the first parent word that was already non-zero. `remove` clears the bit and, only if the word became zero, clears the parent bits upwards, stopping at the first parent word that stays non-zero. `contains` reads layer 0 only.
- `intersect_with`, `difference_with`, `is_subset`, `is_superset` and `is_disjoint` SHOULD visit only the layer-0 words under set bits of the upper layers (of `self`, or of both sets for `is_disjoint` and `intersect_with`), so their cost follows the non-empty words. `union_with` grows `self` to `other`'s length if it is longer.
- `symmetric_difference_with` is left out (my call): nothing planned needs it, and it is the one operation whose upper layers cannot be updated without re-deriving them.
- Every operation's result MUST equal the same operation on a `BTreeSet<usize>` with the same members, and `check_invariants` MUST pass after every operation.
- Equality, hashing and `Debug` follow M3.14 §4.5: equality and `Hash` depend only on the members (`Hash` hashes `as_trimmed_words()`), and `Debug` prints `{3, 1000000}`.

### 4.4 Iteration and `Join`

- `HierarchicalIter<'a>` MUST yield members in ascending order and implement `FusedIterator`. It keeps one word per layer (a fixed array, no allocation), takes the lowest set bit with `trailing_zeros` and clears it with `w & (w - 1)` (M3.14 §11), and descends only into words whose parent bit is set. Its cost is O(1) per member plus O(1) per non-zero word visited in each layer.
- `Join<'a>` is an iterator over `usize`:

  ```rust
  impl<'a> Join<'a> {
      pub fn new(required: &'a [&'a HierarchicalBitSet], excluded: &'a [&'a HierarchicalBitSet]) -> Self;
      pub fn try_new(required: &'a [&'a HierarchicalBitSet], excluded: &'a [&'a HierarchicalBitSet]) -> Result<Self, EmptyJoinError>;
  }
  ```

  It yields, in ascending order, every index present in all of `required` and in none of `excluded`. At each upper layer and at layer 0 it ANDs the `required` sets' words (a word past a set's end counts as zero); it applies `excluded` at layer 0 only, as `word & !excluded_word`, because an upper bit says only that something below is set. It MUST NOT allocate, and implements `FusedIterator`.
- `Join::new` MUST panic if `required` is empty, because the result would be every index not excluded; `try_new` returns `Err(EmptyJoinError)` instead. A set may appear more than once, and in both lists (the join is then empty).
- `HierarchicalBitSet` and both iterators MUST be `Send`, `Sync` and `Unpin`.

### 4.5 Errors and panics

Every panic follows the repository rule: `#[track_caller]`, a `# Panics` section, an actionable message, and a `try_` twin.

| Panics | When | Twin |
|---|---|---|
| `insert(i)` | `i >= MAX_BITS` | `try_insert`, `Err(TryReserveError)` |
| `with_capacity(bits)`, `ensure_capacity(bits)` | `bits > MAX_BITS` | `try_with_capacity`, `try_ensure_capacity` |
| `FromIterator`, `Extend`, `From<[usize; N]>` | an index `>= MAX_BITS` | `try_extend` |
| `Join::new` | `required` is empty | `Join::try_new`, `Err(EmptyJoinError)` |

- `EmptyJoinError` and `BitSetInvariantError` are new public errors in `collections::error`, re-exported at `engine::collections`, each `#[non_exhaustive]` with `Display`, `Error`, `Debug`, `Clone`, `PartialEq` and `Eq`. `BitSetInvariantError`'s `Display` names the layer and word that break the rule.
- Example message: `Join::new: no required sets were given; a join needs at least one set to draw indices from (to iterate everything not excluded, pass the set of live indices)`.
- `remove`, `contains` and iteration never panic.

### 4.6 Heap allocation (ADR 0011)

- After `with_capacity(n)` or `ensure_capacity(n)`, every `insert(i)` with `i < n`, every `remove`, `contains`, `first`, `last`, `len`, `clear`, `intersect_with`, `difference_with`, test, iteration and `Join`, and every `union_with` whose argument holds no index at or above `n`, MUST NOT allocate.
- `Join::new` and `Join::try_new` never allocate.

### 4.7 Space

- Layer 0 holds `ceil((highest index + 1) / 64)` words at least; each upper layer holds at most `1/64` of the layer below, plus one word. The module documentation MUST state this, so a reader can work out the cost of a set with one member at a high index (about `index / 8` bytes) and see why the sparse variant exists.

### 4.8 Tests, benchmarks, space report, fuzzing and documents

**Tests.**

- A proptest model in `libs/engine/src/collections/tests/` drives random operation sequences (every method of §4.3, with indices near zero, near word and layer boundaries (multiples of 64, 4,096, 262,144) and near `MAX_BITS`) against a `BTreeSet<usize>`, comparing members, `len`, `is_empty`, `first`, `last`, iteration and `contains`, and calling `check_invariants`, after each operation.
- A `Join` proptest: one to four required and zero to two excluded sets, including repeats and sets of different lengths, against `BTreeSet` intersection and difference.
- A golden test of the layers: a fixed set of indices produces literal words in layers 0 to 5.
- Conversions to and from `BitSet` keep the members; equality and hashing across capacities, as M3.14 §4.8.
- `tests/allocations.rs` (`dhat`) for §4.6; `#[should_panic(expected = ...)]` tests for every panic of §4.5, with a panic-hook test of the caller's location; compile-time `Send + Sync + Unpin` assertions.

**Fuzzing.** A new target, `fuzz/fuzz_targets/hierarchical_bit_set_ops.rs`, applies operation sequences and joins to three `HierarchicalBitSet`s against `BTreeSet<usize>` models, calls `check_invariants` after each operation, and is added to the nightly fuzz job's target list.

**Benchmarks.**

- `bench/regress/benches/collections.rs` (Gungraun, gated at 5%, each above the gate's 10,000-instruction minimum): inserting 4,096 indices into a set made `with_capacity(4_096)`; `contains` over 4,096 indices at 50% occupancy; iterating a 262,144-bit range at 1% occupancy; a `Join` of three sets with one exclusion at 50% occupancy.
- `bench/compare/benches/collections.rs` (wall clock, report only): insert, `contains`, iteration, and a three-way intersection with one exclusion, on `HierarchicalBitSet`, `BitSet`, `hibitset::BitSet` and `hi_sparse_bitset::BitSet` (the `_128bit` config, whose range of 2,097,152 indices covers the cases), over a 1,048,576-index range at 0.1%, 1%, 50% and 99% occupancy, with members spread uniformly and clustered in runs. `BitSet` has no multi-way join, so its intersection is done with `intersect_with` into a scratch set, and the report says so. The dev-dependencies are `hibitset` with default features off (no `rayon`) and `hi_sparse_bitset` with its defaults; both MUST pass `cargo xtask deny` without changes to `deny.toml`.

**Space report.** A report-only target in `bench/compare` builds each of the four structures for each occupancy and spread above, plus "one member at index 1,000,000", under `dhat`, and prints a table of heap bytes after building, peak heap bytes during building, and bytes per member. `dhat`'s peak is per process, so each measurement runs in a fresh process or the target documents how it isolates them (the plan decides). The pull request carries the table, the machine and the commit.

**xtask.** The new files are in the triaged `collections` mutants module; new missed mutants and timeouts are triaged in `docs/mutation-triage.md`.

**Documents.**

- The `engine::collections` module documentation: a paragraph on the hierarchical bitset and the join, and a row in "Choosing a collection" (a set of indices that is iterated or joined while sparse: `HierarchicalBitSet`).
- The `bit_set` module documentation: the layer rule of §4.2 and the space bound of §4.7.
- `engine`'s module map and `README.md`: the `collections` row names the set.
- `docs/research/data-structures.md`: the Hierarchical bitset entry's **Repo fit** gains `in repo` and its **Sources** list §11.
- `docs/roadmap.md`: the M3.17 row. `CHANGELOG.md` records the chunk.

## 5. Requirements

- **M3.17-R1** `HierarchicalBitSet`, `HierarchicalIter` and `Join` MUST exist as in §4.1, with `HierarchicalBitSet`, `Join`, `EmptyJoinError` and `BitSetInvariantError` re-exported at `engine::collections` and absent from `engine::prelude`, with no new normal dependency.
- **M3.17-R2** The layers MUST follow §4.2 on every target, with `MAX_BITS == u32::MAX as usize`, and `check_invariants` MUST detect any break of the layer rule.
- **M3.17-R3** `HierarchicalBitSet` MUST provide the items of §4.3 with the stated semantics, and every operation MUST agree with a `BTreeSet<usize>` model and leave `check_invariants` passing.
- **M3.17-R4** Iteration MUST be ascending and skip empty upper-layer ranges, as in §4.4.
- **M3.17-R5** `Join` MUST yield exactly the indices in every required set and no excluded set, in ascending order, as in §4.4, and MUST NOT allocate.
- **M3.17-R6** Equality, hashing and `Debug` MUST follow §4.3, and `a == b` MUST imply `hash(a) == hash(b)`.
- **M3.17-R7** The set and both iterators MUST be `Send`, `Sync` and `Unpin`.
- **M3.17-R8** Every panic of §4.5 MUST have `#[track_caller]`, a `# Panics` section, an actionable message and its `try_` twin; no other public item may panic apart from allocation failure.
- **M3.17-R9** Heap allocation MUST follow §4.6.
- **M3.17-R10** Every public item MUST be documented, every public type MUST have a runnable doctest, and the module documentation MUST state §4.2's layer rule, `MAX_BITS` and §4.7's space bound.
- **M3.17-R11** `engine` MUST build in `cargo xtask no-std`, and its tests MUST pass in `cargo xtask wasm`.
- **M3.17-R12** The fuzz target of §4.8 MUST exist, be in the nightly fuzz job, and run for at least 10 minutes locally without a failure before the pull request is marked ready.
- **M3.17-R13** The benchmarks of §4.8 MUST exist and `cargo xtask bench-regress` MUST pass; the wall-clock comparison MUST be in the pull request with a saved baseline and the noise floor.
- **M3.17-R14** The space report of §4.8 MUST exist and its table MUST be in the pull request.
- **M3.17-R15** `cargo xtask mutants --module collections` MUST have been run, with every new missed mutant and timeout triaged.
- **M3.17-R16** The document changes of §4.8 MUST be made.
- **M3.17-R17** `cargo xtask deny` MUST pass without changes to `deny.toml`, and `cargo xtask ci` MUST pass.

## 6. Cross-cutting concerns

- **Determinism:** no hashing, randomness or floats. The layer count, word width and `MAX_BITS` are constants independent of pointer width (R2), which is the property `hibitset` lacks (§11). Iteration and join order is ascending, independent of operation history.
- **`unsafe`:** none in `engine`. The two baselines are dev-dependencies of `bench/compare` only; `hi_sparse_bitset` contains `unsafe` code and is not linked into `engine`.
- **Concurrency:** no interior mutability; `Send` and `Sync` (R7). No parallel iteration (§2).
- **Heap allocation:** §4.6, checked with `dhat` (R9). Space is reported, not gated (R14).
- **`no_std` and wasm:** `alloc` only (R11); the wasm32 run checks the 32-bit behaviour.
- **Frozen formats:** none. The layer rule is a documented contract; changing it is a breaking API change.
- **Public API changes:** new items only (§4.1, §4.5). No existing item changes.
- **FFI exposure:** n/a in M3.
- **Performance:** Gungraun gate for the new benchmarks (R13); the wall-clock and space comparisons are report-only. `hi_sparse_bitset` uses SIMD by default and may be faster on some operations; the pull request says where and by how much.
- **Documentation:** item docs and doctests (R10), the module rows, the catalog entry.
- **Policy changes:** none.

## 7. Alternatives considered

- **Wrap `hibitset`.** Rejected: every layer is `usize`, using 5 bits per level on 32-bit targets and 6 on 64-bit ones, so its largest index (2^20 or 2^24) and layout depend on pointer width. A wrapper cannot change the storage.
- **Wrap `hi_sparse_bitset` privately**, as `indexmap` is wrapped. Its configs are fixed-width, so it is deterministic, and it measures well. Rejected by the maintainer: it contains `unsafe` in 256 places, depends on `wide` for SIMD by default, needs Rust 1.95 or later, has a generic `Config` API that would either leak into our signatures or be fixed to one range, and the join API, which is what M4 needs, would be its own. A layered set on our words is a few hundred lines.
- **A sparse variant now, or both variants.** Rejected by the maintainer for now: it roughly doubles the chunk before there are numbers showing it pays. `hi_sparse_bitset` stands in for it in the comparison (§10).
- **Four layers (indices below 2^24).** Rejected by the maintainer in favour of six layers and the `u32::MAX` limit shared with `BitSet` and `GenId`'s index. The extra layers change only when a word goes empty or non-empty, `contains` reads layer 0 only, and upper layers cost about 1/63 of layer 0 whatever the depth.
- **A height that grows with the largest index.** Rejected: joins over sets of different heights need alignment logic, for a saving of a few words.
- **Composable lazy views** (`a.and(&b).and_not(&c)`, as `hibitset` has). Rejected by the maintainer for now: the number of sets is fixed in the type, so runtime queries cannot use them, and the layer-access trait becomes public API. Views can be added later without changing `Join`.
- **Report space through a `HeapSize` trait now.** Rejected: it would design M3.12's trait inside this chunk, and it cannot measure the external baselines; `dhat` measures all four the same way.
- **Build on `BitSet` as layer 0** (a `BitSet` field). Not chosen (my call): the layer rule ties layer 0 to the upper layers, so `BitSet`'s public mutators could not be exposed anyway; the shared private word routines give the reuse without the coupling.
- **Names** (my call). `HierarchicalBitSet` says what it is; `HiBitSet` copies a crate's name, and `LayeredBitSet` is less familiar. `Join` matches the ECS term and `hibitset`'s concept of a join over sets.

## 8. Verification

| Requirement | Verified by |
|---|---|
| R1 | `cargo doc` and review of `engine::collections` and `engine::prelude`; `cargo tree -p engine -e normal --depth 1` unchanged |
| R2 | golden layer test; a unit test of `MAX_BITS`; unit tests that `check_invariants` reports a hand-broken set (built with a test-only constructor); the suite under `cargo xtask wasm` and on 64-bit CI hosts |
| R3 | the proptest model of §4.8 |
| R4 | the proptest model compares iteration; a unit test counting the layer-0 words visited when iterating a set with two members 1,000,000 apart (a test-only counter, or the Gungraun 1%-occupancy benchmark's instruction count) |
| R5 | the `Join` proptest; `tests/allocations.rs` |
| R6 | proptests of equality and hashing across capacities, with a hasher that records its input |
| R7 | compile-time assertions |
| R8 | `#[should_panic(expected = ...)]` tests; panic-hook location test; review of `# Panics` sections |
| R9 | `tests/allocations.rs` (`dhat`): zero blocks for every operation of §4.6 |
| R10 | documentation review; `cargo xtask doc` |
| R11 | `cargo xtask no-std`; `cargo xtask wasm` |
| R12 | the fuzz target file; the nightly job's target list; the local `cargo xtask fuzz` summary in the pull request |
| R13 | `cargo xtask bench-regress` in the devcontainer; `cargo xtask bench-report` with a saved baseline and the noise floor in the pull request |
| R14 | the space report target; its table, machine and commit in the pull request |
| R15 | `docs/mutation-triage.md`; the mutants summary in the pull request |
| R16 | documentation review; `cargo xtask check-docs` |
| R17 | `cargo xtask deny`; `cargo xtask ci`; the pull request's CI run |

## 9. Done when

- [ ] `cargo xtask ci` passes locally and CI is green on all three platforms.
- [ ] The model and join tests pass, including under `cargo xtask wasm`.
- [ ] The fuzz target ran for 10 minutes without a failure.
- [ ] `cargo xtask bench-regress` passes, and the wall-clock comparison and space table are in the pull request.
- [ ] The mutation triage is recorded.
- [ ] The reviews are recorded in the pull request.
- [ ] The spec is `Implemented` and the plan `Historical`, #44's box in #38 is ticked, and the sparse-variant follow-up issue is open.

## 10. Open questions

None blocking. Deferred, each to a named place:

- **Sparse variant:** a follow-up issue opened at close-out, carrying this chunk's space and time tables; decided on those numbers.
- **`HeapSize` for this set and `BitSet`:** M3.12 (#41), added to its retrofit list.
- **Lazy union join, composable views, backward iteration, `symmetric_difference_with`:** the first chunk that needs one (M4 for an `Or` query filter).
- **Parallel iteration:** M7, splitting at upper-layer words.
- **Serialization and reflection:** M5. Debugger visualizers: M4.

## 11. Prior art and sources

Read for this spec:

- [`hibitset` 0.6.4](https://docs.rs/hibitset) source: four layers of `usize`; `BITS` is 5 on 32-bit targets and 6 on 64-bit ones (`src/util.rs`), so its range depends on pointer width; joins through composable `BitSetAnd`, `BitSetOr` and `BitSetNot` views; `rayon` on by default.
- [`hi_sparse_bitset` 0.10.0](https://docs.rs/hi_sparse_bitset/0.10.0) source: fixed `_64bit`, `_128bit` and `_256bit` configs (262,144 indices for `_64bit`), only occupied blocks stored, `wide` SIMD by default, `unsafe` in 256 places, MSRV 1.95.
- M3.14 spec §4.2 and §4.5: the word layout and the iteration technique this set reuses.

Cited for the technique, to be read before the implementation relies on it:

- Daniel Lemire, [Iterating over set bits quickly](https://lemire.me/blog/2018/02/21/iterating-over-set-bits-quickly/) (2018).
- Henry S. Warren, Jr., *Hacker's Delight*, 2nd ed. (Addison-Wesley, 2012), chapters 2 and 5.
