# M3.17 plan: Hierarchical bitset

- **Status:** Accepted (2026-11-16)
- **Spec:** [spec.md](spec.md). This plan argues from the spec; read both. Progress and surprises go in [log.md](log.md).

## 1. Approach

Ten commits on `m3.17-hierarchical-bitset`, after the spec commits. The set's layers and single-bit operations come first, because they fix the layer rule everything else reads. One private descent cursor is then written once and used three ways: member iteration, `Join`, and the set-algebra and subset tests that skip empty ranges. Allocation checks, fuzzing, benchmarks, the space report, mutation triage and documents close the chunk.

Choices the spec leaves to implementation:

- **Files.** `libs/engine/src/collections/bit_set/hierarchical.rs` (`HierarchicalBitSet`, its unit tests), `bit_set/descent.rs` (the private cursor below), `bit_set/join.rs` (`Join`), `HierarchicalIter` in `hierarchical.rs`. `bit_set.rs` declares the modules and re-exports `HierarchicalBitSet`, `HierarchicalIter` and `Join`; `collections.rs` re-exports `HierarchicalBitSet`, `Join`, `EmptyJoinError` and `BitSetInvariantError`. Errors go in `collections/error.rs`. Model tests: `collections/tests/hierarchical_model.rs` and `collections/tests/join_model.rs`. The golden layer test is a unit test in `hierarchical.rs`, because it reads the private layers.
- **Fields.** `layers: [Vec<u64>; 5]` (layers 0 to 4) and `top: u64` (layer 5). For index `i`, layer `k`'s word is `i >> (6 * (k + 1))` and its bit is `(i >> (6 * k)) & 63`. Constants in `hierarchical.rs`: `const LAYERS: usize = 6`; the shifts are computed from `WORD_BITS` in `words.rs`, never from `usize::BITS`.
- **Lengths.** Each layer `k` in 0 to 4 is grown, when an insert needs it, to `(i >> (6 * (k + 1))) + 1` words, with `try_reserve` then `resize(_, 0)` as `BitSet::grow_to_hold` does, all five layers reserved before any is resized, so a failed reservation leaves the set unchanged. `ensure_capacity(bits)` reserves `ceil(bits / 64^(k + 1))` words in each layer `k` (so no insert below `bits` allocates in any layer) without changing lengths. `capacity()` is the existing private `bits_for_words(layers[0].capacity())`; `words_for_bits` gives the `MAX_BITS` check, as `BitSet` uses them. Both move from `bit_set.rs` to `words.rs` as `pub(super)` so the two types share them.
- **The descent cursor** (`descent.rs`, all `pub(super)`). A private trait `WordSource { fn word(&self, layer: usize, index: usize) -> u64; }`, where a word past a layer's end is `0`. `struct WordCursor { bits: [u64; 6], base: [usize; 6] }` holds, per layer, the bits of the current word not yet visited and that word's index. `WordCursor::new(top_word)` and `fn next_word(&mut self, source: &impl WordSource) -> Option<(usize, u64)>` yields each layer-0 word index under a set path of upper bits, with the source's layer-0 word, skipping zero words (a join can produce them). It reads a word of layer `k − 1` only for a set bit of layer `k`. `first_word` and `last_word` descend with `trailing_zeros` and `leading_zeros`.
  - `impl WordSource for HierarchicalBitSet`: layer 5 is `top`, else `layers[k].get(index)`.
  - `HierarchicalIter<'a> { set: &'a HierarchicalBitSet, cursor: WordCursor, base: usize, bits: u64 }` takes the lowest bit of `bits` and clears it with `bits & (bits - 1)`, and calls `next_word` when `bits` is zero.
  - `Join` holds `required`, `excluded`, a `WordCursor` and the current bits. Its private source's word at layer `k > 0` is the AND of the required sets' words; at layer 0 it is that AND with `!` of the OR of the excluded sets' layer-0 words. Its starting top word is the AND of the required `top`s.
- **Set algebra** uses the cursor over `self` (or over a two-set AND source) to visit only non-zero layer-0 words:
  - `union_with`: grow each layer to `other`'s trimmed length for that layer, then OR layer by layer. A parent bit is set exactly when either child word is non-zero, so ORing every layer keeps the rule.
  - `intersect_with`: for each of `self`'s non-zero words, AND it with `other`'s word; if it becomes zero, clear the parent bits upwards as `remove` does (a private `fn clear_word_upwards(&mut self, word: usize)`, shared with `remove`).
  - `difference_with`: the same, over the words of the two-set AND source (only words both sets have can change), with `word & !other_word`.
  - `is_subset`: over `self`'s non-zero words, `word & !other_word == 0`. `is_superset(o)` is `o.is_subset(self)`. `is_disjoint`: over the two-set AND source, every layer-0 AND is zero.
- **`len`** is `words::count(&layers[0])`; `is_empty` is `top == 0`. **Equality and `Hash`** use `words::trimmed(&layers[0])`; upper layers follow from layer 0, so they are not compared. **`Debug`** prints the members as `{a, b}`, as `BitSet` does.
- **Conversions.** `From<&BitSet>` copies `as_trimmed_words()` into layer 0, then builds layers 1 to 5 bottom-up (a private `fn rebuild_upper_layers(&mut self)`). `From<&HierarchicalBitSet> for BitSet` is `BitSet::from_words(as_trimmed_words())`, which cannot panic because of the `MAX_BITS` invariant.
- **Errors.** `#[non_exhaustive] pub struct EmptyJoinError;` with `Display` "a join needs at least one required set". `#[non_exhaustive] pub struct BitSetInvariantError { layer: usize, word: usize }`, with `layer()` and `word()` accessors and `Display` "hierarchical bitset layer {layer}, word {word}: ..." naming the broken rule (a parent bit that disagrees with its child word, a bit at or above `MAX_BITS`, or a `top` bit above 3). Both derive `Debug`, `Clone`, `PartialEq`, `Eq` and implement `Error`; both get `pub(crate)` constructors.
- **Panic messages** follow M3.14's pattern through a `#[cold] #[track_caller] fn ... -> !`: `"HierarchicalBitSet::insert: bit index {i} is at or above HierarchicalBitSet::MAX_BITS ({MAX_BITS}); use try_insert to handle it"`, and likewise for the other rows of spec §4.5; `Join::new` uses spec §4.5's example message.
- **`#[inline]`** on the hot non-generic methods (`insert`, `try_insert`, `remove`, `contains`, the iterator `next`s, the word routines), as M3.14 found after its review.

**Global constraints**

- Every commit passes `cargo xtask ci --quick`. Steps 7 and 8 also pass `cargo xtask deny` with `deny.toml` unchanged.
- No new normal dependency of `engine` (`cargo tree -p engine -e normal --depth 1` unchanged). `hibitset = { version = "0.6.4", default-features = false }`, `hi_sparse_bitset = "0.10.0"` and `dhat = "0.3"` are dev-dependencies of `bench/compare` only.
- Words are `u64`; no `usize` word type and no `cfg(target_pointer_width)` in library code (tests may use it, spec §4.8). No `unsafe`, no mutable layer access, nothing in `engine::prelude`.
- Model and fuzz indices stay below 1,048,576 (spec §4.8).
- Commit messages are conventional, end with ` (M3.17)`, and end with the session's attribution line. Stage by explicit path.

**Review focus** (inputs the spec implies but no requirement's test pins on its own; each has a test in the step that owns the code)

1. **Parent bits on remove and intersect.** Removing the last member of a word clears exactly the parent bits that become empty, and no others; the same through `intersect_with` and `difference_with`. Step 1: `remove_clears_only_empty_parents` (members `{0, 64, 4_096}`: removing 64 clears layer 1's bit 1 and leaves layer 2's bit 0 set; removing 0 then clears layer 2's bit 0 and leaves bit 1); Step 4: the model's `check_invariants` after every set-algebra op.
2. **The top word's range.** `top` uses bits 0 to 3 only. Step 1: the 64-bit-host top-of-range tests (spec §4.8), each ending with `check_invariants`.
3. **Skipping.** Iteration reads upper words only under set bits. Step 2: `iteration_skips_empty_ranges` wraps the set in a counting `WordSource` and iterates `{0, 1_000_000}`; it must read at most 16 words (a scan of layer 0 reads 15,626).
4. **Joins with sets of different lengths and repeats.** A required set shorter than another contributes zeros past its end; a set in both lists gives an empty join. Step 3: `join_short_required_set_ends_the_join`, `join_set_in_both_lists_is_empty`.
5. **A failed growth leaves the set unchanged.** Step 1: `try_insert_at_the_limit_is_an_error` (`try_insert(MAX_BITS)` and `try_insert(usize::MAX)` are `Err`, the set stays empty and `check_invariants` passes), run under `cargo xtask wasm` too.

## 2. Steps

### Step 1: layers and single-bit operations (R2, R6 to R8 for the set; part of R3 and R10)

- [x] **Who:** builder.
- **Files:** `collections/bit_set.rs` (module declarations and re-exports; module docs gain the layer rule of spec §4.2 and the space bound of §4.7), `bit_set/words.rs` (`bits_for_words`, `words_for_bits` moved here), `bit_set/hierarchical.rs` (new), `collections/error.rs`, `collections.rs` (re-exports), `libs/engine/tests/collections_panics.rs`, `libs/engine/tests/collections_auto_traits.rs`.
- **Interfaces produced:** `pub struct HierarchicalBitSet` with `MAX_BITS`, `const fn new`, `with_capacity`, `try_with_capacity`, `ensure_capacity`, `try_ensure_capacity`, `capacity`, `shrink_to_fit`, `clear`, `insert`, `try_insert`, `remove`, `contains`, `len`, `is_empty`, `first`, `last`, `as_words`, `as_trimmed_words`, `check_invariants`; `Clone`, `Default`, `Debug`, `PartialEq`, `Eq`, `Hash`; `BitSetInvariantError`. `first` and `last` descend from `top` without the cursor (`trailing_zeros` and `leading_zeros` per layer). `Debug` iterates layer 0 with the existing flat `Iter::new` until Step 2.
- **Test first:**
  - Unit test `hierarchical_layer_layout`: `{0, 63, 64, 4_096, 262_144}` gives, as the non-zero words of each layer (`(word index, word)` pairs): layer 0 `(0, 0x8000_0000_0000_0001), (1, 0x1), (64, 0x1), (4_096, 0x1)`; layer 1 `(0, 0x3), (1, 0x1), (64, 0x1)`; layer 2 `(0, 0x3), (1, 0x1)`; layer 3 `(0, 0x3)`; layer 4 `(0, 0x1)`; `top == 0x1`. The test compares literals, not values computed by the code under test.
  - Unit tests in `hierarchical.rs`: `remove_clears_only_empty_parents`; `try_insert_at_the_limit_is_an_error`; `max_bits_is_u32_max`; `check_invariants_reports_a_broken_set` (built by writing the private fields directly: a parent bit with a zero child word, a zero parent with a non-zero child, a `top` bit 4, a bit at `MAX_BITS`; each `Err` names the expected layer and word); `debug_format`; `shrink_to_fit_keeps_members`; `#[cfg(target_pointer_width = "64")]` top-of-range tests of spec §4.8.
  - `collections_panics.rs`: `hierarchical_insert_at_max_bits_panics_at_caller`; `#[cfg(target_pointer_width = "64")] hierarchical_with_capacity_past_max_bits_panics_at_caller`.
  - `collections_auto_traits.rs`: `HierarchicalBitSet`.
  - `cargo xtask test`: compile failure.
- **Change:** implement as §1 describes. A doctest on `HierarchicalBitSet` (spec §3's first example, without iteration until Step 2: use `contains` and `len`).
- **Verify:** `cargo xtask test`, `cargo xtask wasm` (review focus 5), `cargo xtask no-std` and `cargo xtask ci --quick` pass.
- **Commit:** `feat(collections): add HierarchicalBitSet layers and single-bit operations (M3.17)`

### Step 2: descent cursor, iteration, construction and conversions (R3, R4)

- [x] **Who:** builder.
- **Files:** `bit_set/descent.rs` (new), `bit_set/hierarchical.rs`, `collections/tests/mod.rs`, `collections/tests/hierarchical_model.rs` (new), `collections_auto_traits.rs`.
- **Interfaces produced:** `WordSource`, `WordCursor` (§1); `pub struct HierarchicalIter<'a>` (`Iterator<Item = usize>`, `FusedIterator`, `Clone`, `Debug`); `iter`, `IntoIterator for &HierarchicalBitSet`; `FromIterator<usize>`, `Extend<usize>`, `Extend<&usize>`, `From<[usize; N]>`, `try_extend`; `From<&BitSet> for HierarchicalBitSet`, `From<&HierarchicalBitSet> for BitSet`. `Debug` switches to `iter`.
- **Test first:**
  - `hierarchical_model.rs`: an `Op` enum `Insert(i)`, `TryInsert(i)`, `Remove(i)`, `Clear`, `EnsureCapacity(n in 0..1_048_576)`, `ShrinkToFit`, `TryExtend(Vec<usize>)`, `RoundTripBitSet` (through `BitSet` and back), `CloneSwap`. Indices from `prop_oneof![0..8, 56..72, 4_090..4_100, 262_140..262_150, 1_048_500..1_048_576]`. Proptest `hierarchical_matches_model` (ops of length `0..200`) checks after each op: return values, `iter()` against the model, `len`, `is_empty`, `first`, `last`, `contains(j)` for each member and its neighbours `j ± 1`, `as_trimmed_words()` against words built from the model, and `check_invariants()`.
  - Unit tests: `iteration_skips_empty_ranges` (review focus 3); `try_extend_stops_at_the_failing_index`; `bit_set_round_trip_keeps_members`.
  - `collections_panics.rs`: `hierarchical_from_iter_at_max_bits_panics_at_caller` (calls `HierarchicalBitSet::from_iter` directly, M3.14 deviation, Step 2).
  - `collections_auto_traits.rs`: `HierarchicalIter<'static>`.
  - `cargo xtask test`: compile failure.
- **Change:** implement. Doctest: spec §3's first example in full.
- **Verify:** `cargo xtask test`, `cargo xtask wasm`, `cargo xtask ci --quick` pass.
- **Commit:** `feat(collections): iterate HierarchicalBitSet by descending its layers (M3.17)`

### Step 3: `Join` (R5)

- [x] **Who:** builder.
- **Files:** `bit_set/join.rs` (new), `bit_set.rs`, `collections.rs`, `collections/error.rs` (`EmptyJoinError`), `collections/tests/join_model.rs` (new), `collections_panics.rs`, `collections_auto_traits.rs`; `collections.rs` module docs gain the paragraph and "Choosing a collection" row of spec §4.8.
- **Interfaces produced:** `pub struct Join<'a>` with `new` and `try_new` as spec §4.4 states; `Iterator<Item = usize>`, `FusedIterator`, `Clone`, `Debug`; `EmptyJoinError`.
- **Test first:**
  - `join_model.rs`: proptest `join_matches_model`: one to four required and zero to two excluded sets, each from `vec(index_strategy, 0..64)` with the Step 2 strategy, some lists repeating a set; the join equals the model's intersection of the required minus the union of the excluded, in ascending order.
  - Unit tests in `join.rs`: `join_short_required_set_ends_the_join`, `join_set_in_both_lists_is_empty` (review focus 4); `try_new_with_no_required_sets_is_an_error`; `empty_join_error_display`.
  - `collections_panics.rs`: `join_new_with_no_required_sets_panics_at_caller` (the message contains `"no required sets"` and `"pass the set of live indices"`).
  - `collections_auto_traits.rs`: `Join<'static>`.
  - `cargo xtask test`: compile failure.
- **Change:** implement. Doctest: spec §3's second example.
- **Verify:** `cargo xtask test`, `cargo xtask wasm`, `cargo xtask ci --quick` pass.
- **Commit:** `feat(collections): add Join over required and excluded hierarchical bitsets (M3.17)`

### Step 4: set algebra and tests (R3, R6)

- [ ] **Who:** builder.
- **Files:** `bit_set/hierarchical.rs`, `collections/tests/hierarchical_model.rs`, `libs/engine/tests/collections_laws.rs`.
- **Interfaces produced:** `union_with`, `intersect_with`, `difference_with` (`&mut self, &Self`); `is_subset`, `is_superset`, `is_disjoint` (`&self, &Self) -> bool`), as §1 describes.
- **Test first:**
  - The model gains `UnionWith(Vec<usize>)`, `IntersectWith(Vec<usize>)`, `DifferenceWith(Vec<usize>)` (argument built from the vector, so lengths differ), checked against `BTreeSet`'s operations with `check_invariants` after each; after each op, the three tests against a second generated set are compared with the model's.
  - Unit tests: `intersect_with_clears_emptied_parents` (review focus 1), `union_with_grows_to_trimmed_length`, `subset_ignores_capacity`.
  - `collections_laws.rs`: `hierarchical_eq_matches_model_eq` and `equal_hierarchical_sets_hash_equal`, built as the `BitSet` laws are (arbitrary `ensure_capacity`, high inserts then removed).
  - `cargo xtask test`: compile failure.
- **Change:** implement.
- **Verify:** `cargo xtask test`, `cargo xtask ci --quick` pass.
- **Commit:** `feat(collections): add set algebra to HierarchicalBitSet (M3.17)`

### Step 5: allocation checks and platform builds (R9, R11)

- [ ] **Who:** builder.
- **Files:** `libs/engine/tests/collections_allocations.rs`.
- **Test first:** in the file's single `dhat` test (`dhat` allows one profiler per process; M3.14 deviation, Step 4), labelled assertions with the `fewest_blocks` helper: `HierarchicalBitSet::with_capacity(4_096)` and, separately, `new()` then `ensure_capacity(4_096)`, then every operation of spec §4.6 (2,048 inserts below 4,096, `contains`, `first`, `last`, `len`, iteration, a `Join` of three sets with one exclusion, `intersect_with`, `difference_with`, `union_with` with an argument below 4,096 that was built with capacity 65,536, the three tests, `remove`, `clear`, inserts again): 0 blocks; `try_insert(MAX_BITS)` on `new()`: 0 blocks; `Join::new` and `try_new`: 0 blocks. These may pass at once, since Steps 1 to 4 implement the behaviour. Break one setup on purpose (`new()` without `ensure_capacity`) and see that check fail before restoring it.
- **Change:** whatever a failing check shows; expected none.
- **Verify:** `cargo xtask test`, `cargo xtask no-std`, `cargo xtask wasm`, `cargo xtask ci --quick` pass.
- **Commit:** `test(collections): check that HierarchicalBitSet and Join do not allocate (M3.17)`

### Step 6: fuzz target (R12)

- [ ] **Who:** builder.
- **Files:** `fuzz/fuzz_targets/hierarchical_bit_set_ops.rs` (new), `fuzz/Cargo.toml` (a `[[bin]]` entry), `xtask/src/fuzz.rs` (`TARGETS` gains `"hierarchical_bit_set_ops"` last).
- **Interfaces:** `#[derive(Arbitrary, Debug)] enum Op` over three sets (`which: u8 % 3`): `Insert(which, u32)`, `Remove(which, u32)`, `Clear(which)`, `EnsureCapacity(which, u32)`, `ShrinkToFit(which)`, `UnionWith(which, which)`, `IntersectWith(which, which)`, `DifferenceWith(which, which)`, `Tests(which, which)`, `Join(required: Vec<u8>, excluded: Vec<u8>)`, `RoundTripBitSet(which)`. Every `u32` index is taken `% 1_048_576`. After each op: members, `len`, `first`, `last` against three `BTreeSet<usize>` models, and `check_invariants()`; a `Join` with an empty required list checks `try_new` is `Err`.
- **Test first:** the xtask unit test that checks `TARGETS` against `fuzz/fuzz_targets/*.rs` fails once the file exists and `TARGETS` is not updated.
- **Change:** as listed.
- **Verify:** `cargo xtask fuzz-build`; `cargo xtask fuzz hierarchical_bit_set_ops --time 600` with no finding (the summary goes in `log.md` and the pull request); `cargo xtask ci --quick`.
- **Commit:** `test(fuzz): add a hierarchical bitset fuzz target (M3.17)`

### Step 7: benchmarks (R13)

- [ ] **Who:** builder writes them; the main session runs the measurements.
- **Files:** `bench/regress/benches/collections.rs`, `bench/compare/Cargo.toml`, `bench/compare/benches/collections.rs`, `Cargo.lock`.
- **Interfaces:**
  - Gungraun, in the file's existing pattern (setup, a never-inlined `*_work`, seed `0x5EED`, `Prng::shuffle`), a group `hierarchical_bit_set` in `main!`: `hierarchical_insert_4k`, `hierarchical_contains_4k` (50% occupancy), `hierarchical_iter_sparse_256k` (1% of a 262,144-index range), `hierarchical_join_3x1_64k` (three required, one excluded, 65,536-index range, 50% occupancy).
  - Criterion groups `hierarchical_insert`, `hierarchical_contains`, `hierarchical_iter`, `hierarchical_join`, over a 1,048,576-index range at 0.1%, 1%, 50% and 99% occupancy, spread `uniform` and `clustered` (runs of 256 consecutive indices), on subjects `engine::HierarchicalBitSet`, `engine::BitSet`, `hibitset::BitSet` and `hi_sparse_bitset::BitSet<_128bit>`. The join for `BitSet` is `clone` + `intersect_with` + `difference_with` into a scratch set, labelled so; a baseline without an exclusion operation computes it the way its docs recommend, and the pull request says how.
- **Test first:** `cargo xtask bench-smoke` runs the new benchmarks once.
- **Change:** as listed. A Gungraun benchmark under the 10,000-instruction minimum is enlarged and the change recorded in §5.
- **Verify:** `cargo xtask bench-smoke`, `cargo xtask deny` (unchanged `deny.toml`), `cargo xtask ci --quick`. The main session then runs `cargo xtask bench-regress` (devcontainer, or a throwaway Debian container with Valgrind as M3.14 did) and the wall-clock comparison under `/bench-run` (`cargo xtask bench --save m3.17-hierarchical` twice, `cargo xtask bench-report`, with the noise floor), and puts the numbers in the pull request, not in `docs/benchmarks.md`.
- **Commit:** `bench(collections): add hierarchical bitset instruction-count and comparison benchmarks (M3.17)`

### Step 8: space report (R14)

- [ ] **Who:** builder writes it; the main session runs it.
- **Files:** `bench/compare/examples/space_report.rs` (new), `bench/compare/Cargo.toml` (`dhat = "0.3"`), `bench/README.md` (how to run it; main session).
- **Interfaces:** `cargo run -p engine_benches --release --example space_report` prints a Markdown table: structure, case (occupancy and spread of Step 7, plus `one member at 1,000,000`), members, heap bytes after building, peak heap bytes, bytes after `shrink_to_fit` (ours and `BitSet`; `—` otherwise), bytes per member. It installs `dhat::Alloc` as the global allocator. With no arguments it runs each case by spawning itself (`std::env::current_exe()`) with `--case <structure> <case>`, because `dhat`'s peak is per process; a child builds the structure by inserting the members one at a time in a seeded random order (`engine::random`, seed `0x5EED`), reads `dhat::HeapStats`, and prints one tab-separated row. No `dhat-heap.json` is left behind (testing mode, or a file under `target/`).
- **Test first:** `cargo xtask ci --quick` builds examples; a `#[test]` in the example (`cargo test -p engine_benches --example space_report`) checks that a child row for `engine::HierarchicalBitSet` at `one member at 1,000,000` reports at least 125,000 bytes after building (layer 0 alone is 15,625 words), which fails until the build path is right.
- **Change:** as listed.
- **Verify:** the test passes; the main session runs the report and puts the table, machine and commit in `log.md` and the pull request; `git status` shows no stray output files.
- **Commit:** `bench(collections): add a heap-space report for the bitsets (M3.17)`

### Step 9: mutation testing and triage (R15)

- [ ] **Who:** main session runs `/mutants-run`; builder writes the tests for test gaps; main session writes the triage.
- **Files:** tests in the files above, `.cargo/mutants.toml` (exclusions with reasons, only for *equivalent* or *not worth a test*), `docs/mutation-triage.md` (a "Hierarchical bitset (M3.17)" subsection under `engine::collections`).
- **Test first:** `/mutants-run` with `--module collections` before any new test; classify every missed mutant and timeout in `bit_set/hierarchical.rs`, `descent.rs`, `join.rs` and the new error code. Likely survivors: the shift arithmetic of the layer index, the `top` range check in `check_invariants`, and the `Display` texts.
- **Change:** a test per *test gap, fixed*; an exclusion with a reason per *equivalent* or *not worth a test*.
- **Verify:** a second `/mutants-run`; every remaining missed mutant and timeout has a row, with both runs' summaries; `cargo xtask ci --quick`.
- **Commit:** `test(collections): triage the hierarchical bitset mutants (M3.17)`

### Step 10: documents (R10, R16)

- [ ] **Who:** main session.
- **Files:** `libs/engine/src/lib.rs` (module map `collections` row), `README.md` (same row), `libs/engine/README.md` if it lists collection types, `docs/research/data-structures.md` (Hierarchical bitset entry: **Repo fit** gains `in repo`, **Sources** lists spec §11), `CHANGELOG.md` (an M3.17 entry under Unreleased).
- **Test first:** the documentation review checklist against these files finds the rows missing; `cargo xtask check-docs` and `cargo xtask doc` run.
- **Change:** as listed. Read Lemire's post and *Hacker's Delight* chapters 2 and 5 before citing them as read, or list them as cited, not read.
- **Verify:** `cargo xtask doc`, `cargo xtask check-docs` and the full `cargo xtask ci` pass.
- **Commit:** `docs(collections): document the hierarchical bitset (M3.17)`

## 3. Final verification

- `cargo xtask ci` passes locally; `bench-regress` passes; the fuzz target has run clean for 10 minutes.
- Every requirement of spec §5 has the evidence spec §8 names (spec conformance review).
- Reviews run and recorded in the pull request, with the assistant's use stated.
- Close-out: spec `Implemented`, this plan `Historical`, roadmap row `Done (PR #n)`, the done-when list ticked with evidence, #44's box ticked in #38, and the sparse-variant follow-up issue opened with the time and space tables.

## 4. Lessons

- **testing-1** (bounded iteration and `Debug` in tests): followed. Every new unit and model test in `libs/engine/src/` walks iterators with `collections::tests::bounded` and formats with `debug_bounded`, never bare `collect` or `format!("{:?}")`, so a mutant that makes `HierarchicalIter` or `Join` endless cannot exhaust memory (Steps 1 to 4). Tests in `libs/engine/tests/` compare with `==` and do not iterate unbounded.
- **specs-2** (evidence noted in `log.md` needs its own step): followed. The fuzz summary (Step 6), benchmark numbers (Step 7) and space table (Step 8) are written to `log.md` inside the step that produces them, as part of its verify.
- **bench-1** (Markdown read by benchmark code): not applicable; no benchmark or library code includes Markdown.
- **specs-1**, **docs-1**: not applicable to this plan; spec §8 has no "finds nothing" check, and no rule is copied into several entry files.

## 5. Deviations

- **Step 1: panic message.** It follows `BitSet`'s wording ("... is not allowed: indices and capacities are limited to HierarchicalBitSet::MAX_BITS (...) and the allocation must succeed; use try_insert to handle it"), because the plan's text was wrong when an allocation fails.
- **Step 1: `BitSetInvariantError`** has a third private field, the broken rule, so `Display` names it.
- **Step 1: top-of-range members.** Layer 4's word index is `i >> 30`, so `2^30`, `2^31`, `3 × 2^30` and `MAX_BITS - 1` set only `top`'s bits 1 to 3; the test adds member 0 for bit 0. Spec §4.8's parenthesis said the three powers set bits 0 to 3; corrected there.
- **Step 1: `MAX_BITS`, `bits_for_words` and `words_for_bits`** moved to `words.rs`; `BitSet::MAX_BITS` now refers to `words::MAX_BITS` (same value).
- **Step 2: `first_word` and `last_word`** were not written: nothing uses them (the set's `first` and `last` descend on their own), and an unused `pub(super)` function fails clippy's `dead_code`. `LAYERS` moved to `descent.rs`.
- **Step 3: exclusions** are applied as one `word & !excluded` per excluded set at layer 0, not through an OR of the excluded words first (same result). `Join`'s `Debug` prints only the number of required and excluded sets.
