# M3.14 plan: Bitsets (`BitSet` and `BitArray`)

- **Status:** Accepted (2026-11-14)
- **Spec:** [spec.md](spec.md). This plan argues from the spec; read both.

## 1. Approach

Eight commits on branch `m3.14-bitset`, after the spec commits already on it. The word routines and the inline `BitArray` come first, because they need only `core` and fix the layout of spec §4.2 that everything else uses. `BitSet` follows in one commit for its single-bit operations, and set algebra for both types goes in another, so the model test grows with the API. Allocation checks, the fuzz target, benchmarks, mutation triage and documents close the chunk.

Choices the spec leaves to implementation:

- **Module layout.** `libs/engine/src/collections/bit_set.rs` (module docs, `BitSet`, re-exports of `BitArray`, `Iter` and `IntoIter`), `bit_set/array.rs` (`BitArray`), `bit_set/iter.rs` (`Iter`, `IntoIter` and the private `Cursor` they share), `bit_set/words.rs` (the private word routines of spec §4.1). `BitIndexError` goes in `collections/error.rs`. Unit model tests go in `collections/tests/bit_set_model.rs` and `bit_array_model.rs`, and the layout golden test in `collections/tests/golden.rs`, next to M3.1's.
- **One cursor for both iterators.** `Cursor { front: usize, back: usize, front_bits: u64, back_bits: u64 }` holds the next word index from each end and the bits of that word not yet yielded. `Iter<'a>` is `{ words: &'a [u64], cursor: Cursor }` and `IntoIter` is `{ words: Vec<u64>, cursor: Cursor }`. Both call `cursor.next(&words)` and `cursor.next_back(&words)`. When `front == back`, the two ends share one word: `next` takes the lowest bit of `front_bits` and clears it in `back_bits` too, and `next_back` takes the highest bit and clears it in both. That is the only subtle part of the iterator (review focus 4).
- **Growth.** `insert(i)` computes `needed = i / 64 + 1`. If `needed > words.len()`, it calls `words.try_reserve(needed - words.len())` (amortized growth, as `Vec` does) and then `words.resize(needed, 0)`. `ensure_capacity(bits)` calls `try_reserve_exact` for `bits.div_ceil(64) - words.len()` words and does not change `words.len()`. `union_with` and `symmetric_difference_with` grow `self` to `other.as_trimmed_words().len()`, not to `other.as_words().len()`, so trailing zeros in `other` never cost an allocation (spec §4.7).
- **`capacity()`** is `words.capacity().saturating_mul(64).min(MAX_BITS)`, computed in a private `fn bits_for_words(words: usize) -> usize`, because `capacity × 64` overflows `usize` on a 32-bit target (review focus 1).
- **The `MAX_BITS` check in `from_words`** finds the last non-zero word and computes its highest bit index as `u64` (`word_index as u64 * 64 + (63 - leading_zeros)`), then compares it with `MAX_BITS as u64`. A private `fn highest_bit_u64(words: &[u64]) -> Option<u64>` in `words.rs` does this, so it can be unit-tested without a 512 MiB slice.
- **`TryReserveError`** gains `pub(crate) const fn capacity_overflow() -> Self` and `pub(crate) fn from_alloc(_: alloc::collections::TryReserveError) -> Self`. The existing `from_inner` is unchanged.
- **`BitArray`'s `Default`** is written by hand as `Self::new()`, because `[u64; N]: Default` holds only for `N <= 32`. `Clone`, `Copy`, `PartialEq`, `Eq` and `Hash` are derived; `Debug` is written by hand (spec §4.5).
- **`BitSet`'s `PartialEq` and `Hash`** compare and hash `as_trimmed_words()`; `Hash` calls `<[u64] as Hash>::hash`, which writes the length and then each word.
- **Panic messages** (spec §4.6): `BitSet::insert` uses `"BitSet::insert: bit index {i} is at or above BitSet::MAX_BITS ({MAX_BITS}); use try_insert to handle it"`. `with_capacity`, `ensure_capacity`, `from_words`, `FromIterator`, `Extend` and `From` follow the same pattern and name their `try_` twin. `BitArray::insert` and `remove` use the spec's example message. Each goes through a `#[cold] #[track_caller] fn ... -> !`.

**Global constraints**

- Every commit passes `cargo xtask ci --quick`. Step 6 also passes `cargo xtask deny`, with `deny.toml` unchanged (R17).
- No new normal dependency of `engine`; `cargo tree -p engine -e normal --depth 1` is unchanged (R1). `fixedbitset = "0.5.9"` is a dev-dependency of `bench/compare` only.
- Words are `u64` everywhere; no `usize` word type and no `cfg(target_pointer_width)` in library code. `BitSet::MAX_BITS` is `u32::MAX as usize`.
- No `unsafe`, no mutable word access, no `FromIterator` or `From<[usize; N]>` for `BitArray`, nothing in `engine::prelude`.
- Every documented panic has `#[track_caller]`, a `# Panics` section, the message above, and its `try_` twin (R9).
- Commit messages are conventional, end with ` (M3.14)`, and end with the session's attribution line. Stage by explicit path.
- Stop and ask the maintainer before the first push and before opening the pull request.

**Review focus** (inputs the spec implies but no requirement's test pins on its own; each has a test in the step that owns the code)

1. **`capacity()` on a 32-bit target.** A `Vec<u64>` of `2^26` words would report `2^32` bits, which overflows `usize`. Step 2: `bits_for_words_saturates_at_max_bits` (`bits_for_words(usize::MAX / 8) == MAX_BITS`, `bits_for_words(3) == 192`), run under `cargo xtask wasm` too.
2. **Word boundaries.** Indices 0, 63, 64, 127 and 128 land in the right word and bit, and `first`, `last` and iteration cross the boundary. Step 1: `word_boundary_indices` for `BitArray<3>`; Step 2: the same for `BitSet`.
3. **Set algebra on sets of different lengths.** `intersect_with` a shorter set clears `self`'s words past `other`'s end; `difference_with` a shorter set leaves them alone; `union_with` a longer set that ends in zero words does not allocate. Step 3: `intersect_with_shorter_clears_tail`, `difference_with_shorter_keeps_tail`; Step 4: `union_with_trailing_zeros_does_not_allocate`.
4. **Double-ended iteration meeting in one word.** Alternating `next` and `next_back` yields each member exactly once, whether the ends meet in one word or across words. Step 1: `double_ended_meets_once` (members `{1, 5, 9, 63}` in `BitArray<1>`, and `{1, 70, 140}` in `BitArray<3>`, alternating calls collect every member once and then both ends return `None`).
5. **Indices at the limit, without allocating.** `try_insert(usize::MAX)` is `Err` on 32-bit and 64-bit targets; `try_insert(BitSet::MAX_BITS)` is `Err`; neither changes the set or allocates. Step 2: `try_insert_at_the_limit_is_an_error`, run under `cargo xtask wasm` too; Step 4: the allocation check.

## 2. Steps

### Step 1: word routines, `BitArray` and the iterator (R2, R4, R5, R7, R8, R9 for `BitArray`)

- **Files:** `libs/engine/src/collections.rs` (`pub mod bit_set;`, re-exports `BitArray`, `BitIndexError`), `collections/error.rs`, `collections/bit_set.rs` (new; module docs and re-exports only in this step), `bit_set/words.rs`, `bit_set/array.rs`, `bit_set/iter.rs` (new), `collections/tests/mod.rs`, `collections/tests/bit_array_model.rs` (new), `collections/tests/golden.rs`, `libs/engine/tests/collections_panics.rs`, `libs/engine/tests/collections_auto_traits.rs`, `docs/roadmap.md` (row status `In progress`).
- **Interfaces produced:**
  - `words.rs` (all `pub(super)`): `const WORD_BITS: usize = 64`; `fn locate(i: usize) -> (usize, u64)` (word index, mask); `fn count(&[u64]) -> usize`; `fn first(&[u64]) -> Option<usize>`; `fn last(&[u64]) -> Option<usize>`; `fn trimmed(&[u64]) -> &[u64]`; `fn highest_bit_u64(&[u64]) -> Option<u64>`; `fn is_subset(a: &[u64], b: &[u64]) -> bool` and `fn is_disjoint(a: &[u64], b: &[u64]) -> bool`, both treating words past either end as zero; `fn union_into`, `intersect_into`, `difference_into`, `xor_into` (`dst: &mut [u64], src: &[u64]`, where `src.len() <= dst.len()`; `intersect_into` also zeroes `dst[src.len()..]`).
  - `iter.rs`: `pub struct Iter<'a>` (`Iterator<Item = usize>`, `DoubleEndedIterator`, `FusedIterator`, `Clone`, `Debug`) and `pub(crate) fn Iter::new(&'a [u64]) -> Iter<'a>`; the private `Cursor` of §1.
  - `error.rs`: `#[non_exhaustive] pub struct BitIndexError { index: usize, capacity: usize }` with `pub const fn index(&self) -> usize`, `pub const fn capacity(&self) -> usize`, `Display` ("bit index {index} is out of range: this set holds bits 0 to {capacity - 1}", or "this set holds no bits" when `capacity == 0`), `Error`, and derived `Debug`, `Clone`, `PartialEq`, `Eq`; `pub(crate) const fn new(index, capacity) -> Self`.
  - `array.rs`: `pub struct BitArray<const WORDS: usize>` with the API of spec §4.4 except set algebra and the subset, superset and disjointness tests (Step 3): `pub const CAPACITY: usize`, `pub const fn new()`, `pub const fn from_words([u64; WORDS])`, `pub const fn as_words(&self) -> &[u64; WORDS]`, `len`, `is_empty`, `clear`, `insert(usize) -> bool`, `try_insert(usize) -> Result<bool, BitIndexError>`, `remove(usize) -> bool`, `try_remove(usize) -> Result<bool, BitIndexError>`, `pub const fn contains(&self, usize) -> bool`, `first`, `last`, `iter(&self) -> Iter<'_>`, `IntoIterator for &BitArray<WORDS>`.
- **Test first:**
  - `tests/bit_array_model.rs`: an `Op` enum `Insert(i)`, `Remove(i)`, `TryInsert(i)`, `TryRemove(i)`, `Clear`, `RoundTripWords` (`BitArray::from_words(*a.as_words())`), with `i` drawn from `prop_oneof![0..8, 56..72, 120..136, 184..200]`, against a `BTreeSet<usize>` that rejects `i >= CAPACITY`. Proptests `bit_array_1_matches_model`, `bit_array_3_matches_model` and `bit_array_0_matches_model` (ops of length `0..200`; for `BitArray<0>` every insert is an error). After each op it compares the return value (`insert`, `try_insert` and so on), `iter()` with the model's ascending order, `iter().rev()` with the descending order, `len`, `first`, `last`, and `contains(j)` for `j` in `0..CAPACITY + 64`.
  - `tests/golden.rs` `bit_array_word_layout`: `{0, 1, 63, 64, 130}` in `BitArray<3>` gives `as_words() == &[0x8000_0000_0000_0003, 0x1, 0x4]`.
  - Unit tests in `array.rs`: `word_boundary_indices` and `double_ended_meets_once` (review focus 2 and 4); `debug_format` (`{3, 64}` and `{}`); `default_is_empty_for_large_words` (`BitArray::<40>::default().is_empty()`, which would fail to compile with a derived `Default`); `bit_index_error_display` (both texts).
  - `collections_panics.rs`: `bit_array_insert_out_of_range_panics_at_caller` and `bit_array_remove_out_of_range_panics_at_caller`. Each checks that the location ends with `tests/collections_panics.rs` and that the message contains `"bit index 64"`, `"BitArray<1>"` and `"use a larger WORDS"`.
  - `collections_auto_traits.rs`: `BitArray<2>` and `bit_set::Iter<'static>` are `Send + Sync + Unpin`.
  - Run `cargo xtask test`: compile failure (items missing).
- **Change:** implement the interfaces. Each public type gets a doctest; the `BitArray` doctest is the one in spec §3.
- **Verify:** `cargo xtask test` passes; `cargo xtask no-std` builds; `cargo xtask ci --quick` passes.
- **Commit:** `feat(collections): add BitArray and the shared bit routines (M3.14)`

### Step 2: `BitSet` (R2, R3, R5 to R9 for `BitSet`)

- **Files:** `collections/bit_set.rs`, `bit_set/iter.rs` (`IntoIter`), `collections/error.rs` (`TryReserveError` constructors), `collections.rs` (re-export `BitSet`), `collections/tests/bit_set_model.rs` (new), `collections/tests/golden.rs`, `libs/engine/tests/collections_laws.rs`, `collections_panics.rs`, `collections_auto_traits.rs`.
- **Interfaces produced:** `pub struct BitSet { words: Vec<u64> }` with spec §4.3's API except set algebra and the subset, superset and disjointness tests (Step 3): `pub const MAX_BITS: usize = u32::MAX as usize`; `pub const fn new()`; `with_capacity(bits)`, `try_with_capacity(bits) -> Result<Self, TryReserveError>`; `from_words(&[u64])`, `try_from_words(&[u64]) -> Result<Self, TryReserveError>`; `len`, `is_empty`, `capacity`, `ensure_capacity(bits)`, `try_ensure_capacity(bits) -> Result<(), TryReserveError>`, `shrink_to_fit`, `clear`; `insert(usize) -> bool`, `try_insert(usize) -> Result<bool, TryReserveError>`, `remove(usize) -> bool`, `contains(&self, usize) -> bool`; `first`, `last`, `iter`; `as_words(&self) -> &[u64]`, `as_trimmed_words(&self) -> &[u64]`; `try_extend(&mut self, impl IntoIterator<Item = usize>) -> Result<(), TryReserveError>`; `pub struct IntoIter` (the traits of `Iter`, without `Clone`); `IntoIterator` for `BitSet` and `&BitSet`; `Clone`, `Default`, `Debug`, `PartialEq`, `Eq`, `Hash`, `FromIterator<usize>`, `Extend<usize>`, `Extend<&usize>`, `From<[usize; N]>`; the private `bits_for_words` of §1.
- **Test first:**
  - `tests/bit_set_model.rs`: an `Op` enum `Insert(i)`, `TryInsert(i)`, `Remove(i)`, `Clear`, `EnsureCapacity(n in 0..4_300)`, `ShrinkToFit`, `TryExtend(Vec<usize>)`, `RoundTripWords` (`from_words(a.as_words())`), `CloneSwap` (replace the set with its clone). Indices come from `prop_oneof![0..8, 56..72, 120..136, 4_000..4_200]`, against a `BTreeSet<usize>`. Proptest `bit_set_matches_model` (ops of length `0..200`) checks what the `BitArray` model checks, with `contains(j)` over `0..4_300`, plus `capacity() >= 64 * as_words().len()` and `as_trimmed_words()` against words built from the model.
  - `tests/golden.rs` `bit_set_word_layout`: `{0, 1, 63, 64, 130}` gives `as_words() == [0x8000_0000_0000_0003, 0x1, 0x4]`; after `remove(130)`, `as_words().len() == 3` and `as_trimmed_words() == [0x8000_0000_0000_0003, 0x1]`.
  - Unit tests in `bit_set.rs`: `word_boundary_indices`; `bits_for_words_saturates_at_max_bits` and `try_insert_at_the_limit_is_an_error` (review focus 1 and 5: `try_insert(usize::MAX)` and `try_insert(BitSet::MAX_BITS)` are `Err`, and `len() == 0` and `as_words().is_empty()` afterwards); `max_bits_is_u32_max` (`BitSet::MAX_BITS as u64 == u64::from(u32::MAX)`); `contains_past_the_end_is_false` (`contains(10_000)` on `{3}` leaves `as_words().len() == 1`); `try_extend_stops_at_the_failing_index` (`[1, 2, usize::MAX, 4]` gives `Err`, and the set is `{1, 2}`); `highest_bit_u64_and_the_limit` (in `words.rs`: a slice whose last non-zero word is at index 3 with bit 5 set gives `Some(197)`; and in `bit_set.rs`, the private `fn exceeds_max_bits(highest: u64) -> bool` is `false` for `u64::from(u32::MAX) - 1` and `true` for `u64::from(u32::MAX)`, the index of bit 63 of word `2^26 - 1`); `debug_format`; `shrink_to_fit_drops_trailing_zeros`.
  - `collections_laws.rs` (proptest over two `Vec<usize>` of indices below 4,200, each turned into a `BitSet` after an arbitrary `ensure_capacity` and an arbitrary number of high inserts that are then removed): `bit_set_eq_matches_model_eq` (`a == b` exactly when their `BTreeSet`s are equal); `equal_bit_sets_hash_equal` (with `foldhash::fast::FixedState::with_seed(0).hash_one`, as the M3.1 laws do).
  - `collections_panics.rs`: `bit_set_insert_at_max_bits_panics_at_caller` (message contains `"MAX_BITS"` and `"use try_insert"`); `bit_set_from_iter_at_max_bits_panics_at_caller` (`[BitSet::MAX_BITS].into_iter().collect::<BitSet>()`, message names `try_extend`); `#[cfg(target_pointer_width = "64")] bit_set_with_capacity_past_max_bits_panics_at_caller` (`with_capacity(BitSet::MAX_BITS + 1)`, which panics before allocating); `#[cfg(target_pointer_width = "64")] bit_set_from_words_past_max_bits_panics_at_caller`, which builds `vec![0_u64; 1 << 26]` (calloc'd, so its pages stay unmapped), sets bit 63 of the last word (index `2^32 - 1 = MAX_BITS`), and expects a panic naming `try_from_words` before any copy.
  - `collections_auto_traits.rs`: `BitSet` and `bit_set::IntoIter`.
  - Run `cargo xtask test`: compile failure.
- **Change:** implement the interfaces as §1 describes. Doctests on `BitSet` and `IntoIter`, including the first and third examples of spec §3.
- **Verify:** `cargo xtask test` passes; `cargo xtask wasm` passes (review focus 1 and 5 run on a 32-bit target); `cargo xtask ci --quick` passes.
- **Commit:** `feat(collections): add BitSet (M3.14)`

### Step 3: set algebra for both types (R3, R4)

- **Files:** `collections/bit_set.rs`, `bit_set/array.rs`, both model test files.
- **Interfaces produced:** on `BitSet`, each taking `&BitSet`: `union_with`, `intersect_with`, `difference_with`, `symmetric_difference_with` (`&mut self`), and `is_subset`, `is_superset`, `is_disjoint` (`&self`, returning `bool`). The same on `BitArray<WORDS>`, each taking `&BitArray<WORDS>`. All of them go through the `words.rs` routines of Step 1.
- **Test first:**
  - Both model tests gain `UnionWith(Vec<usize>)`, `IntersectWith(Vec<usize>)`, `DifferenceWith(Vec<usize>)` and `SymmetricDifferenceWith(Vec<usize>)` (the argument is built from the vector, with indices from the same strategy, so its length often differs from `self`'s), checked against `BTreeSet`'s `union`, `intersection`, `difference` and `symmetric_difference`. After each op, `is_subset`, `is_superset` and `is_disjoint` against a second generated set are compared with the model's.
  - Unit tests in `bit_set.rs`: `intersect_with_shorter_clears_tail` (`{1, 200}` intersected with `{1}` gives `{1}`, with `as_words().len()` still 4); `difference_with_shorter_keeps_tail` (`{1, 200}` minus `{1}` gives `{200}`); `subset_ignores_trailing_zeros` (`{1}` with capacity 4,096 is a subset of a fresh `{1}`, and the reverse); and the second example of spec §3 as a doctest.
  - Run `cargo xtask test`: compile failure.
- **Change:** implement the interfaces.
- **Verify:** `cargo xtask test` passes; `cargo xtask ci --quick` passes.
- **Commit:** `feat(collections): add set algebra to BitSet and BitArray (M3.14)`

### Step 4: allocation checks and platform builds (R10, R12)

- **Files:** `libs/engine/tests/collections_allocations.rs`.
- **Test first** (with the file's `fewest_blocks` helper):
  - `bit_set_with_capacity_never_allocates`: setup `BitSet::with_capacity(4_096)` and an argument `{0, 4_095}`. The work inserts 2,048 indices below 4,096, runs `contains` on 4,096 indices, iterates forwards and backwards, calls `first`, `last` and `len`, removes 1,024 indices, runs `union_with`, `symmetric_difference_with`, `intersect_with`, `difference_with`, `is_subset`, `is_superset` and `is_disjoint` against the argument, then `clear`, and inserts 2,048 indices again. Expected: 0 blocks.
  - `bit_set_ensure_capacity_never_allocates`: the same work after `new()` and `ensure_capacity(4_096)`.
  - `union_with_trailing_zeros_does_not_allocate` (review focus 3): `self` is `{1}` with capacity 64; `other` is `{2}` built with capacity 4,096 and with index 4,000 inserted and then removed. Expected: 0 blocks.
  - `try_insert_at_the_limit_does_not_allocate` (review focus 5): `try_insert(BitSet::MAX_BITS)` on `new()`. Expected: 0 blocks.
  - `bit_array_never_allocates`: every `BitArray<4>` operation of spec §4.4. Expected: 0 blocks.
  - Run `cargo xtask test`. These test behaviour that Steps 2 and 3 already implement, so they may pass at once. If one fails, fix the code, not the test: spec §4.7 is a MUST.
- **Change:** whatever a failing test shows; expected none.
- **Verify:** `cargo xtask test` passes; `cargo xtask no-std` builds `engine` for `thumbv7em-none-eabihf`; `cargo xtask wasm` runs the tests under wasmtime; `cargo xtask ci --quick` passes.
- **Commit:** `test(collections): check that bitsets do not allocate after reserving (M3.14)`

### Step 5: fuzz target (R13)

- **Files:** `fuzz/fuzz_targets/bit_set_ops.rs` (new), `fuzz/Cargo.toml` (a `[[bin]]` entry like the others; the description becomes "cargo-fuzz targets for the genid and collections modules of engine ..."), `xtask/src/fuzz.rs` (`TARGETS` becomes `[&str; 4]` with `"bit_set_ops"` last), `.github/workflows/nightly.yml` (the comment "fuzzing of the genid module" becomes "of the genid and collections modules").
- **Interfaces:** `#[derive(Arbitrary, Debug)] enum Op { Insert(u16), Remove(u16), Contains(u16), Clear, EnsureCapacity(u16), ShrinkToFit, UnionWith(Vec<u16>), IntersectWith(Vec<u16>), DifferenceWith(Vec<u16>), SymmetricDifferenceWith(Vec<u16>), IsSubset(Vec<u16>), IsDisjoint(Vec<u16>), RoundTripWords, Reverse(u8) }`. Indices are `u16`, so they stay below 65,536 and keep the sets small. `Reverse(k)` takes `k` items from the back of `iter()` and checks them against the model. Every op is applied to a `BitSet`, to a `BitArray<4>` (with indices taken `% 256`) and to two `BTreeSet<usize>` models. After each op the target asserts that the members, `len`, `first` and `last` match.
- **Test first:** the existing xtask unit test that checks `TARGETS` against `fuzz/fuzz_targets/*.rs` fails once the file exists and `TARGETS` is not yet updated. Run `cargo xtask test` to see it fail, then update `TARGETS`.
- **Change:** as listed.
- **Verify:** `cargo xtask fuzz-build` passes; `cargo xtask fuzz bit_set_ops --time 600` runs for 10 minutes with no finding (keep the summary for the pull request); `cargo xtask ci --quick` passes.
- **Commit:** `test(fuzz): add a bitset fuzz target (M3.14)`

### Step 6: benchmarks (R14)

- **Files:** `bench/regress/benches/collections.rs`, `bench/compare/Cargo.toml` (dev-dependency `fixedbitset = "0.5.9"`), `bench/compare/benches/collections.rs`, `Cargo.lock`.
- **Interfaces:**
  - Gungraun, following the file's existing pattern (a setup function, a `*_work` function that is never inlined, seed `0x5EED`, indices shuffled with `Prng::shuffle`), as a new group `bit_set` added to `main!`: `bit_set_insert_4k` (4,096 shuffled indices into `with_capacity(4_096)`), `bit_set_contains_4k` (4,096 lookups at 50% occupancy), `bit_set_iter_4k` (the sum of members at 50% occupancy), `bit_set_union_256k` and `bit_set_is_subset_256k` (two 262,144-bit sets at 50% occupancy), and `bit_array_ops` (16 rounds of inserting, testing and iterating all 256 indices of a `BitArray<4>` in shuffled order).
  - Criterion: groups `bit_set_insert`, `bit_set_contains`, `bit_set_iter` and `bit_set_union`, each at 4,096 bits with occupancies of 1%, 50% and 99%, on four subjects labelled `engine::BitSet`, `fixedbitset::FixedBitSet`, `BTreeSet` and `Vec<bool>` (the label convention of M3.13). `FixedBitSet` is made with `with_capacity(4_096)`, because it does not grow on insert.
- **Test first:** the check is `cargo xtask bench-smoke`, which runs every target once without measuring. It must run the new benchmarks after the change.
- **Change:** as listed. If a benchmark falls below the gate's 10,000-instruction minimum, enlarge it and record that in §4.
- **Verify:**
  - `cargo xtask bench-smoke` passes; `cargo xtask deny` passes with `deny.toml` unchanged; `cargo xtask ci --quick` passes.
  - In the devcontainer: `cargo xtask bench-regress` passes. The merge base has no `bit_set` group; if the command fails on that rather than skipping it, record it in §4 and report it to the maintainer before changing anything in `xtask`.
  - Wall clock, under the benchmark protocol (AC power, `caffeinate`, no builds during the run, check the power log afterwards): `cargo xtask bench --save m3.14-bitset` twice, then `cargo xtask bench-report --baseline m3.14-bitset`, recording the run-to-run noise floor. The numbers go in the pull request, with the gap to `fixedbitset` on `union` stated plainly (spec §6, Performance), not in `docs/benchmarks.md`.
- **Commit:** `bench(collections): add bitset instruction-count and comparison benchmarks (M3.14)`

### Step 7: mutation testing and triage (R15)

- **Files:** `.cargo/mutants.toml` (exclusions only for mutants classed *equivalent* or *not worth a test*, each with a reason), tests in the files above for mutants classed *test gap, fixed*, `docs/mutation-triage.md` (a "Bitsets (M3.14)" subsection under `engine::collections`).
- **Test first:** `cargo xtask mutants --module collections -j 2` before any new test. Classify every missed mutant and timeout in `bit_set*` with the classes in CONTRIBUTING.md "Mutation testing". Likely survivors: `bits_for_words`'s `.min(MAX_BITS)` (caught only by the review-focus 1 test; check that it is), the `63 - leading_zeros` arithmetic in `last` and `highest_bit_u64`, and the `Display` of `BitIndexError`.
- **Change:** a test for each *test gap, fixed*; an exclusion with a reason for each *equivalent* or *not worth a test*.
- **Verify:** a second run of the same command; the subsection has a row for every remaining missed mutant and timeout, and both runs' summary tables, as the M3.1 section does. `cargo xtask ci --quick` passes.
- **Commit:** `test(collections): triage the bitset mutants (M3.14)`

### Step 8: documents (R11, R16)

- **Files:** `libs/engine/src/collections.rs` (module docs: a paragraph on the bitsets and two rows in "Choosing a collection"), `libs/engine/src/lib.rs` (the module map's `collections` row), `README.md` (the same row), `docs/research/data-structures.md` (the Bitset entry: **Sources** lists spec §11, **Repo fit** gains `in repo`), `CHANGELOG.md` (an M3.14 entry under Unreleased, in the style of M3.1's).
- **Test first:** `cargo xtask doc` and the documentation review checklist (CONTRIBUTING.md "Reviews before a pull request") against the files listed; the module docs must state the layout of §4.2, `MAX_BITS`, and that equality ignores capacity (R11). Before this step, the review finds those statements missing from `collections.rs`.
- **Change:** as listed. Before citing them as read, read the §11 bit-technique sources (*Hacker's Delight* chapters 2 and 5, Knuth §7.1.3, Lemire's post), or move them to "cited, not read" in the catalog.
- **Verify:** `cargo xtask doc` passes with no broken intra-doc links; relative links resolve; `cargo xtask ci` (full) passes.
- **Commit:** `docs(collections): document the bitsets (M3.14)`

## 3. Final verification

- `cargo xtask ci` passes locally; `cargo xtask bench-regress` passes in the devcontainer; `cargo xtask fuzz bit_set_ops --time 600` has run clean.
- Every requirement in spec §5 has its evidence from spec §8 (the spec conformance review).
- The eight reviews of CONTRIBUTING.md are run and recorded in the pull request description, with the assistant's use stated.
- Ask the maintainer before the first push and before opening the pull request. Once it is open and its checks pass, push one close-out commit: spec `Implemented (PR #n, date)`, this plan `Historical`, roadmap row `Done (PR #n)`, the spec's done-when list ticked with evidence, and #42's box ticked in #38.

## 4. Deviations

Filled in during implementation.
