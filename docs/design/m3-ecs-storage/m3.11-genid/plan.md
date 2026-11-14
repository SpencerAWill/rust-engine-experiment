# M3.11 plan: `engine_genid`: one lean allocator, `GenId<T>`, and an untyped core

- **Status:** Draft (2026-11-14)
- **Spec:** [spec.md](spec.md). This plan argues from the spec; read both.

## 1. Approach

Two pull requests.

- **Part A** is a chore pull request from `main` (branch `chore/bench-regress-genid-names`). It renames the regress bench target, its `pool` group and that group's functions, and changes nothing under `libs/` (spec §4.7). It is reviewed and merged first.
- **Part B** is the chunk on `m3.11-genid`, rebased onto `main` after Part A merges. It holds the spec commits already on the branch and five commits: accept the spec and add the plan, remove the dense allocator, rename mechanically, reword, build `GenId<T>` on `UntypedGenId`, and update the documents. That follows spec §4.8, with the rename split into an identifier commit and a wording commit so each diff reads as one kind of change.

Choices the spec leaves to implementation:

- **The mechanical rename (B3)** is a scripted, ordered replacement over the tracked files outside the historical records (spec §6, Documentation), run from the session scratchpad and not committed. The order is: `HandlePool` to `GenIdMap`, `HandleAllocator` to `GenIdAllocator`, `InvalidHandleBits` to `InvalidGenIdBits`, `engine_handle` to `engine_genid`, `engine::handle` to `engine::genid`, the three fuzz target names, then the word `Handle` to `GenId` (word boundaries, Rust files only). The `git mv` moves come first so `git diff -M` pairs the files. B4 does the wording by hand: `handle` and `handles` to `id` and `ids`, `pool` to `map`, and prose. No script, because the verb "handle" must survive (spec §4.2).
- **Where the containers construct IDs.** `UntypedGenId` gets `pub(crate) const fn new(index: u32, generation: NonZeroU32) -> Self`. `GenId<T>` keeps `pub(crate) const fn new(index, generation)`, now `Self::from_untyped(UntypedGenId::new(index, generation))`, so the container code does not change in B5. `SlotTable` keeps returning `(u32, NonZeroU32)`.
- **`slot_limit`** has the dense allocator as its only caller. It is deleted in B2 (spec §4.1, as amended on acceptance).
- **The dense `check_invariants` test** covers the "dense arrays" length branch of the pool's `check_invariants`, which no pool test reaches. It moves into the pool's unit tests on a pool of `()`, using the existing `#[cfg(test)] corrupt_dense_length`.
- **Typed-against-untyped hash checks** use `std::hash::DefaultHasher::new()`, as `contracts.rs` already does. It has fixed keys, so it is deterministic (the bans cover `RandomState`, not `DefaultHasher`).
- **Checking the `.cargo/mutants.toml` patterns (R13)** uses `cargo mutants -p engine_genid --list --no-config`. Each pattern must match exactly one listed mutant there, and `cargo mutants -p engine_genid --list` (which applies the config) must list none of them. If this `cargo-mutants` version has no `--no-config`, use a copy of the config with `exclude_re` emptied, passed by `--config`, and record that in §4.

**Global constraints**

- Every Part B commit passes `cargo xtask ci --quick`. B3, B5 and B6 also pass `cargo xtask ci` (it includes `no-std`, `wasm`, `fuzz-build`, `doc` and `check-unsafe`).
- `engine_genid` stays `#![no_std]` with `extern crate alloc`, `#![forbid(unsafe_code)]` and no dependencies (R18). Nothing transmutes.
- The bit layout `(index << 32) | generation` does not change. The golden `0x0000_0003_0000_0002` and the `fixed_step_sim` `golden_checksum` stay byte-identical (R7).
- In Part B, the regress bench target (`genid`), group (`map`, `allocator`) and function names do not change (spec §4.2).
- Historical records (spec §6) are not edited, except ADR 0010's status line and the dated notes in `benchmarks.md` and `mutation-triage.md`.
- Commit messages are conventional and end with ` (M3.11)`, except Part A's chore commit, and with the session's attribution line. Stage by explicit path.
- Stop and ask the maintainer before each push, before opening each pull request, and before any merge.

**Review focus** (inputs the spec implies but no requirement's test pins on its own; each has a test in the step that owns the code)

1. **An untyped ID from elsewhere used on an empty or small container.** `GenIdMap::<u8>::new()` given `GenId::from_untyped(UntypedGenId::from_bits(u64::MAX).unwrap())` returns `None` from `get`, `false` from `contains`, and `None` from `remove`. `GenIdAllocator::is_alive` and `free` return `false`. Nothing panics. B5: `foreign_untyped_id_is_rejected_not_a_panic` (in `tests/contracts.rs`).
2. **Panic and `Display` messages name the new types.** `GenIdMap::reserve(usize::MAX)` panics with a message containing `GenIdMap`, the allocator's with `GenIdAllocator`, and `CapacityError`'s `Display` starts with `GenIdMap is full`. B3: the expectations in `tests/panics.rs` and the `capacity_error_reports_the_pools_own_limit` unit test are changed first, then the rename.
3. **The extreme encodings.** `UntypedGenId::from_bits(u64::MAX)` is index `u32::MAX`, generation `u32::MAX`. `from_bits(1)` is index 0, generation 1. `from_bits(1 << 32)` is `None`. All three give the same answers through `GenId<u8>`. B5: `untyped_extreme_encodings` (unit test in `id.rs`).
4. **Iterating live IDs on `GenIdMap<()>` after removals.** It yields exactly the live IDs, deterministically, and never allocates with fixed capacity. This replaces what the dense allocator's model and fixed-capacity tests covered. B2: the `run_alloc` model and `fixed_capacity.rs` move onto a pool of `()`.
5. **Debug output with nested generic types.** `GenId<Vec<(Particle, &str)>>` prints `GenId<Vec<(Particle, &str)>>(0v1)`. `UntypedGenId` prints without a type. B3 changes `debug_strips_paths_inside_generics`, and B5 adds `untyped_debug_has_no_type_name`.

## 2. Steps

### Part A, Step A1: rename the regress bench target, group and functions (spec §4.7)

- **Branch:** `chore/bench-regress-genid-names`, from `main`.
- **Files:** `git mv bench/regress/benches/handle.rs bench/regress/benches/genid.rs`; `bench/regress/Cargo.toml` (`[[bench]] name = "genid"`); `genid.rs` (group `pool` becomes `map`; `pool_insert_1k`, `pool_get_seq_1k`, `pool_get_random_1k`, `pool_remove_1k`, `pool_iter_10k`, `pool_churn_1k` and their `*_work` helpers become `map_*`; the `allocator` and `dense` groups are unchanged; the `main!` list follows; the crate still uses `engine_handle`); the comments in `bench/regress/benches/prng.rs` and `collections.rs` and in `spikes/ecs_storage/bench_cache/benches/cache.rs`; `CONTRIBUTING.md` ("Adding a benchmark"); `xtask/src/bench_regress/report.rs` (fixtures `row("handle", "pool_insert_1k", ..)` become `row("genid", "map_insert_1k", ..)`, `handle::dense_free_1k` becomes `genid::dense_free_1k`, and so on); `xtask/src/bench_regress/cargo_json.rs` (the `HANDLE` fixture becomes `GENID`, with target name `genid` and executable `/t/release/deps/genid-99e28a33`).
- **Test first:** change the `report.rs` and `cargo_json.rs` fixtures and assertions to the new names, then run `cargo test -p xtask bench_regress`. The tests still pass, because they use only their own fixtures, so instead check that `git grep -n 'benches/handle\|"handle"\|pool_insert_1k' -- bench xtask spikes CONTRIBUTING.md` has hits before the change and none after.
- **Verify:** `cargo xtask ci --quick`. In the devcontainer: `cargo xtask bench-smoke` (the Gungraun benches run once), then `cargo xtask bench-regress --report-only`. It prints that every `genid::*` benchmark is new, with no regression line. Keep that output for the pull request.
- **Commit:** `chore(bench): rename the regress handle benchmarks to genid and map`
- **Then:** run the reviews, ask before pushing and opening the pull request, and say in it that it prepares M3.11 and changes nothing under `libs/`. Merge after the maintainer approves, then rebase `m3.11-genid` onto `main`.

### Part B, Step B1: accept the spec and add the plan

- **Files:** `spec.md` (status `Accepted`, with the `slot_limit` correction in §4.1 and §4.7), `plan.md` (this file, status `Accepted` once reviewed), `docs/roadmap.md` (row `Accepted`, plan link).
- **Verify:** `cargo xtask typos`.
- **Commit:** `docs(m3.11): accept the spec and add the plan (M3.11)`

### Step B2: remove `DenseHandleAllocator` (R1, R11)

Still under the old names.

- **Files:** delete `libs/engine_handle/src/dense_allocator.rs` and `fuzz/fuzz_targets/dense_allocator_ops.rs`. Modify `libs/engine_handle/src/lib.rs` (module and `pub use`; the container table's third row becomes `HandlePool<()>`; the "All three issue identical handles" sentence becomes "Both"), `src/pool.rs` (delete `slot_limit`; add the moved test), `src/allocator.rs` (any doc link to the dense allocator), `tests/model.rs`, `tests/contracts.rs`, `tests/panics.rs`, `tests/fixed_capacity.rs`, `libs/engine/src/lib.rs` (crate map row, prelude), `libs/engine/tests/auto_traits.rs`, `fuzz/Cargo.toml`, `xtask/src/fuzz.rs` (target list), `bench/regress/benches/genid.rs` (`dense` group and its setups), `bench/compare/benches/handles.rs` (`EngineAlloc` and its calls). Comments in `nightly.yml` or `ci.yml` that count the fuzz targets are corrected to three.
- **Test first** (moved tests, written before anything is deleted; each passes on the current code, which proves the shared code keeps its coverage):
  - `tests/model.rs`: `run_alloc` drives a `HandlePool<()>` (`insert(())`, `remove`, `contains`, `iter().map(|(h, _)| h)`, `retain`) against the same model. Rename `allocator_matches_model_and_is_deterministic` to `unit_pool_matches_model_and_is_deterministic`. `run_lean_against_dense` becomes `run_lean_against_unit_pool`, and `lean_allocator_matches_dense_allocator_and_model` becomes `lean_allocator_matches_unit_pool_and_model`, with the same assertion: identical `to_bits` for every issued handle.
  - `tests/fixed_capacity.rs`: the `dense` field becomes `unit: HandlePool<()>`, built with `with_fixed_capacity(N)` and `with_capacity(N)`, and `work_on_fixed` does the same operations through the pool's API (`insert(())`, `remove`, `iter`, `retain`, `clear`). The `dhat` assertion of zero allocations after construction is unchanged.
  - `src/pool.rs` unit test `check_invariants_reports_a_dense_length_mismatch`: `HandlePool::<()>::new()`, insert one, `corrupt_dense_length()`, `check_invariants()` is `Err` whose message contains `dense arrays`.
  - `tests/contracts.rs` and `tests/panics.rs`: drop the dense cases. `emptiness_follows_the_live_count`, `a_clone_is_independent_and_continues_identically` and `reserve_overflow_panics_at_caller_with_an_actionable_message` keep their pool and lean-allocator cases. `iterator_debug_output_shows_the_remaining_count` keeps its pool iterator cases and loses the `DenseAllocIter` lines.
  - Run `cargo xtask test`: passes (the moved tests run on existing code).
- **Change:** delete the dense allocator and everything in **Files** that names it.
- **Verify:** `cargo xtask ci --quick`. `cargo xtask fuzz-build` builds three targets. `git grep -n 'DenseHandleAllocator\|DenseAllocIter\|dense_allocator'` hits only the historical records, ADR 0010 to 0012, this spec and plan, the CHANGELOG's earlier entries, `README.md`, `CONTRIBUTING.md` and `docs/research/data-structures.md` (the last three are updated in B6).
- **Commit:** `refactor(handle): remove DenseHandleAllocator (M3.11)`

### Step B3: mechanical rename to `engine_genid`, `GenId`, `GenIdMap` and `GenIdAllocator` (R2, R3, R11, R12, R13)

- **Files:** `git mv libs/engine_handle libs/engine_genid`; inside it, `git mv src/handle.rs src/id.rs`, `src/pool.rs src/map.rs`, `src/pool src/map`; `git mv fuzz/fuzz_targets/handle_from_bits.rs genid_from_bits.rs`, `handle_pool_ops.rs genid_map_ops.rs`, `handle_allocator_ops.rs genid_allocator_ops.rs`. Then apply the ordered replacement in §1 across: `Cargo.toml` (workspace dependency and path), `Cargo.lock` (regenerated by `cargo check`), `libs/engine_genid/**` (including `Cargo.toml`'s self dev-dependency and its description), `libs/engine/**`, `examples/fixed_step_sim/**`, `bench/regress/**`, `bench/compare/**` (Criterion `NAME`s: `engine_handle` and `engine_handle_lean` both become `engine_genid`), `fuzz/**` (including `fuzz/Cargo.lock`), `xtask/src/**` (`main.rs` `no-std` and `wasm` lists, `check_unsafe.rs`, `mutants.rs` `TRIAGED` and tests, `mutants/summary.rs` fixtures, `fuzz.rs` list and tests, `bench_doc.rs` tally subject and fixtures, `bench_report/*` fixtures, `bench_regress.rs` relevance test path), `.cargo/mutants.toml` (paths `libs/engine_genid/src/id\.rs`, `slots\.rs`, `map\.rs`; function names `GenId<T>::to_bits`, `GenIdMap<T>::resolve`; `Slot::set_state` and `write_short_type_name` are unchanged), `.github/workflows/mutants.yml` (path filter), and the comments in `ci.yml` and `nightly.yml`. Rename the module declarations in `lib.rs` (`mod id; mod map;`) and the `use` paths to match.
- **Test first:** change these expectations to the new names, then run `cargo xtask test` and watch them fail on the old output:
  - `id.rs` `debug_uses_short_type_name`: `"GenId<Particle>(3v2)"`. `debug_strips_paths_inside_generics`: `"GenId<Vec<(Particle, &str)>>(0v1)"` (review focus 5).
  - The `id.rs` doctest: `format!("{h:?}") == "GenId<Particle>(3v2)"`.
  - `tests/panics.rs` `reserve_overflow_panics_at_caller_with_an_actionable_message`: the case labels become `GenIdMap` and `GenIdAllocator`, and each case also asserts `message.starts_with(&format!("{name}::reserve"))` (review focus 2). `index_on_stale_handle_panics_at_caller` asserts its message starts with `GenIdMap index`.
  - The `map.rs` `capacity_error_reports_the_pools_own_limit` test: the message starts with `GenIdMap is full`.
  - `id.rs` `u64_conversions_match_to_bits_and_from_bits`: the error message also contains `is not a generational ID`.
- **Change:** the moves and ordered replacements above. `Debug` for `GenId` writes `GenId<`. `CapacityError`'s `Display` and every panic message name `GenIdMap` or `GenIdAllocator`. `InvalidGenIdBits`' `Display` becomes `"{bits:#018x} is not a generational ID: its generation (low 32 bits) is zero"`.
- **Verify:** `cargo xtask ci` passes. `git diff -M --stat HEAD~1` shows the moved files as renames. `git grep -nwE 'Handle|HandlePool|HandleAllocator|InvalidHandleBits|engine_handle'` hits only the files R1 to R3 allow and the prose that B4 rewrites. `cargo xtask fuzz-build` builds `genid_from_bits`, `genid_map_ops` and `genid_allocator_ops`. The `golden_checksum` test passes unchanged.
- **Commit:** `refactor(genid): rename engine_handle to engine_genid and Handle to GenId (M3.11)`

### Step B4: reword "handle" to "ID" in code and comments (R3, spec §4.2)

- **Files:** every file B3 touched, reviewed by hand with `git grep -niw 'handles\?\|pool\|pools'` over `libs/engine_genid`, `libs/engine`, `examples`, `bench`, `fuzz` and `xtask`.
- **Change:** parameters, locals, closure arguments and private fields: `handle` becomes `id`, `handles` becomes `ids` (including the map's private `handles: Vec<GenId<T>>` field and the iterators' fields), and `pool` becomes `map` where it names a `GenIdMap`. Test names that say "handle" say "id" (for example `index_on_stale_handle_panics_at_caller` becomes `index_on_stale_id_panics_at_caller`). Doc comments and messages say "ID" for the identifier, and "map" or "map or allocator" for the containers. The verb "handle", winit's `ApplicationHandler` and unrelated uses of "pool" stay (spec §4.2).
- **Test first:** none; behaviour is unchanged. The check is the grep above: before, it lists the identifier uses; after, only the allowed ones remain.
- **Verify:** `cargo xtask ci --quick`. The `cargo xtask test` count equals B3's, apart from renamed tests.
- **Commit:** `refactor(genid): say ID instead of handle in code and comments (M3.11)`

### Step B5: `UntypedGenId` and `GenId<T>` over it (R4 to R10, R12)

- **Files:** `libs/engine_genid/src/id.rs`, `src/lib.rs` (`pub use id::{GenId, UntypedGenId};`), `src/error.rs` (`TryFrom` docs name both types), `tests/model.rs`, `tests/contracts.rs`, `tests/auto_traits.rs`, `libs/engine/tests/auto_traits.rs`, `libs/engine/src/lib.rs` (`engine::genid` re-exports it through the module; the prelude does not), `fuzz/fuzz_targets/genid_from_bits.rs`.
- **Interfaces produced** (spec §4.3 and §4.4):
  - `pub struct UntypedGenId { index: u32, generation: NonZeroU32 }`, deriving `Clone, Copy, PartialEq, Eq`. `pub(crate) const fn new(u32, NonZeroU32) -> Self`. `pub const fn index(self) -> u32`, `generation(self) -> NonZeroU32`, `to_bits(self) -> u64`, `from_bits(u64) -> Option<Self>`. `From<UntypedGenId> for u64`, `TryFrom<u64>` (`Error = InvalidGenIdBits`), `PartialOrd`/`Ord` by `to_bits`, `Hash` of `to_bits`, `Debug` as `UntypedGenId({index}v{generation})`. No `Default`.
  - `#[repr(transparent)] pub struct GenId<T> { untyped: UntypedGenId, _marker: PhantomData<fn() -> T> }`. `pub const fn untyped(self) -> UntypedGenId` and `pub const fn from_untyped(id: UntypedGenId) -> Self`. `index`, `generation`, `to_bits` and `from_bits` forward. `pub(crate) const fn new` as in §1. The trait impls are as today, with `eq`, `cmp` and `hash` defined through `untyped`. No `Default`.
  - Compile-time assertions in `id.rs`, per spec §4.6, for `T = ()` and `T = [u8; 4096]`.
- **Test first:**
  - Unit tests in `id.rs`:
    - `untyped_bits_layout_is_index_high_generation_low`: `UntypedGenId::new(3, 2).to_bits() == 0x0000_0003_0000_0002`, the same value through `GenId::<u8>::from_untyped`, and `UntypedGenId::from_bits(0)` and `from_bits(7 << 32)` are `None`.
    - `untyped_extreme_encodings` (review focus 3).
    - `untyped_debug_has_no_type_name`: `"UntypedGenId(3v2)"` (review focus 5).
    - `untyped_u64_conversions`: `u64::from(u) == u.to_bits()`, and `UntypedGenId::try_from(5 << 32)` is `Err(e)` with `e.bits() == 5 << 32`.
    - `untyped_ord_matches_bits_order`: as `ord_matches_bits_order`, for `UntypedGenId`.
  - Doctests on `GenId`: the guide example from spec §3 (typed to untyped and back), and three `compile_fail` blocks: `takes_b(GenId::<A>::from_bits(1).unwrap())` (it exists; renamed in B3), `GenId::<u8>::from_bits(1).unwrap().cast::<u16>()`, and `let _: GenId<u8> = UntypedGenId::from_bits(1).unwrap();`. One `compile_fail` on `UntypedGenId`: `let _ = UntypedGenId::default();`.
  - Proptests in `tests/model.rs`:
    - `typed_and_untyped_agree(bits in any::<u64>())`: `GenId::<u8>::from_bits(bits).map(GenId::untyped) == UntypedGenId::from_bits(bits)`, and for `Some(u)`: `GenId::<u8>::from_untyped(u).untyped() == u` and `GenId::<u8>::from_untyped(u).to_bits() == u.to_bits() == bits`.
    - `typed_and_untyped_compare_and_hash_alike(a in any::<u64>(), b in any::<u64>())`, with the low halves forced non-zero by `| 1`: `cmp`, `==` and a `DefaultHasher` hash give the same results for the typed pair and for their `untyped()` pair, and `cmp` equals `a.cmp(&b)`.
  - `tests/contracts.rs` `foreign_untyped_id_is_rejected_not_a_panic` (review focus 1).
  - Both `auto_traits.rs` files: `UntypedGenId` and `GenId<Rc<u8>>` (the latter already present) are `Send + Sync + Debug`, and in `engine_genid`'s test also `Unpin + UnwindSafe + RefUnwindSafe` (spec §4.3).
  - Run `cargo xtask test`: compile failure (`UntypedGenId` does not exist).
- **Change:** implement the interfaces above. `GenId`'s methods become one-line forwards. The `Debug` for `GenId` reads `index` and `generation` through `untyped`. The docs of `to_bits` move to `UntypedGenId` with the "**This layout is frozen**" paragraph, and `GenId::to_bits` says it equals `self.untyped().to_bits()`. `genid_from_bits.rs` asserts that `UntypedGenId::from_bits` agrees with `GenId::<()>::from_bits` and that both round-trip.
- **Verify:** `cargo xtask ci` passes, including the doctests, the `compile_fail` blocks, the size assertions on host and `wasm32`, and `fuzz-build`. `golden_checksum` passes unchanged.
- **Commit:** `feat(genid): add UntypedGenId and build GenId<T> on it (M3.11)`

### Step B6: documents, glossary and ADRs (R15 to R17, R19, spec §4.5, §4.9, §4.10)

- **Files:**
  - `libs/engine_genid/src/lib.rs` crate docs: the first sentence says "generational IDs", followed by the ADR 0004 justification, the four-row "Choosing a container" table and the "Known limitation" wording.
  - The first doc sentence of `GenId<T>`; `libs/engine/src/lib.rs` crate map row ("Generational IDs: `GenId`, `UntypedGenId`, the `GenIdMap` container and the `GenIdAllocator`"); `libs/engine_time/src/lib.rs` ("neither math nor IDs").
  - `docs/glossary.md` (new: status `Accepted (date)`, the GenId-specific entries of spec §4.10, each linking to its rustdoc item or ADR 0012; no bare common words); `docs/README.md` (table row, convention line).
  - `docs/adr/0012-generational-ids.md` (status `Accepted (date)`); `0010` (status `Superseded by ADR 0012 (date)`); `0004` (crate table row).
  - `README.md` (crate table row, frozen line); `CONTRIBUTING.md` (frozen formats; the fuzzing section's targets and commands; the mutation section's crate names; the PR-title example becomes `feat(genid): add UntypedGenId (M3.11)`); `AGENTS.md`; `SECURITY.md`; `.github/ISSUE_TEMPLATE/bug_report.md`, `feature_request.md`, `determinism_bug.yml`; `.github/PULL_REQUEST_TEMPLATE.md`.
  - `CHANGELOG.md`: the preamble names `UntypedGenId::to_bits`, plus a **breaking** "Changed" entry listing the renames, the removal and `UntypedGenId`.
  - `docs/benchmarks.md` and `docs/mutation-triage.md`: the dated notes of spec §4.7.
  - `docs/research/data-structures.md`: the five repository-reference lines (98, 204, 214, 262, 264 as of `main` at 38b952e).
- **Test first:** before editing, `git grep -nE 'Handle::to_bits|engine_handle|HandlePool|HandleAllocator|DenseHandleAllocator'` over the files listed lists every line to change. After editing it lists none (the `benchmarks.md` and `mutation-triage.md` tables keep theirs below the notes).
- **Verify:** `cargo xtask typos`, `cargo xtask doc`, `cargo xtask ci --quick`. `git grep -niw handle` across the repository, reviewed line by line: what remains is the historical records, ADRs 0005 and 0008, the verb, `ApplicationHandler`, ADR 0012, the glossary's former-names entry, this spec and plan, and the new CHANGELOG entry.
- **Commit:** `docs(genid): ADR 0012, glossary and the renamed documents (M3.11)`

## 3. Final verification

Not commits. Each result goes into the pull request.

- `cargo xtask ci` on the branch tip.
- In the devcontainer: `cargo xtask bench-regress`. Every `genid::map_*` and `genid::allocator_*` benchmark is within 5% of the base, with no "new" rows for them (R14). Quote the table with base and head commits.
- `cargo xtask fuzz genid_map_ops --time 60`, then the same for `genid_allocator_ops` and `genid_from_bits`. No finding (R12).
- The mutants patterns check described in §1, then `cargo xtask mutants` for `engine_genid`, whose summary shows the four exclusions applied and no new missed mutants compared with `docs/mutation-triage.md` (R13). If the run is too slow locally, run it in the devcontainer, or trigger the weekly workflow on the branch and link the run.
- The spec's §9 checklist, ticked in the close-out commit.
- Then the eight reviews (CONTRIBUTING, "Reviews before a pull request"). Ask before pushing, open the pull request (closing #34 and #39; saying an assistant was used; saying the fuzz corpus restarts, per spec §4.2), and after its checks pass, push the close-out commit: spec `Implemented (PR #n, date)`, plan `Historical`, roadmap `Done (PR #n)`.

## 4. Deviations

Filled in during implementation.
