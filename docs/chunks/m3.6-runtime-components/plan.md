# M3.6 plan: Runtime-registered components in safe byte columns

- **Status:** Accepted (2026-11-22)
- **Spec:** [spec.md](spec.md). This plan argues from the spec; read both. Progress and surprises go in [log.md](log.md).

## 1. Approach

Nine steps, one commit each, bottom up: the byte column and its views; identity and the raw `World` operations; the raw query; three workload commits; the bench registrations; the measurements; the documents. Everything is in `spike_ecs_archetype` (`spikes/ecs_storage/archetype/`). The builder SHOULD read M3.4's and M3.5's plan §5 Deviations and logs first, which record the compiler and lint traps they met.

Choices the spec leaves to implementation:

- **Module homes.** `src/raw.rs` (new): `RawLayout`, `LayoutError`, `ByteColumn`, `RawColumn<'t>`, `RawColumnMut<'t>`, `CastError`; the 16-byte block type is private to it. `src/world/raw.rs` (new, a child module declared in `world.rs`, so it sees `World`'s private fields): `register_raw`, `spawn_raw`, `insert_raw`, `remove_raw`, `get_raw`, `RawError`. `src/raw_query.rs` (new): `RawQuery`, `RawQueryError`, `RawTable<'a, 'w>`, the run types. `src/codec.rs` (new): the `Codec` trait. Workload modules `src/bytes_simple_iter.rs`, `src/bytes_add_remove.rs`, `src/bytes_frag_iter.rs` (new, `pub mod`), each with both backends, its codec impls and its mirror types.
- **Names.** Backend types `BytesCopySimpleIter`, `BytesCastSimpleIter`, and so on, `BACKEND` = `"archetype_bytes_copy"` and `"archetype_bytes_cast"`. Public items from `lib.rs`: `pub use raw::{CastError, LayoutError, RawColumn, RawColumnMut, RawLayout};`, `pub use raw_query::{RawQuery, RawQueryError, RawTable};`, `pub use world::RawError;` (re-exported from `world::raw`), `pub use codec::Codec;`. `ComponentId` becomes `pub` (re-exported), with no public constructor.
- **Identity.** `ComponentInfo { type_id, name: &'static str, new_column: fn() -> … }` becomes `ComponentInfo { kind: ComponentKind, name: Box<str> }` with `enum ComponentKind { Rust { type_id: TypeId, new_column: fn() -> Box<dyn ErasedColumn> }, Raw(RawLayout) }` and a method `ComponentInfo::new_column(&self) -> Box<dyn ErasedColumn>`. Only column creation, `check_invariants` and the raw operations read `kind`; `register::<T>`, `component_id::<T>` and the typed hot paths keep their code (R15).
- **Column check.** `ErasedColumn::value_type_id` is replaced by `fn describe(&self) -> ColumnDesc` with `enum ColumnDesc { Typed(TypeId), Raw(RawLayout) }`, and `Table::check` compares it with what `ComponentInfo` expects (T2), plus, for a byte column, the zero tail (§4.3). `value_type_name` stays; a byte column returns `"<raw>"`, and its `Debug` prints the registered name through `World`'s `Debug`.
- **Byte column.** `ByteColumn { layout: RawLayout, len: usize, blocks: Vec<Block> }`, `#[repr(C, align(16))] #[derive(Clone, Copy, Pod, Zeroable)] struct Block([u8; 16])`. Bytes through `bytemuck::cast_slice::<Block, u8>(&blocks)[..len * size]`. `push(&mut self, bytes: &[u8])` grows `blocks` with `Vec::resize` to `ceil((len + 1) * size / 16)` blocks after `reserve` doubles, so growth is amortized; the length check against the layout is a `debug_assert!` (the raw operations have already returned `WrongSize`). `overwrite(row, bytes)` for `insert_raw` on a present value.
- **Views.** `RawColumn<'t>(&'t ByteColumn)` (`Copy`) and `RawColumnMut<'t>(&'t mut ByteColumn)`, `rows()` returning `ChunksExact<'t, u8>`, `rows_mut()` `ChunksExactMut<'t, u8>`, `get` and `get_mut` returning `Option<&[u8]>` and `Option<&mut [u8]>`; `cast` and `cast_mut` check size and alignment then `bytemuck::try_cast_slice(_mut)`, mapping its error to `CastError`. The views' constructors are `pub(crate)` and take the column itself, never a `&mut dyn ErasedColumn` (ecs-5); the downcast from `dyn ErasedColumn` happens in `raw.rs` (`ByteColumn::from_erased(&dyn ErasedColumn) -> &ByteColumn`, `from_erased_mut`), panicking with "T2: a raw component id names a byte column" if the column is typed.
- **Raw moves.** `Table` gains `move_row_adding_raw(src, src_ids, row, dst, dst_ids, bytes)` beside `move_row_adding<T>`; removal reuses the column-by-column move (`swap_remove_into`) with the removed column's `swap_remove_drop`, as `move_row_removing` does without returning a value. `spawn_raw` sorts `(ComponentId, &[u8])` pairs into a scratch `Vec` by id, finds or creates the archetype through `by_components` (counting `lookups` as `place` does), and writes the row with `Table::push_raw_row(entity, &sorted)`.
- **Raw query run.** `query_raw(&self, &RawQuery) -> Result<RawRun<'_>, RawQueryError>` and `query_raw_mut(&mut self, &RawQuery) -> Result<RawRunMut<'_>, RawQueryError>`, both `#[must_use]`; `for_each_table(self, f: impl FnMut(RawTable<'_, 'w>))`. Validation happens in `query_raw*`, so a rejected query visits nothing. Matching walks the archetypes as M3.4's `matched()` does, testing `components` (sorted) by binary search per id. Slots: one `Vec<Slot<'w>>` per run with `enum Slot<'w> { Read(&'w ByteColumn), Write(Option<&'w mut ByteColumn>) }`, cleared per table and filled from a single `iter_mut` pass over the table's columns, with fetch `k` mapped to its column index once per archetype (the stub in log.md, 2026-11-22, compiled this shape). No `Vec` per table.
- **Codec.** `pub trait Codec: Sized { const LAYOUT: RawLayout; fn encode(&self, out: &mut [u8]); fn decode(bytes: &[u8]) -> Self; }` with `RawLayout::new` made `const fn` so `LAYOUT` can be a constant (unwrapped with a `match` and `panic!` in const context). Fields as `f32::to_le_bytes` and `from_le_bytes` in declaration order; `Vec3` as `x, y, z`; `Mat4` as its 16 columns-major floats (`to_cols_array`).
- **Mirror types.** `#[repr(C)] #[derive(Clone, Copy, Pod, Zeroable)] struct RawVec3 { x: f32, y: f32, z: f32 }` and `RawMat4([f32; 16])` in `bytes_simple_iter.rs`, with `From` both ways; `add_remove` and `frag_iter` cast to `[f32]` directly (every component there is an `f32` newtype). The cast backends spawn with `bytemuck::bytes_of(&mirror)`, the copy backends with `Codec::encode`; neither backend reads a column the other path wrote.
- **Test hooks.** Each backend has `#[cfg(test)] archetype_count()` and `world()` hooks (as `ArchetypeAddRemove`), and the step's raw query is built by one function shared by the step and the hooks (ecs-1). `RawRun` has a `#[cfg(test)] matched_archetypes()` built from the same matching function the run uses.
- **Panic and error messages** name the operation first ("RawTable::column: fetch 1 is a write fetch; take it with column_mut", "RawTable::column_mut: fetch 0 was already taken for this table", "RawTable::column_mut: fetch 1 is a read fetch"). Error `Display`s say what was wrong and what to do ("component id 3 is a Rust type's; use the typed API").

**Global constraints**

- Every commit passes `cargo xtask ci --quick`. Step 1 also passes `cargo xtask deny`, with `deny.toml` unchanged.
- `bytemuck = { version = "=1.25.2", features = ["derive"] }` goes in `[workspace.dependencies]` with a comment ("`Pod` casts for the M3.6 byte columns, spike only"), and `spike_ecs_archetype` uses `bytemuck = { workspace = true }`. No other package gains it.
- The crate root keeps `#![forbid(unsafe_code)]`; never lift, relax or `allow` a CI-enforced attribute, even temporarily in the working tree. Every public item is documented. Doctests are not required ([spikes/README.md](../../../spikes/README.md)).
- Items a later step uses and nothing uses yet are `#[cfg(test)]` until then, never `#[allow(dead_code)]`.
- Conformance: `Params { count: 1_000, seed: 42 }` (`count: 20` for `frag_iter`) and 120 ticks, the typed tests' `PARAMS` and `TICKS`.
- Run `cargo fmt --all` before `ci --quick`. Commits are conventional, end with ` (M3.6)` and the session's attribution line, and stage files by explicit path.
- While a full `cargo xtask ci` runs, nothing else runs `cargo` in the tree.

**Review focus** (inputs the spec implies that no single requirement's test pins; each has a test in the step that owns the code)

1. **A stale id whose index was reused**: after `despawn(a)` and a `spawn()` that returns `b` at the same index, `get_raw(a, id)` is `None`, `insert_raw(a, …)` and `remove_raw(a, …)` return `DeadEntity`, and `b` has none of `a`'s values. Step 2: `stale_id_does_not_reach_the_reused_slot_raw`.
2. **Swap-remove of the last row and of a middle one** in a byte column, through `remove_raw` and `despawn`: the moved entity's bytes and location are intact. Step 2: `raw_remove_fixes_the_moved_row`.
3. **Replacing a present raw value**: `Ok(true)`, location and archetype count unchanged. Step 2: `replacing_a_raw_value_does_not_move_the_entity`.
4. **Layouts of different sizes in one table** (1, 12 and 16 bytes, alignments 1, 4 and 16): moves keep each column's values. Step 2: `mixed_layouts_move_together`.
5. **`spawn_raw` with ids out of order** stores each value in its own column. Step 2: `spawn_raw_sorts_its_entries`.
6. **`spawn_raw(&[])`** spawns into the empty archetype. Step 2: `spawn_raw_with_no_components`.
7. **A raw query with only filters and no fetches** visits the matching non-empty archetypes and gives `entities()`. Step 3: `raw_query_with_filters_only`.
8. **`count: 0`**: each new backend sets up empty, has checksum 0 and steps without panicking. Steps 4 to 6: `zero_count_is_valid_bytes_*`.

## 2. Steps

### Step 1: layout, byte column and column views (R1, R3 in part, R6, R9 in part)

- [x] **Who:** builder
- **Files:** `Cargo.toml` (workspace dependency), `Cargo.lock`, `spikes/ecs_storage/archetype/Cargo.toml`, `src/raw.rs` (new), `src/lib.rs`, `src/table.rs` (`ByteColumn` implements `ErasedColumn`; the `describe` change if needed here, else step 2).
- **Interfaces produced:** `RawLayout::new(size, align) -> Result<RawLayout, LayoutError>` (`const fn`), `size()`, `align()`; `LayoutError::{ZeroSize, AlignNotPowerOfTwo { align }, AlignTooLarge { align }, SizeNotMultipleOfAlign { size, align }}`; `ByteColumn` (crate-private) with `new(layout)`, `len`, `push`, `overwrite`, `from_erased`, `from_erased_mut`, and `ErasedColumn`; `RawColumn`, `RawColumnMut`, `CastError::{SizeMismatch { expected, got }, AlignTooLarge { column, requested }}`.
- **Test first:** in `raw.rs`: `zero_size_is_rejected`, `non_power_of_two_align_is_rejected`, `align_above_16_is_rejected`, `size_not_a_multiple_of_align_is_rejected`, `align_16_is_accepted` (R1); `cast_succeeds_at_every_alignment` (1, 2, 4, 8, 16), `cast_rejects_a_size_mismatch`, `cast_rejects_a_larger_alignment`, `cast_lengths_are_the_row_count` (R6); `swap_remove_drop_zeroes_the_tail`, `swap_remove_into_moves_the_bytes_and_zeroes_the_tail`, `swap_remove_into_a_typed_column_panics` (asserting the message), `push_grows_by_doubling` (capacity in blocks after 1,000 pushes is under twice the need, and `push` count of reallocations is logarithmic, read from `blocks.capacity()` changes) (R3). They fail to compile before `raw.rs` exists.
- **Verify:** `cargo nextest run -p spike_ecs_archetype raw::` passes; `cargo xtask deny` passes; `cargo xtask ci --quick` passes.
- **Commit:** `feat(spikes): add the byte column and its views to the archetype prototype (M3.6)`

### Step 2: identity and the raw `World` operations (R2, R3, R4, R5, R10)

- [x] **Who:** builder
- **Files:** `src/world.rs`, `src/world/raw.rs` (new), `src/table.rs`, `src/archetype.rs` (`ComponentId` public), `src/lib.rs`, `src/tests.rs`.
- **Interfaces produced:** `World::register_raw(&mut self, name: impl Into<Box<str>>, layout: RawLayout) -> ComponentId`; `spawn_raw(&mut self, &[(ComponentId, &[u8])]) -> Result<Entity, RawError>`; `insert_raw(&mut self, Entity, ComponentId, &[u8]) -> Result<bool, RawError>`; `remove_raw(&mut self, Entity, ComponentId) -> Result<bool, RawError>`; `get_raw(&self, Entity, ComponentId) -> Option<&[u8]>`; `RawError::{DeadEntity(Entity), UnknownId(ComponentId), NotRaw(ComponentId), WrongSize { id, expected, got }, Duplicate(ComponentId)}`. `spawn_raw` keeps `spawn`'s capacity panic (documented, `#[track_caller]`, `World::spawn_raw:` prefix), checked before the archetype lookup (ecs-4).
- **Test first:**
  - R2: `register_raw_returns_a_new_id_every_call`, `raw_and_rust_ids_never_coincide`, and one `not_raw_*` test per raw operation (`spawn_raw`, `insert_raw`, `remove_raw`; `get_raw` returns `None`), each asserting `NotRaw` and an unchanged world.
  - R3: `check_invariants_reports_a_wrong_layout` and `check_invariants_reports_a_dirty_tail` (corrupting a column inside `table.rs`'s tests, ecs-6), `check_invariants_reports_a_short_raw_column`.
  - R4, one per §4.5 row and per `RawError` variant, each on a fresh world (cold path), asserting the error value, `len()`, `archetype_count()` and `check_invariants()`: `spawn_raw_places_the_entity_in_its_final_archetype`, `spawn_raw_creates_one_archetype_for_a_new_set_and_none_for_a_known_one`, `insert_raw_adds_and_moves`, `insert_raw_replaces_in_place`, `remove_raw_moves_and_drops`, `remove_raw_of_a_missing_component_is_false`, `get_raw_returns_the_bytes`, `get_raw_is_none_for_each_missing_case`, `dead_entity_is_rejected_cold` (`insert_raw`, `remove_raw`), `unknown_id_is_rejected_cold` (`spawn_raw`, `insert_raw`, `remove_raw`), `wrong_size_is_rejected_cold` (`spawn_raw`, `insert_raw`), `duplicate_is_rejected_cold`, `despawn_drops_raw_values`.
  - R5: `raw_toggle_creates_no_archetype_after_the_first` and `raw_toggle_reads_the_edge_cache` (the `lookups` counter does not grow on a warm toggle, as the typed test does).
  - R10: `mixed_world_is_seen_by_both_kinds` (a typed `query::<&Matrix>()` and `get_raw` on one entity) and a proptest model `raw_and_typed_operations_match_the_model` over `spawn`, `spawn_raw`, `insert`, `remove`, `insert_raw`, `remove_raw` and `despawn` (specs-3), checking `check_invariants`, `get`, `get_raw` and `len` against a `BTreeMap` model after each operation.
  - Review focus 1 to 6.
  - They fail to compile before the operations exist; the red check for `check_invariants` is to comment out the zero-tail check and see `check_invariants_reports_a_dirty_tail` fail (`cargo nextest run -p spike_ecs_archetype --no-fail-fast`, ecs-6).
- **Verify:** every typed test in `tests.rs` and `world.rs` still passes unchanged; `cargo xtask ci --quick` passes. In the report, list the typed hot-path functions touched (if any) and say what changed in each (R15, ecs-7).
- **Commit:** `feat(spikes): register and store runtime components in the archetype prototype (M3.6)`

### Step 3: the raw query (R8, R9)

- [x] **Who:** builder
- **Files:** `src/raw_query.rs` (new), `src/lib.rs`, `src/tests.rs`.
- **Interfaces produced:** `RawQuery::new()`, `read`, `write`, `with`, `without` (each `fn(self, ComponentId) -> Self`, `#[must_use]`); `World::query_raw`, `World::query_raw_mut`; `RawRun<'w>` and `RawRunMut<'w>` with `for_each_table`; `RawTable<'a, 'w>` with `entities`, `column`, `column_mut`, `len`; `RawQueryError::{UnknownId(ComponentId), NotRaw(ComponentId), Repeated(ComponentId), WriteInReadQuery(ComponentId)}`.
- **Test first:** `unknown_id_is_rejected_before_any_table`, `not_raw_is_rejected_before_any_table`, `repeated_id_is_rejected_before_any_table` (fetch with fetch, fetch with filter, filter with filter), `write_in_read_query_is_rejected` (each with a counting closure that stays at 0); `raw_query_visits_matching_non_empty_archetypes_in_creation_order` (a world with matching, non-matching, `without`-excluded and empty archetypes); `column_on_a_write_fetch_panics`, `column_mut_on_a_read_fetch_panics`, `column_mut_twice_panics` (`should_panic(expected = …)` with each message); `two_write_fetches_are_usable_at_once`; `writes_through_column_mut_are_seen_by_get_raw`; `typed_query_ignores_raw_columns`; review focus 7. They fail to compile before `raw_query.rs` exists.
- **Verify:** `cargo xtask ci --quick` passes.
- **Commit:** `feat(spikes): add a query over runtime component ids (M3.6)`

### Step 4: codecs and the `simple_iter` backends (R7, R11 in part)

- [x] **Who:** builder
- **Files:** `src/codec.rs` (new), `src/bytes_simple_iter.rs` (new), `src/lib.rs`, `src/tests.rs`.
- **Change:** `Codec` and its impls for `Matrix`, `Position`, `Rotation`, `Velocity`; `RawVec3`, `RawMat4`; `BytesCopySimpleIter` and `BytesCastSimpleIter` (§4.7). The copy step: `pos.rows_mut().zip(vel.rows())`, decode, `rules::update`, encode. The cast step: `cast_mut::<RawVec3>()` and `cast::<RawVec3>()` once per table, then per row convert to `Position`/`Velocity`, `rules::update`, write back.
- **Test first:** `codec_round_trips_*` proptests per type over arbitrary `u32` bit patterns per field, comparing `to_bits` (R7); `simple_iter_bytes_copy_matches_the_reference`, `simple_iter_bytes_cast_matches_the_reference` (R11); `simple_iter_bytes_*_has_the_typed_archetype_count` (after setup and after one step, against `ArchetypeSimpleIter`); `zero_count_is_valid_bytes_simple_iter`. The conformance tests fail to compile first; as a red check, swap the order of two fields in `Position::encode` and see the round-trip and conformance tests fail.
- **Verify:** `cargo xtask ci --quick` passes. The builder reports whether the cast step's inner loop vectorizes, as the typed `ArchetypeSimpleIter` step's does (`cargo asm` or `--emit asm` on both steps, if available; otherwise say it was not checked) (ecs-9).
- **Commit:** `feat(spikes): add the byte-column simple_iter backends (M3.6)`

### Step 5: the `add_remove` backends (R11 in part)

- [x] **Who:** builder
- **Files:** `src/bytes_add_remove.rs` (new), `src/lib.rs`, `src/tests.rs`.
- **Change:** `Codec` for `A` and `B`; `BytesCopyAddRemove` and `BytesCastAddRemove` with the two passes of §4.7 (the read pass collects `(Entity, f32)` from a `read A` query, inserts `B` from `rules::make_b`; the remove pass collects the entities of a `with B` query and removes `B`). Checksum: a `read A, without B` query then a `read A, read B` query, `rules::hash(a, None)` and `rules::hash(a, Some(b))`.
- **Test first:** `add_remove_bytes_copy_matches_the_reference`, `add_remove_bytes_cast_matches_the_reference`, `add_remove_bytes_*_has_the_typed_archetype_count`, `zero_count_is_valid_bytes_add_remove`; the step query functions are shared with `#[cfg(test)]` hooks (ecs-1).
- **Verify:** `cargo xtask ci --quick` passes.
- **Commit:** `feat(spikes): add the byte-column add_remove backends (M3.6)`

### Step 6: the `frag_iter` backends (R11 in part)

- [x] **Who:** builder
- **Files:** `src/bytes_frag_iter.rs` (new), `src/lib.rs`, `src/tests.rs`.
- **Change:** `Codec` for the 26 letter types and `Data` through a macro; setup registers 27 raw components (letters in `LETTERS` order, then `Data`, or whatever order makes `ComponentId` order match the typed backend's column order; the builder states which); one `spawn_raw` per entity with `(letter, data)`. Step: one `write Data` query. Checksum: one `read letter, read Data` query per letter, `Letter::from_index`, `rules::hash`.
- **Test first:** `frag_iter_bytes_copy_matches_the_reference`, `frag_iter_bytes_cast_matches_the_reference` (`count: 20`), `frag_iter_bytes_*_has_the_typed_archetype_count` (27 after setup and after a step), `zero_count_is_valid_bytes_frag_iter`.
- **Verify:** `cargo xtask ci --quick` passes.
- **Commit:** `feat(spikes): add the byte-column frag_iter backends (M3.6)`

### Step 7: bench registrations (R12)

- [ ] **Who:** builder
- **Files:** `spikes/ecs_storage/bench_compare/benches/compare.rs`, `spikes/ecs_storage/bench_cache/benches/cache.rs`.
- **Change:** `bench::<…>(c);` for the six backends; six `cache_bench!` lines with each workload's `Spec::SIZES` (`[1_000, 10_000, 100_000]`, `[1_000, 10_000, 100_000]`, `[20]`) and their names in the `simple_iter`, `add_remove` and `frag_iter` groups.
- **Test first:** none possible without Valgrind (#74); the check is `cargo xtask bench-smoke` and a `cargo bench -p spike_ecs_bench_compare -- --list` that shows the six ids `spike/ecs/<workload>/archetype_bytes_{copy,cast}/<count>`.
- **Verify:** both commands; `cargo xtask ci --quick` passes. The report quotes the `--list` lines.
- **Commit:** `feat(spikes): register the byte-column backends in the spike benches (M3.6)`

### Step 8: measurements (R13, R14, R15)

- [ ] **Who:** builder for R13's count (a scratch crate outside the repository, ecs-2); main session for R15's code review against the parent commit, the two benchmark runs under `/bench-run`, and the `log.md` entries.
- **Files:** `docs/chunks/m3.6-runtime-components/log.md`.
- **Change:**
  1. R13 (builder): a scratch crate in the session's scratchpad with its own `[workspace]`, a `rust-toolchain.toml` copied from the repository, a path dependency on `spike_ecs_archetype`, and a counting `#[global_allocator]`. It reports allocations per `step` for the six new backends and the three typed ones at their smallest and largest size, per query run for a raw query at 2 and at 28 registered types (whether setup grows with registered types), and per `spawn_raw`. The commit it ran on is named (specs-4).
  2. R15 (main session): `git diff main -- src/world.rs src/table.rs src/query.rs src/bundle.rs` reviewed for any new lookup, branch on the kind or allocation in typed `insert`, `remove`, `spawn_with`, `despawn`, `get` and query runs; then, on a worktree of `main`, `caffeinate -i cargo bench -p spike_ecs_bench_compare --bench compare -- --save-baseline m3.6-parent '/archetype/'` (`spike-bench` takes no filter; the regex matches the typed backend's ids only).
  3. R14 (main session): `cargo xtask spike-bench ecs --save m3.6-informal` on the chunk's head, the whole set; `archetype` compared with `m3.6-parent` (within noise or not), and the six new backends read against `archetype`, `sparse_set` and `vec_aos`. Both runs record machine, power, load, times and the `pmset -g log` check.
- **Verify:** the three `log.md` entries exist, each naming its commit.
- **Commit:** `docs: record the M3.6 allocation counts and informal runs (M3.6)`

### Step 9: documents

- [ ] **Who:** main session
- **Files:** `spikes/ecs_storage/README.md` (the `archetype/` row names the raw components and the two backends; the backend list for the three workloads), `docs/chunks/m3.6-runtime-components/log.md`.
- **Check that fails before:** the README row does not mention `archetype_bytes_copy`; `cargo xtask check-docs` passes after.
- **Verify:** `cargo xtask check-docs` passes.
- **Commit:** `docs(spikes): document the byte-column backends (M3.6)`

The roadmap's M3.8 row caveats and `Done` status, the spec's `Implemented` status and the plan's `Historical` status go in the close-out commit (stage 7).

## 3. Final verification

`cargo xtask ci` on the final code commit, with nothing else running `cargo` in the tree; `cargo xtask bench-smoke`; the spec's Done-when checklist.

## 4. Lessons

- **ecs-1** followed: each step query is built by one function shared with its test hooks (steps 4 to 6), and `matched_archetypes()` uses the run's matching function (step 3).
- **ecs-2** followed: step 8 counts allocations from a scratch crate, including whether a raw run's setup grows with the registered type count.
- **ecs-3** n/a: no sealed query trait is added; the raw query is a value, not a trait.
- **ecs-4** followed: every `RawError` test is on a fresh world and asserts the error value (step 2); `spawn_raw` validates before it touches an archetype.
- **ecs-5** followed: the views are newtypes over `&ByteColumn` built in `raw.rs`; the downcast happens there too (step 1).
- **ecs-6** followed: step 2's red check targets `check_invariants` tests in the storage's own tests, run with `--no-fail-fast`.
- **ecs-7** followed: step 2's report lists the typed hot paths touched; step 8 reviews them against `main` and compares `insert_raw`'s lookups with `insert::<T>`'s.
- **ecs-8** n/a: no seal trait with a fetch GAT.
- **ecs-9** followed: `cast` and `cast_mut` return exactly the row count; step 4 checks the cast loop's vectorization and step 8 compares the first iteration numbers with `archetype` before quoting.
- **specs-2** followed: step 8 is the plan step for every `log.md` evidence item (R13 to R15).
- **specs-3** followed: steps 2 and 3 name one test per §4.5 row, per `RawError` and `RawQueryError` variant and per misuse panic, and step 2's model test lists its operations.
- **specs-4** followed: step 8's entries name their commit; a later code change means re-running or saying so in the pull request.
- **docs-1** followed: the README links to the spec rather than restating the raw API.
- **docs-2** followed: after any change to a count (backends, archetypes, tests), grep the plan and spec for the old count.
- **docs-3** followed: the spec's "expected, unmeasured" predictions keep that wording if the close-out copies them into the M3.8 row.

## 5. Deviations

- Step 1: `cast` and `cast_mut` map `bytemuck`'s cast error, which the size and alignment checks make impossible, to `CastError::SizeMismatch` through `unreachable_cast` rather than `expect`, so the public casts stay panic-free (clippy `missing_panics_doc`). The views also offer `len`, `is_empty` and `layout`.
- Step 2: `Table::check` gained a third invariant, T3 (no nonzero byte past a byte column's values), for the spec's zero tail; the `check_invariants_*` red-check tests corrupt columns in `table.rs`'s tests, where the columns are reachable (ecs-6). `insert_raw` and `remove_raw` do no map lookup (the kind check is an array index), against one `BTreeMap` lookup in `insert::<T>` and `remove::<T>`.
- Step 3: runs borrow the query (`RawRun<'w, 'q>`, `RawRunMut<'w, 'q>`) rather than cloning it, so a run's one allocation is its slot buffer; the runs are public types (`RawRun`, `RawRunMut`). `Table` gained `raw_columns` and `raw_columns_mut`, which hand out views of byte columns only, so the query module never holds `&mut dyn ErasedColumn` and skips a mixed table's typed columns.
- Step 4: `RawRunMut` also has a `#[cfg(test)] matched_archetypes()`, since the step's query writes and a read run rejects it. The backends build their step and checksum queries once at setup and keep them; `codec.rs` holds a `const fn layout` helper and the `f32` read and write helpers (`as_chunks`, per clippy).
- Step 5: the implementation was written before the tests (not test-first); the tests' first failure was a compile error, and the behavioural red check (the remove pass skipped in both backends) failed both conformance tests. The remove pass collects a table's entities with `extend_from_slice`, where the typed backend pushes per row.
- Step 6: letters are registered A to Z (ids 0 to 25), then `Data` (26); the typed backend's ids interleave (A, Data, B, ...), but archetypes are created in the same spawn order, so both worlds have 27 and the step matches archetypes 1 to 26.
