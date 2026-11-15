# M3.14: Bitsets (`BitSet` and `BitArray`)

- **Status:** Implemented (PR #56, 2026-11-15). Amended the same day while writing the plan, before any code: `try_from_words` returns `TryReserveError` (a bit index on a 32-bit target may not fit in `usize`), `try_extend` is added as the fallible twin of `BitSet`'s `FromIterator`, `Extend` and `From`, `BitIndexError`'s module is stated once, and the set-algebra benchmarks use 262,144-bit sets to clear the gate's minimum (§4.1, §4.2, §4.3, §4.6, §4.8). Changed during implementation: one union of two 262,144-bit sets measured 9,483 instructions, under the gate's minimum, so that benchmark does two rounds (plan §4).
- **Milestone:** M3 (scheduled during M3, not part of its exit criteria; M3.5, M4 and the hierarchical bitset build on it) · **Depends on:** none · **Decides:** no ADR
- **Issues:** #42 (sub-issue of #38). Related: #44 (hierarchical bitset, which builds on the word layout of §4.2), #41 / M3.12 (space metrics, left out here; §2).
- **Estimated size:** about 900 changed lines, excluding tests, lockfiles and generated files (shared word routines about 120, `BitSet` about 300, `BitArray` about 180, iterators and the error type about 80, benchmarks about 100, fuzz target about 60, documents about 60)

Requirement keywords (MUST, MUST NOT, SHOULD, MAY) follow [RFC 2119](https://www.rfc-editor.org/rfc/rfc2119) as clarified by [RFC 8174](https://www.rfc-editor.org/rfc/rfc8174): they are normative only in uppercase.

## 1. Summary

This chunk adds `engine::collections::bit_set`, with two sets of small non-negative integers stored one bit per index in `u64` words: `BitSet`, which lives on the heap and grows to fit any index it is given, and `BitArray<WORDS>`, which is inline, never allocates, and holds `WORDS × 64` bits. It comes now because component masks, dirty flags, occupancy and allocator free bins all need one, the M3.5 hybrid prototype and M4 will use them, and the hierarchical bitset (#44) is built from the word layout fixed here. At the end, `use engine::collections::{BitSet, BitArray};` works in engine code, the model tests show both types behave like a `BTreeSet<usize>`, and the words and iteration order are the same on x86_64, aarch64, wasm32 and 32-bit targets.

## 2. Goals and non-goals

**Goals**

- A set of indices whose behaviour, word layout and limits are the same on every target: words are `u64` regardless of pointer width, and the largest index is the same constant everywhere.
- Set algebra in place (union, intersection, difference, symmetric difference) and the subset, superset and disjointness tests that component-mask matching needs.
- Iteration in ascending order, forwards and backwards, a word at a time.
- Read access to the words, in a documented layout, so the hierarchical bitset and allocator code can work a word at a time.
- An inline type for masks whose size is known at compile time, with no heap use at all.
- Equality and `Hash` that depend only on which bits are set, never on capacity or growth history.

**Non-goals**

- No dependency. The implementation is our own (§7); `fixedbitset` is a dev-dependency only, as a benchmark baseline.
- No space reporting. Issue #42 asks for bytes per element next to time, using M3.12's `HeapSize`; the maintainer left that out of this chunk, and M3.12 can add `BitSet` to its retrofit list (§10).
- No hierarchical, sparse, compressed (Roaring) or atomic variant: those are #44 and later catalog entries.
- No lazy set-algebra iterators (`a.intersection(&b)` yielding indices), no operations between a `BitSet` and a `BitArray`, no range operations (`insert_range`, `iter_from`), no bit slicing or bit order parameter, no `Ord`. Each is added when a chunk needs it (§10).
- No mutable access to the words: it would let callers break the invariants of §4.3 and is not needed by #44, which reads words.
- No serde or reflection support (M5), no debugger visualizers (M4), no allocator type parameter (ADR 0011).
- No new ADR, no frozen format. The word layout of §4.2 is a public contract, not a serialized format.

## 3. Guide-level explanation

A `BitSet` is a set of indices. It grows when you insert an index past its end, and asking about an index past its end answers `false`:

```rust
use engine::collections::BitSet;

let mut dirty = BitSet::new();
dirty.insert(3);
dirty.insert(130);          // grows to fit
assert!(dirty.contains(130));
assert!(!dirty.contains(10_000)); // past the end: not a member, no allocation

assert_eq!(dirty.iter().collect::<Vec<_>>(), [3, 130]); // always ascending
assert!(dirty.remove(3));
assert_eq!(dirty.len(), 1);
```

Component-mask matching is set algebra:

```rust
use engine::collections::BitSet;

let archetype: BitSet = [0, 2, 5, 9].into_iter().collect();
let query: BitSet = [2, 5].into_iter().collect();
let excluded: BitSet = [7].into_iter().collect();

assert!(query.is_subset(&archetype));
assert!(excluded.is_disjoint(&archetype));

let mut shared = archetype.clone();
shared.intersect_with(&query);
assert_eq!(shared, query);
```

Equality ignores how much room a set has, so sets with the same members are equal however they got there:

```rust
use engine::collections::BitSet;

let mut grown = BitSet::with_capacity(4_096);
grown.insert(1);
grown.insert(4_000);
grown.remove(4_000);

let small: BitSet = [1].into_iter().collect();
assert_eq!(grown, small); // equal, and their hashes are equal
```

A `BitArray<WORDS>` is the same idea with a fixed size, stored inline: `BitArray<1>` holds bits 0 to 63, `BitArray<2>` holds 0 to 127. The size is a number of words, because stable Rust cannot yet size an array from a bit count (§7). Inserting past the end is a bug, so it panics, and `try_insert` is the fallible version:

```rust
use engine::collections::BitArray;

let mut free_bins = BitArray::<1>::new();
free_bins.insert(5);
free_bins.insert(17);
assert_eq!(free_bins.first(), Some(5)); // lowest free bin
assert!(free_bins.try_insert(64).is_err()); // BitArray<1> holds 0..64
assert_eq!(BitArray::<1>::CAPACITY, 64);
```

## 4. Reference-level design

### 4.1 Module and layout

- `engine::collections` MUST gain a public module `bit_set` holding `BitSet`, `BitArray` and their iterators; `BitIndexError` goes in `collections::error` with `TryReserveError` (§4.6). `BitSet`, `BitArray` and `BitIndexError` MUST be re-exported at `engine::collections`. Neither type goes in `engine::prelude`, as with `IndexMap` (M3.1 §4.1).
- The word routines the two types share (iteration, first and last, counting, subset and disjointness tests, equality and hashing over a trimmed word slice) MUST live in one private module and be used by both types, so the two cannot drift apart.
- `BitSet` needs `alloc`; `BitArray` needs only `core`. Both build in `cargo xtask no-std` and run in `cargo xtask wasm`.
- No `unsafe` and no new normal dependency.

### 4.2 Bit layout and word access

- A word is a `u64`. Bit `i` is bit `i % 64` (counted from the least significant bit) of word `i / 64`. This MUST hold on every target, and the module documentation MUST state it, because `as_words` exposes it.
- `BitSet::as_words(&self) -> &[u64]` returns the words in use. It MAY end in zero words (after removals, or after `intersect_with`); callers that need a canonical form use `as_trimmed_words`.
- `BitSet::as_trimmed_words(&self) -> &[u64]` returns `as_words` without its trailing zero words. Equal sets return equal slices.
- `BitArray::as_words(&self) -> &[u64; WORDS]` and `const fn BitArray::from_words(words: [u64; WORDS]) -> Self`.
- `BitSet::from_words(words: &[u64]) -> BitSet` copies the words. It MUST panic if the slice holds a bit at or above `MAX_BITS` (§4.6), with `try_from_words(&[u64]) -> Result<BitSet, TryReserveError>` as its twin. (The check is done in `u64` arithmetic: on a 32-bit target a slice can hold bits whose index does not fit in `usize`.)

### 4.3 `BitSet`

```rust
pub struct BitSet { words: Vec<u64> }
```

Invariant: no bit at or above `MAX_BITS` is set. Trailing zero words are allowed.

`pub const MAX_BITS: usize = u32::MAX as usize;` Indices are `usize`, and valid indices are `0..MAX_BITS` on every target. The limit keeps behaviour the same on 32-bit and 64-bit targets: without it, the largest index a program could insert would depend on pointer width.

| Group | Items |
|---|---|
| Construction | `const fn new()`, `with_capacity(bits)`, `try_with_capacity(bits)`, `from_words(&[u64])`, `try_from_words(&[u64])`, `Default`, `FromIterator<usize>`, `Extend<usize>`, `Extend<&usize>`, `From<[usize; N]>`, `try_extend(impl IntoIterator<Item = usize>) -> Result<(), TryReserveError>` (the fallible twin of the three trait impls; on `Err`, the indices before the failing one have been inserted) |
| Size and capacity | `len()` (members, O(words)), `is_empty()` (O(words)), `capacity()` (bits that fit without allocating: `64 × words allocated`), `ensure_capacity(bits)`, `try_ensure_capacity(bits) -> Result<(), TryReserveError>`, `shrink_to_fit()` (drops trailing zero words, then shrinks the allocation), `clear()` (keeps the allocation) |
| Bits | `insert(usize) -> bool` (true if newly added), `try_insert(usize) -> Result<bool, TryReserveError>`, `remove(usize) -> bool` (true if it was a member; never allocates), `contains(usize) -> bool` |
| Order | `first() -> Option<usize>`, `last() -> Option<usize>`, `iter() -> Iter<'_>`; `IntoIterator` for `&BitSet` (`Iter`) and `BitSet` (`IntoIter`) |
| Set algebra, in place | `union_with(&BitSet)`, `intersect_with(&BitSet)`, `difference_with(&BitSet)`, `symmetric_difference_with(&BitSet)` |
| Tests | `is_subset(&BitSet)`, `is_superset(&BitSet)`, `is_disjoint(&BitSet)` |
| Words | `as_words()`, `as_trimmed_words()` (§4.2) |
| Traits | `Clone`, `Debug`, `Default`, `PartialEq`, `Eq`, `Hash` (§4.5) |

- `insert(i)` grows the storage to the smallest word count that holds `i` (amortized, as `Vec` does), so `capacity()` may then exceed what was asked for.
- `ensure_capacity(bits)` makes `capacity() >= bits`. It takes a total bit count, not an additional count as `Vec::reserve` does, which is why it is not named `reserve`.
- `union_with` and `symmetric_difference_with` grow `self` to `other`'s word count if `other` is longer. `intersect_with` and `difference_with` never allocate and never shrink the storage.
- `is_subset`, `is_superset`, `is_disjoint` and equality treat words past the end of the shorter set as zero.
- Every operation's result MUST equal the same operation on a `BTreeSet<usize>` holding the same members.

### 4.4 `BitArray<const WORDS: usize>`

```rust
#[derive(Clone, Copy)]
pub struct BitArray<const WORDS: usize> { words: [u64; WORDS] }
```

`pub const CAPACITY: usize = WORDS * 64;` Valid indices are `0..CAPACITY`. `BitArray<0>` is allowed and holds nothing.

| Group | Items |
|---|---|
| Construction | `const fn new()`, `const fn from_words([u64; WORDS])`, `Default` |
| Size | `len()`, `is_empty()`, `clear()`, `CAPACITY` |
| Bits | `insert(usize) -> bool`, `try_insert(usize) -> Result<bool, BitIndexError>`, `remove(usize) -> bool`, `try_remove(usize) -> Result<bool, BitIndexError>`, `const fn contains(usize) -> bool` (`false` at or above `CAPACITY`) |
| Order | `first()`, `last()`, `iter() -> Iter<'_>`; `IntoIterator` for `&BitArray` |
| Set algebra, in place | `union_with`, `intersect_with`, `difference_with`, `symmetric_difference_with`, each taking `&BitArray<WORDS>` |
| Tests | `is_subset`, `is_superset`, `is_disjoint` |
| Words | `as_words()`, `from_words()` (§4.2) |
| Traits | `Clone`, `Copy`, `Debug`, `Default`, `PartialEq`, `Eq`, `Hash` |

- No `FromIterator` or `From<[usize; N]>`: both would have to panic on an index past `CAPACITY` with no fallible twin in the trait.
- No method of `BitArray` allocates.

### 4.5 Iteration, equality, hashing and `Debug`

- `Iter<'a>` is shared by both types (it walks a `&[u64]`). It MUST yield members in ascending order, implement `DoubleEndedIterator` (descending from the back) and `FusedIterator`, and find each member in O(1) per member plus O(1) per zero word, by taking the lowest set bit with `u64::trailing_zeros` and clearing it with `w & (w - 1)` (§11). `IntoIter` is the owning equivalent for `BitSet`.
- `PartialEq` and `Eq` MUST compare members only: two `BitSet`s with the same members are equal whatever their capacities or trailing zero words.
- `Hash` for `BitSet` MUST hash `as_trimmed_words()` (its length, then each word), so `a == b` implies `hash(a) == hash(b)`. `BitArray` MAY derive `Hash`.
- `Debug` MUST print the members as a set in ascending order, like `BTreeSet`: `{3, 130}`.
- Both types MUST be `Send`, `Sync` and `Unpin`.

### 4.6 Errors and panics

Every panic follows the repository rule: `#[track_caller]`, a `# Panics` section, an actionable message naming the index and the limit, and a `try_` twin.

| Panics | When | Twin |
|---|---|---|
| `BitSet::insert(i)` | `i >= MAX_BITS` | `try_insert`, `Err(TryReserveError)` |
| `BitSet::with_capacity(bits)`, `ensure_capacity(bits)` | `bits > MAX_BITS` | `try_with_capacity`, `try_ensure_capacity` |
| `BitSet::from_words(words)` | a bit at or above `MAX_BITS` is set | `try_from_words`, `Err(TryReserveError)` |
| `FromIterator<usize>`, `Extend<usize>`, `Extend<&usize>`, `From<[usize; N]>` for `BitSet` | an index `>= MAX_BITS` | `BitSet::try_extend` (on a new set, for `FromIterator` and `From`) |
| `BitArray::insert(i)`, `remove(i)` | `i >= CAPACITY` | `try_insert`, `try_remove`, `Err(BitIndexError)` |

- Every `BitSet::try_*` returns the existing `TryReserveError`, for an index or capacity past `MAX_BITS` and for allocation failure, since both mean "the capacity overflowed". Its constructor from `indexmap`'s error stays; private constructors for these two cases are added.
- `BitIndexError` is a new public error in `collections::error`, used by `BitArray`, re-exported at `engine::collections`, with `index() -> usize` and `capacity() -> usize` accessors, `Display` (for example `bit index 70 is out of range: this set holds bits 0 to 63`), `Error`, `Debug`, `Clone`, `PartialEq` and `Eq`. It is `#[non_exhaustive]`.
- Example panic message: `BitArray::insert: bit index 70 is out of range for BitArray<1>, which holds bits 0 to 63; use a larger WORDS, or BitSet if the size is not known at compile time`.
- `remove` and `contains` on `BitSet`, and `contains` on `BitArray`, never panic. No other public item panics, apart from allocation failure during growth, as with `Vec`.

### 4.7 Heap allocation (ADR 0011)

- `BitArray` never allocates.
- After `BitSet::with_capacity(n)` or `ensure_capacity(n)`, every `insert(i)` with `i < n`, every `remove`, `contains`, `first`, `last`, `len`, iteration, `clear`, `intersect_with` and `difference_with`, every subset, superset and disjointness test, and every `union_with` or `symmetric_difference_with` whose argument's words fit in `n` bits, MUST NOT allocate.
- `clone` allocates once, sized to `as_words().len()`.

### 4.8 Tests, benchmarks, fuzzing and documents

**Tests.**

- A proptest model in `libs/engine/src/collections/tests/` drives random operation sequences (every method of §4.3 and §4.4, with indices drawn from near zero, near word boundaries, and near `CAPACITY`) against a `BTreeSet<usize>`, and after each operation compares the members, `len`, `first`, `last`, forward and backward iteration, and `contains` for each index in a window around the members.
- Set algebra and subset, superset and disjointness tests on pairs of sets of different lengths, against `BTreeSet`'s operations.
- A golden test of the layout of §4.2: a fixed set of indices produces a literal word slice.
- Equality and hashing across different capacities and trailing zero words, as in the third example of §3.
- `tests/allocations.rs` (`dhat`), for §4.7.
- `#[should_panic(expected = ...)]` tests for every panic of §4.6, and a panic-hook test that the reported location is the caller's line.
- Compile-time `Send + Sync + Unpin` assertions for both types and both iterators.

**Fuzzing.** A new target, `fuzz/fuzz_targets/bit_set_ops.rs`, applies operation sequences to a `BitSet` and a `BitArray<4>` and checks them against a `BTreeSet<usize>` model. Unlike M3.1, which wraps a backend, this chunk owns the bit arithmetic, so a fuzz target is worth adding. It is added to the nightly fuzz job's target list.

**Benchmarks.**

- `bench/regress/benches/collections.rs` (Gungraun, gated at 5% like the existing benchmarks, each above the gate's 10,000-instruction minimum): inserting 4,096 indices into a `BitSet` made `with_capacity(4_096)`, `contains` over 4,096 indices at 50% occupancy, iterating that set, `union_with` and `is_subset` on two 262,144-bit sets (smaller sets fall below the instruction minimum), and `insert`, `contains` and iteration on a `BitArray<4>`.
- `bench/compare/benches/collections.rs` (wall clock, report only): the same operations on `BitSet`, `fixedbitset::FixedBitSet`, `BTreeSet<usize>` and `Vec<bool>`, at 1%, 50% and 99% occupancy. `fixedbitset` is added as a dev-dependency of `bench/compare` only, and MUST pass `cargo xtask deny` (it is `MIT OR Apache-2.0`).

**xtask.** `collections` is already a triaged mutants module, so the new files fall under it; the new missed mutants and timeouts are triaged in `docs/mutation-triage.md`.

**Documents.**

- The `engine::collections` module documentation: a paragraph on the two bitsets and two rows in "Choosing a collection" (a set of small indices: `BitSet`, or `BitArray` when the size is known at compile time).
- `engine`'s module map and `README.md`: the `collections` row names the bitsets.
- `docs/research/data-structures.md`: the Bitset entry's **Sources** line lists the sources of §11, and its **Repo fit** gains `in repo`.
- `docs/roadmap.md`: the M3.14 row.
- `CHANGELOG.md` records the chunk.

## 5. Requirements

- **M3.14-R1** `engine::collections::bit_set` MUST exist as in §4.1, with `BitSet`, `BitArray` and `BitIndexError` re-exported at `engine::collections` and absent from `engine::prelude`, using one private module of word routines for both types, and with no new normal dependency.
- **M3.14-R2** The words MUST be `u64` in the layout of §4.2 on every target, and `as_words`, `as_trimmed_words` and `from_words` MUST behave as §4.2 states.
- **M3.14-R3** `BitSet` MUST provide the items of §4.3 with the stated signatures and semantics, `BitSet::MAX_BITS` MUST be `u32::MAX as usize` on every target, and every operation MUST agree with a `BTreeSet<usize>` model.
- **M3.14-R4** `BitArray` MUST provide the items of §4.4 with the stated signatures and semantics, and every operation MUST agree with a `BTreeSet<usize>` model.
- **M3.14-R5** Iteration MUST be ascending forwards and descending backwards, as in §4.5.
- **M3.14-R6** `PartialEq`, `Eq` and `Hash` MUST depend only on the members (§4.5), and `a == b` MUST imply `hash(a) == hash(b)`.
- **M3.14-R7** `Debug` output MUST follow §4.5, and every public type MUST implement `Debug`.
- **M3.14-R8** Both types and both iterators MUST be `Send`, `Sync` and `Unpin`.
- **M3.14-R9** Every panic of §4.6 MUST have `#[track_caller]`, a `# Panics` section, the actionable message of §4.6 and its `try_` twin, and no other public item may panic apart from allocation failure.
- **M3.14-R10** Heap allocation MUST follow §4.7.
- **M3.14-R11** Every public item MUST be documented, every public type MUST have a runnable doctest, and the module documentation MUST state the layout of §4.2, `MAX_BITS`, and that equality ignores capacity.
- **M3.14-R12** `engine` MUST build in `cargo xtask no-std`, and its tests MUST pass in `cargo xtask wasm`.
- **M3.14-R13** The fuzz target of §4.8 MUST exist, MUST be in the nightly fuzz job, and MUST run for at least 10 minutes locally without a failure before the pull request is marked ready.
- **M3.14-R14** The benchmarks of §4.8 MUST exist, and `cargo xtask bench-regress` MUST pass.
- **M3.14-R15** `cargo xtask mutants --module collections` MUST have been run, and every new missed mutant and timeout triaged in `docs/mutation-triage.md`.
- **M3.14-R16** The document changes of §4.8 MUST be made.
- **M3.14-R17** `cargo xtask deny` MUST pass without changes to `deny.toml`, and `cargo xtask ci` MUST pass.

## 6. Cross-cutting concerns

- **Determinism:** the types are deterministic by construction: no hashing, no randomness, no floats. The two ways a bitset can depend on the platform are closed off: the word type is `u64`, not `usize` (R2), and the largest index is the same constant everywhere (R3). Iteration order is ascending, so it does not depend on the operation sequence at all, unlike `IndexSet`. `Hash` ignores trailing zero words, so equal sets hash equally whatever their growth history (R6). `u64::trailing_zeros`, `leading_zeros` and `count_ones` are integer intrinsics, not the banned float methods.
- **`unsafe`:** none, and no dependency with `unsafe` added to `engine`. `fixedbitset` is a dev-dependency of `bench/compare` only.
- **Concurrency:** n/a, because neither type has interior mutability; both are `Send` and `Sync` (R8). No atomic variant (§2).
- **Heap allocation:** §4.7, checked with `dhat` (R10). `BitArray` is the no-heap option for code that must not allocate.
- **`no_std` and wasm:** `BitSet` needs `alloc`, which `engine::collections` already uses; `BitArray` needs only `core` (R12). The 32-bit question is settled by R2 and R3, and the wasm32 test run checks it.
- **Frozen formats:** none. The word layout of §4.2 is a public, documented contract, so changing it is a breaking API change, but nothing is serialized and no golden value outside this module depends on it.
- **Public API changes:** a new module `engine::collections::bit_set`, two re-exported types and one new error type; `TryReserveError` gains a private constructor. No existing public item changes.
- **FFI exposure:** n/a, because nothing crosses the C ABI in M3.
- **Performance:** `cargo xtask bench-regress` gates the new Gungraun benchmarks. The wall-clock comparison against `fixedbitset`, `BTreeSet` and `Vec<bool>` is reported in the pull request with a saved baseline and the noise floor (`cargo xtask bench-report`). `fixedbitset` uses SIMD on x86 and wasm32 for set algebra, so this chunk may be slower there, and the pull request says by how much.
- **Documentation:** item docs and doctests (R11), the module and README rows, the catalog's sources, this spec and its plan.
- **Policy changes:** none.

## 7. Alternatives considered

- **Re-export `bitvec`.** Rejected: its default storage is `BitVec<usize, Lsb0>`, so word width follows pointer width unless every use names a type parameter; it puts a foreign, generic-heavy type (`BitVec<T, O>`, `BitSlice`) in `engine`'s public API, against M3.1's rule that no public signature names a backend type and against readable names; it brings four dependencies (`funty`, `radium`, `tap`, `wyz`) and a public `unsafe` surface; and its scope (bit-addressed memory, bit order, bitfields, atomics) is far beyond a set of indices.
- **Wrap `fixedbitset` privately**, as M3.1 wraps `indexmap`. The strongest alternative: `MIT OR Apache-2.0`, used by `bevy_ecs` and petgraph, with SIMD set algebra. Rejected: its public `Block` type is `usize`, so word-level access for #44 and the allocators would be pointer-width words or need converting, and wrapping it would not save the hard part, because there is no hard part. Unlike a hash table, a bitset is a few hundred lines of straightforward code, and owning it lets the word layout be the contract. It stays as the benchmark baseline, and as the place to look if the SIMD gap turns out to matter.
- **`bit-set` and `bit-vec`.** Their default block is `u32`, so storage is the same on every platform, but the inherent constructors exist only for `BitSet<u32>`, the API is generic over a `BitBlock` trait, and `no_std` support is not documented. Rejected for the same API reason as `bitvec`, and because owning the code costs little.
- **One type with a runtime-fixed capacity** (as `fixedbitset` does: inserting past `len` panics until `grow` is called). Rejected for `BitSet` by the maintainer in favour of growing on insert; the compile-time-sized `BitArray` covers the case where going past the end is a bug.
- **Size `BitArray` in bits** (`BitArray<128>`). Rejected because stable Rust cannot compute the array length `BITS.div_ceil(64)` from a const parameter (it needs the unstable `generic_const_exprs`). A bit-sized wrapper can be added when that stabilizes, without breaking `BitArray<WORDS>`.
- **`u32` indices.** They would make the limit part of the type and match `GenId`'s index, but every slice index would need a cast. Rejected by the maintainer in favour of `usize` with `MAX_BITS`.
- **No index limit.** Rejected: the largest insertable index would then depend on pointer width (and on how much memory the target can address), which the determinism rules forbid.
- **A cached member count**, making `len()` O(1). Rejected for now: it adds an invariant that every set-algebra operation must maintain, for a method the expected users (masks, free bins) rarely call; `is_empty` is what hot code asks. Revisit if a profile shows `len` hot.
- **Keeping words trimmed** (no trailing zero words, so `as_words` is canonical). Rejected: removing the highest member would then scan backwards for the new end, making `remove` O(words) in the worst case. `as_trimmed_words` gives the canonical form on demand.
- **Mutable word access.** Rejected: it would let callers set bits at or above `MAX_BITS`; `from_words` covers building a set from words, and #44 only reads.
- **Names.** `FixedBitSet` for the inline type clashes with `fixedbitset`'s type, which the comparison benchmarks import; `InlineBitSet` was the other candidate. `BitArray`/`BitSet` mirrors `array`/`Vec`. `BitVec` was rejected for the growable type because it reads as a sequence of bools, not a set.
- **No fuzz target**, as in M3.1. Rejected: M3.1 wrapped a tested backend, while this chunk writes the bit arithmetic, where word-boundary mistakes are the expected bug.

## 8. Verification

| Requirement | Verified by |
|---|---|
| R1 | `cargo doc` and review of `engine::collections` and `engine::prelude`; `cargo tree -p engine -e normal --depth 1` unchanged; review that both types call the shared word module |
| R2 | golden layout test (§4.8); the test suite under `cargo xtask wasm` (32-bit) and on the 64-bit CI hosts; unit tests of `as_trimmed_words` and `from_words` |
| R3 | proptest model for `BitSet` against `BTreeSet<usize>`; a unit test that `MAX_BITS == u32::MAX as usize`; unit tests of `ensure_capacity`, `shrink_to_fit` and growth |
| R4 | proptest model for `BitArray<1>`, `BitArray<3>` and `BitArray<0>` against `BTreeSet<usize>` |
| R5 | the model tests compare forward and reversed iteration; a test interleaving `next` and `next_back` |
| R6 | proptests: equality against the model's equality across different capacities and trailing zeros; equal sets hash equally (with a hasher that records its input) |
| R7 | unit tests of `Debug` output; the `missing_debug_implementations` lint |
| R8 | compile-time assertions in a unit test |
| R9 | `#[should_panic(expected = ...)]` tests; panic-hook location test; review of `# Panics` sections |
| R10 | `tests/allocations.rs` (`dhat`): zero blocks for every operation listed in §4.7 |
| R11 | documentation review (CONTRIBUTING.md "Reviews before a pull request"); `cargo xtask doc` |
| R12 | `cargo xtask no-std`; `cargo xtask wasm` |
| R13 | `fuzz/fuzz_targets/bit_set_ops.rs`; the nightly job's target list; the local `cargo xtask fuzz` summary in the pull request |
| R14 | `cargo xtask bench-regress` in the devcontainer; `cargo xtask bench-report` output with a saved baseline and the noise floor in the pull request |
| R15 | `docs/mutation-triage.md`; the `cargo xtask mutants --module collections` summary in the pull request |
| R16 | documentation review; relative links resolve |
| R17 | `cargo xtask deny`; `cargo xtask ci` locally; the pull request's CI run |

## 9. Done when

- [ ] `cargo xtask ci` passes locally and CI is green on all three platforms. (`cargo xtask ci`, `wasm`, `no-std` and `deny` pass locally; CI on the pull request is pending when this is written.)
- [x] The model tests pass for both types, including under `cargo xtask wasm` (`bit_set_matches_model`, `bit_array_{0,1,3}_matches_model`).
- [x] The fuzz target ran for 10 minutes without a failure (`bit_set_ops`: 4,204,841 runs in 601 s).
- [x] `cargo xtask bench-regress` passes (aarch64 container run: no regression, six new benchmarks of 18,743 to 172,580 instructions), and the wall-clock comparison against `fixedbitset`, `BTreeSet` and `Vec<bool>` is in the pull request with its baseline and noise floor.
- [x] The mutation triage is recorded ([docs/mutation-triage.md](../../../mutation-triage.md)).
- [x] The eight reviews in CONTRIBUTING.md are recorded in the pull request.
- [x] The spec is `Implemented` and the plan `Historical`, with Deviations filled in, and #42's box in #38 is ticked.

## 10. Open questions

None blocking. Deferred, each to a named place:

- **Space reporting** (`HeapSize` for `BitSet`, bytes per element): M3.12 (#41), as an addition to its retrofit list. `BitSet`'s heap use is `capacity() / 8` bytes; `BitArray` has none.
- **Lazy set-algebra iterators** (`intersection`, `union`, `difference` yielding indices without allocating): the first chunk that joins masks without a scratch set, M4 at the latest, or the hierarchical bitset (#44) if it covers the need.
- **Operations between `BitSet` and `BitArray`, range operations, `iter_from`, `Ord`:** the chunk that first needs one.
- **A cached member count:** if a profile shows `len` hot (§7).
- **A bit-sized `BitArray`:** when `generic_const_exprs` stabilizes (§7).
- **Serialization and reflection:** M5. Debugger visualizers: M4.

## 11. Prior art and sources

Read for this spec:

- [`fixedbitset` 0.5.9 documentation](https://docs.rs/fixedbitset/latest/fixedbitset/): a runtime-sized bitset with `pub type Block = usize` and SIMD set algebra on x86 and wasm32; used by `bevy_ecs` 0.20 for its component access sets.
- [`bitvec` 1.1.1 documentation](https://docs.rs/bitvec/latest/bitvec/): bit-addressed memory with storage and order type parameters, defaulting to `<usize, Lsb0>`.
- [`bit-set` 0.11.1 documentation](https://docs.rs/bit-set/latest/bit_set/): a `BitSet<B: BitBlock = u32>`, whose default storage is the same on every platform.
- [`hibitset` 0.6.4 documentation](https://docs.rs/hibitset): its top layer is one `usize`, so its largest index is 2^20 on 32-bit targets and 2^24 on 64-bit ones, which is why this spec fixes `MAX_BITS` (and why #44 must fix its layer sizes).
- [`bevy_ecs` 0.20.1 `ComponentIdSet`](https://docs.rs/bevy_ecs/latest/bevy_ecs/query/struct.ComponentIdSet.html): an ECS whose component access sets are bitsets.

Cited for the bit techniques, to be read before the implementation relies on them:

- Henry S. Warren, Jr., *Hacker's Delight*, 2nd ed. (Addison-Wesley, 2012), chapter 2 (rightmost-bit manipulation: `x & (x - 1)` clears the lowest set bit) and chapter 5 (counting bits).
- Donald E. Knuth, *The Art of Computer Programming*, Vol. 4A, §7.1.3, "Bitwise tricks and techniques".
- Daniel Lemire, [Iterating over set bits quickly](https://lemire.me/blog/2018/02/21/iterating-over-set-bits-quickly/) (2018): iterating with trailing-zero count and clearing the lowest bit, against bit-by-bit testing.
- Rust `core` documentation for [`u64::trailing_zeros`](https://doc.rust-lang.org/core/primitive.u64.html#method.trailing_zeros), `leading_zeros` and `count_ones`.
