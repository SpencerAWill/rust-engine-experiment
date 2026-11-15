# M3.13 plan: One `engine` library crate, and the rule for splitting from it

- **Status:** Draft (2026-11-14)
- **Spec:** [spec.md](spec.md). This plan argues from the spec; read both.

## 1. Approach

One pull request on `m3.13-one-engine-crate`, which already holds the spec, the in-place ADR 0004 draft and the roadmap row. Ten steps. Step 2 captures evidence before anything changes and makes no commit. Step 8 is five commits, one per document group. Every other step is one commit.

Choices the spec leaves to implementation:

- **Order differs from spec §4.10 (a SHOULD).** The bench-label fix comes first (Step 3), with its test, because Criterion labels are plain strings and do not depend on the move. Committing the failing test on its own would break the rule that every commit passes `cargo xtask ci --quick`. Instead, Step 3 runs the test, saves its failing output for the pull request (R15), and commits the test together with the fix.
- **The move (Step 4) is scripted**, from a script in the session scratchpad that is not committed. First come the `git mv` commands, in the mapping below. Then come these edits, all applied to files under `libs/engine/`, `bench/`, `fuzz/`, `xtask/` and `examples/`:
  - `crate::` becomes `crate::<module>::` inside each module's files;
  - `engine_<m>::` becomes `engine::<m>::` everywhere;
  - the inner attributes `#![no_std]` and `#![forbid(unsafe_code)]`, and the `extern crate` lines, are deleted from the module roots;
  - the `pub use engine_<m> as <m>;` lines in `lib.rs` become `pub mod <m>;`, and the prelude's `engine_<m>::` paths become `crate::<m>::`.

  Nothing else changes, so `git diff -M` shows renames. No library code uses `$crate` or a crate path outside doc comments. The only `crate::` uses are in module code (75 in all), and the script rewrites all of them.
- **File mapping** (for module `<m>` in `genid`, `collections`, `random`, `time`, `math`):

  | From | To |
  |---|---|
  | `libs/engine_<m>/src/lib.rs` | `libs/engine/src/<m>.rs` |
  | `libs/engine_<m>/src/<path>` | `libs/engine/src/<m>/<path>` |
  | `libs/engine_<m>/tests/<name>.rs` | `libs/engine/tests/<m>_<name>.rs` |
  | `libs/engine_math/tests/transform_props.proptest-regressions` | `libs/engine/tests/math_transform_props.proptest-regressions` |
  | `libs/engine_time/benches/fixed_timestep.rs` | `libs/engine/benches/time_fixed_timestep.rs` |
  | `libs/engine/tests/auto_traits.rs` | `libs/engine/tests/prelude_auto_traits.rs` |

- **`extern crate std` at the root** is `#[cfg(any(feature = "std", test))]`, because the ID and collection modules' unit tests use `std` (today each crate has its own `#[cfg(test)] extern crate std;`).
- **The `libm` backend of `math::scalar`** is a private module, `libs/engine/src/math/scalar/libm_backend.rs`. It is always compiled, with one function per scalar operation and float type (`sin_f32`, `sin_f64`, ...). The `Float` impls call it when `std` is off. With `std` on, only the agreement test calls it, so the module carries `#[cfg_attr(all(feature = "std", not(test)), allow(dead_code, reason = "the no_std backend; with std it is only exercised by the agreement test"))]`. Keeping it compiled lets a host test catch a wrong mapping, for example `ln` mapped to `log10`, which a `no_std` build alone cannot.
- **Non-transcendental float methods that `core` lacks.** Step 5 starts with the failing `no_std` build, and its errors list exactly which methods are missing (candidates: `f32::abs` in `math` and `time`, and `f32::rem_euclid` in `transform2d.rs`). Each one is replaced with an implementation that is bit-identical to `std`'s, in a private `math::float` module (or `time`'s own private helper, if `time` needs one). For example, `rem_euclid(a, b)` is `let r = a % b; if r < 0.0 { r + abs(b) } else { r }`, and `abs` clears the sign bit (`f32::from_bits(x.to_bits() & 0x7fff_ffff)`). A test pins each against `std` on a set of inputs that includes `-0.0`, NaN and the infinities. None of these goes through `libm`, because `scalar` stays its only caller.
- **Mutant selection by module.** `cargo xtask mutants` runs `cargo mutants -p engine` with `-f libs/engine/src/<m>.rs -f 'libs/engine/src/<m>/**'` for each selected module. `-f` limits which files are examined and is separate from the configuration's `exclude_re`, so the exclusions still apply. Step 6 verifies this rather than assuming it (R9).
- **Rewriting the historical records (Step 8)** follows this table. It applies only to the crate split; other renames stay as they were (spec §2):

  | Written | Rewritten |
  |---|---|
  | `engine_handle`, `engine_genid`, `engine_collections`, `engine_random`, `engine_time`, `engine_math` (as crates) | `engine::handle`, `engine::genid`, ... (as modules), or "the `genid` module" in prose |
  | `libs/engine_<m>/src/lib.rs`, `libs/engine_<m>/src/<path>` | `libs/engine/src/<m>.rs`, `libs/engine/src/<m>/<path>` |
  | `libs/engine_<m>/tests/<name>.rs` | `libs/engine/tests/<m>_<name>.rs` |
  | `cargo mutants -p engine_<m>`, `cargo xtask mutants -p engine_<m>` | `cargo xtask mutants --module <m>` |
  | other `cargo ... -p engine_<m>` | `cargo ... -p engine` |
  | Criterion subjects `engine_handle`, `engine_handle_lean`, `engine_handle_dense`, `engine_random`, `engine_collections` | `engine::HandlePool`, `engine::HandleAllocator`, `engine::DenseHandleAllocator`, `engine::Prng`, `engine::IndexMap` (the type names of their time); after M3.11, `engine::GenIdMap` and `engine::GenIdAllocator` |
  | "the facade", "leaf crates", "a separate crate because ..." | the `engine` crate, its modules; the justification is removed, or becomes the module's purpose |

  No script does this step. Each document is edited by hand, with `git grep` used to find the mentions, because arguments about crate boundaries must be rewritten as text, not replaced word for word. Requirement IDs and section numbers do not change.

**Global constraints**

- Every commit passes `cargo xtask ci --quick`. Steps 5, 6, 7 and 9 also pass `cargo xtask ci`. Step 4 cannot pass it: `no-std` builds all of `engine`, and math needs `std` until Step 5.
- `engine` stays `#![no_std]` + `alloc`, `#![forbid(unsafe_code)]`, with the features `std` (default) and `test-invariants` only (R3).
- With `std`, behaviour and output stay byte-identical: the PRNG golden tests, the `UntypedGenId::to_bits` golden values, the `fixed_step_sim` `golden_checksum`, and `math`'s layout assertions (R5 to R7).
- `bench/regress` targets, groups and functions keep their names (spec §4.9).
- Git history and GitHub text are not rewritten. This spec and plan are the only files that may name the former crates once the chunk is done (R18).
- Commit messages are conventional, end with ` (M3.13)` and the session's attribution line, and stage files by explicit path.
- Stop and ask the maintainer before the benchmark run (Step 9), before each push, before opening the pull request, and before any merge.

**Review focus** (inputs the spec implies but no requirement's test pins on its own; each has a test or check in the step that owns the code)

1. **A wrong `libm` mapping** (for example `ln` to `log10`, `exp_m1` to `exp`) would pass every `std` test and break only `no_std` users. Step 5: `libm_backend_agrees_with_std`.
2. **A proptest regression file that no longer matches its test's name** is silently ignored, so saved failing cases stop being replayed. Step 4 checks that the file sits next to `math_transform_props.rs` with the same base name and the same number of `cc` lines as before.
3. **A `.cargo/mutants.toml` exclusion that stops matching after the path change**, or that `-f` overrides. Then the mutant would be tested, not excluded, and the triage would look worse without any code change. Step 6: the existing exclusion check, plus the list comparison.
4. **Doc links and doctests that still name a crate path**, which the doc build would break on. Step 4: `cargo xtask doc` with `broken_intra_doc_links = "deny"`, and the doctests.
5. **A user building on a `std` target with `default-features = false`** silently gets `libm` results. That is allowed (ADR 0003 as edited), but it must be documented where a user looks. No test can observe it, so Step 7 puts the sentence in the `engine` crate docs' feature table and in the `math::scalar` module docs, and the documentation review checks both.

## 2. Steps

### Step 1: accept the spec and add the plan

- **Files:** `spec.md` (status `Accepted (2026-11-14)`), `plan.md` (this file; status `Accepted` once the maintainer has reviewed it), `docs/roadmap.md` (the M3.13 row: status `Accepted`, link to the plan).
- **Verify:** `cargo xtask typos`.
- **Commit:** `docs(m3.13): accept the spec and add the plan (M3.13)`

### Step 2: capture the evidence from before the change (no commit)

Everything goes to the session scratchpad (`$S` below) and is quoted in the pull request.

- **Mutant lists (R9):** `cargo mutants --list -p engine_collections -p engine_genid -p engine_random > $S/mutants-before.txt`, and the same with `--no-config` into `$S/mutants-before-noconfig.txt`.
- **Public items (R2):** `cargo doc --no-deps -p engine_genid -p engine_collections -p engine_random -p engine_time -p engine_math -p engine`, then extract each crate's item list from `target/doc/<crate>/all.html` into `$S/items-before.txt` as `<crate>::<path>` lines.
- **Clean build time:** `cargo build -p engine --timings --target-dir $S/tgt-before`. Record the total and the per-crate times.
- **Proptest regressions:** `grep -c '^cc ' libs/engine_math/tests/transform_props.proptest-regressions > $S/regressions-before.txt`.

### Step 3: Criterion subjects named after their types, and the tally fix (R15)

- **Files:** `bench/compare/benches/handles.rs` (the value-store `NAME` at line 114 becomes `"engine::GenIdMap"`; the allocator `NAME` at line 513 becomes `"engine::GenIdAllocator"`), `bench/compare/benches/prng.rs:39` (`"engine::Prng"`), `bench/compare/benches/collections.rs:38` (`"engine::IndexMap"`), `xtask/src/bench_doc.rs` (a new `pub(crate) const TALLY_SUBJECT: &str = "engine::GenIdMap";`, used at line 281 and in the sentence at line 291; fixtures), `xtask/src/bench_report/{load,html,svg,mod}.rs` (fixtures).
- **Test first:** in `xtask/src/bench_doc.rs`, `tally_subject_is_a_compare_bench_subject`: read `bench/compare/benches/handles.rs` (from `env!("CARGO_MANIFEST_DIR")/../bench/compare/benches/handles.rs`) and assert that it contains `const NAME: &'static str = "<TALLY_SUBJECT>";`. Write it first with the subject as it is today (`"engine_genid"`), run `cargo test -p xtask tally_subject`, and check that it fails because `handles.rs` defines no subject named `engine_genid`. Save the output for the pull request.
- **Change:** the labels, the constant, and the fixtures.
- **Verify:** `cargo test -p xtask` passes; `cargo xtask bench-smoke` (every bench runs once); `cargo xtask ci --quick`.
- **Commit:** `fix(bench): name Criterion subjects after their types and fix the bench-doc tally subject (M3.13)`

### Step 4: move the five crates into `engine` as modules (R1, R2, R3, R7)

- **Files:** the `git mv` mapping in §1. Delete `libs/engine_{genid,collections,random,time,math}/Cargo.toml`. Modify:
  - **`libs/engine/Cargo.toml`:**
    - `[features]`: `default = ["std"]`, `std = ["glam/std"]`, `test-invariants = []`.
    - `[dependencies]`: `indexmap`, `foldhash`, `glam`, all `workspace = true`.
    - `[dev-dependencies]`: `proptest`, `dhat = "0.3"`, `criterion` (the version and features from `engine_time`), `rand_xoshiro = "0.8.1"`, and the self dependency `engine = { path = ".", features = ["test-invariants"] }`.
    - `[[bench]]`: `name = "time_fixed_timestep"`, `harness = false`.
    - `description`: "The engine library: generational IDs, deterministic collections, random numbers, simulation time and math".
  - **`libs/engine/src/lib.rs`:** the root attributes and `extern crate` lines (§1), `pub mod` for the five modules, and the prelude through `crate::`.
  - **The root `Cargo.toml`:** drop the five `[workspace.dependencies]` entries.
  - **The dependents:** `bench/compare/Cargo.toml`, `bench/regress/Cargo.toml` and `xtask/Cargo.toml` depend on `engine`; `fuzz/Cargo.toml` uses `engine = { path = "../libs/engine", features = ["test-invariants"] }`.
  - **The paths** in `bench/*/benches/*.rs`, `fuzz/fuzz_targets/*.rs`, `xtask/src/prng_stats.rs` and `libs/engine/{src,tests,benches}`.
  - **`xtask/src/main.rs`:** `no_std()` becomes `-p engine`; `wasm()` tests `-p engine`.
  - **`xtask/src/check_statics.rs`:** lines 229 and 246 become `libs/engine/src/random`.
- **Test first:** none new. The existing tests are the check, and they must pass with only path changes.
- **Verify:**
  - `cargo xtask ci --quick`, then `cargo xtask doc` and `cargo xtask test`.
  - `git diff -M --stat HEAD` shows the moved files as renames (`R0xx`). `git diff -M HEAD -- libs/engine/tests` touches only `use` lines and file names.
  - `cargo test -p fixed_step_sim golden_checksum` passes.
  - `cargo xtask fuzz-build` passes.
  - `ls libs` prints `engine`.
  - `grep -c '^cc ' libs/engine/tests/math_transform_props.proptest-regressions` equals the Step 2 count.
  - Public items: rebuild the item list from `target/doc/engine/all.html`, map `engine_<m>::` to `engine::<m>::` in `$S/items-before.txt`, and `diff` the two. The only lines allowed to differ are the removed crate roots.
- **Commit:** `refactor(engine): move the five library crates into engine as modules (M3.13)`

### Step 5: `math` without `std` (R4, R5, R6, R8)

- **Files:** the root `Cargo.toml` (`glam` features become `["f64", "i32", "u32", "nostd-libm"]`; add `libm = { version = "0.2", default-features = false }` to `[workspace.dependencies]` (glam 0.34.3 asks for `0.2`; 0.2.16 is in the local registry)), `libs/engine/Cargo.toml` (`libm = { workspace = true }`), `libs/engine/src/math/scalar.rs`, new `libs/engine/src/math/scalar/libm_backend.rs`, a new private `libs/engine/src/math/float.rs` if needed (§1), `clippy.toml`.
- **Test first:**
  1. `cargo xtask no-std` fails. Save the list of errors: every `std`-only method used in library code.
  2. In `scalar.rs`'s tests, `std_backend_is_the_inherent_method`: for each scalar function and both float types, over the inputs `[-2.5, -1.0, -0.5, -0.0, 0.0, 0.25, 0.5, 1.0, 2.0, 3.0]` restricted to the function's domain, `scalar::f(x).to_bits() == x.f().to_bits()`. It passes on the current code and pins R5.
  3. `libm_backend_agrees_with_std`: same inputs, `(libm_backend::f(x) - x.f()).abs() <= 4.0 * EPSILON * x.f().abs().max(1.0)` (with `f32::EPSILON` or `f64::EPSILON`). Include `log(x, base)` with base 2 and 10, and `powi` with exponents -3, 0, 1 and 5. It fails to compile until the backend exists.
  4. For each `math::float` helper, a test that it is bit-identical to `std`'s method over the inputs above plus `NaN`, `INFINITY` and `NEG_INFINITY`.
- **Change:**
  - The `libm` backend, with the mapping of spec §4.2.
  - `cfg`-selected bodies in the `Float` impls.
  - The `math::float` helpers, used where the build reported a missing method.
  - The per-function doc lines, for example "Without `std`: `libm::sinf` / `libm::sin`."
  - `clippy.toml`: a `disallowed-methods` entry for each wrapped `libm` function (`libm::sinf`, `libm::sin`, and so on through `libm::powf` and `libm::pow`), with the reason "use engine::math::scalar (ADR 0003)". `libm_backend.rs` carries the same `#![expect(clippy::disallowed_methods, reason = ...)]` that `scalar.rs` has.
- **Verify:**
  - `cargo xtask no-std` and `cargo xtask wasm` pass.
  - `cargo test -p engine scalar` passes.
  - Temporarily add `let _ = libm::sinf(1.0);` to `libs/engine/src/math/helpers.rs`, run `cargo xtask clippy`, and check that it fails on `disallowed_methods`; then remove the line. Save the output for the pull request (R8).
  - `cargo xtask ci` passes.
- **Commit:** `feat(math): build without std, with a libm backend for the scalar funnel (M3.13)`

### Step 6: tooling for one crate (R9, R10, R11)

- **Files:** `xtask/src/mutants.rs`, `xtask/src/mutants/summary.rs`, `xtask/src/coverage/summary.rs`, `.cargo/mutants.toml` (the four `exclude_re` entries, two in `^libs/engine/src/genid/id\.rs`, one each in `.../genid/slots\.rs` and `.../genid/map\.rs`), `.github/workflows/mutants.yml` (path filters: `libs/engine/src/{collections,genid,random}.rs` and `libs/engine/src/{collections,genid,random}/**`), `clippy.toml` (comments and reasons: `engine::collections::IndexMap`, `engine::math::scalar::<f>`, the `engine::genid` glossary), the root `Cargo.toml` comments, `xtask/src/bench_regress.rs` fixtures (the old crate paths at lines 757 and 882).
- **Interfaces:** `Options { modules: Vec<String>, jobs, list, in_diff }`. `parse_flags` accepts `--module NAME` (repeatable) and rejects `-p` with the message "`-p` was replaced by `--module`: the library is one crate". `const TRIAGED: &[&str] = &["collections", "genid", "random"];`. `fn file_args(modules: &[String]) -> Vec<String>` yields `-f libs/engine/src/<m>.rs -f libs/engine/src/<m>/**` for each module. `run_args` and `list_args` pass `-p engine` plus `file_args`.
- **Test first** (each fails before the change):
  - In `mutants.rs`:
    - `module_flag_selects_modules`: `parse(&["mutants", "--module", "time", "--module", "genid"])` gives modules `["time", "genid"]`.
    - `p_flag_is_rejected_with_a_pointer`: `-p engine_genid` is an `Err` containing `--module`.
    - `default_is_the_triaged_modules`.
    - `run_args_examine_each_module_root_and_directory`: for `genid`, the args contain `-p engine -f libs/engine/src/genid.rs -f libs/engine/src/genid/**`.
    - The existing `packages` and `run_args` tests are rewritten to modules.
  - In `mutants/summary.rs`:
    - `rows_are_ordered_by_module_with_total_last`: names under `libs/engine/src/random.rs` and `libs/engine/src/genid/map.rs` give rows `engine::genid` and `engine::random`.
    - `crate_root_maps_to_engine`: `libs/engine/src/lib.rs` gives row `engine`.
    - `examples_paths_map_to_their_crate` is unchanged.
  - In `coverage/summary.rs`:
    - `engine_rows_are_per_module`: `libs/engine/src/time/fixed.rs` gives `engine::time`, `libs/engine/src/time.rs` gives `engine::time`, `libs/engine/src/lib.rs` gives `engine`, and `examples/a/src/main.rs` gives `a`.
    - The existing `engine_time` fixture at line 492 moves to the new path.
- **Change:** as in **Interfaces**, and in spec §4.5.
- **Verify:**
  - `cargo test -p xtask`.
  - `cargo xtask mutants --list > $S/mutants-after.txt`. Map the paths in `$S/mutants-before.txt` (`libs/engine_<m>/src/lib.rs` to `libs/engine/src/<m>.rs`, `libs/engine_<m>/src/` to `libs/engine/src/<m>/`) and `diff` the two: they must be identical.
  - The exclusion check that `cargo xtask mutants --list` runs first reports each of the four patterns matching exactly one mutant.
  - `cargo xtask coverage` prints one row per module plus `engine`, and the example rows.
  - `cargo xtask fuzz-build` and `cargo xtask ci` pass.
- **Commit:** `refactor(xtask): select and report mutants and coverage by module of engine (M3.13)`

### Step 7: ADRs 0004 and 0003, the crate and module docs, the live documents and the CHANGELOG entry (R13, R14, R17)

- **Files:**
  - `docs/adr/0004-crate-splitting-rule.md`: the status line becomes `Accepted (2026-11-12). Rewritten in place by M3.13 (<date>); see its spec §4.8.`, with the text as drafted.
  - `docs/adr/0003-same-build-determinism.md`: the transcendental-funnel paragraph, the sanctioned-exceptions sentence and the status line, as in spec §4.7 and §4.8.
  - The crate and module docs: `libs/engine/src/lib.rs` (module map, first paragraph, a feature table with `std` and `test-invariants`, no "facade" or "leaf"); each `libs/engine/src/<m>.rs` (first paragraph without the crate justification; `no_std` wording as in spec §4.3); the `math::scalar` module docs (the two backends, and that `default-features = false` on a `std` target selects `libm`).
  - The live documents: `README.md` (the crate table becomes the module table; the command list says `mutants [--module NAME]...`), `CONTRIBUTING.md` (mutants commands, triaged modules, fuzz paths, coverage table, the funnel's path), `SECURITY.md`, `docs/roadmap.md` (spec §4.8: the M4 and M5 rough lists), and `AGENTS.md` and `docs/README.md` if `git grep` finds a former crate in them.
  - `CHANGELOG.md`: the entry of spec §4.12.
- **Test first:** `git grep -nwE 'engine_(genid|collections|random|time|math|handle)' -- libs README.md CONTRIBUTING.md SECURITY.md AGENTS.md docs/README.md docs/roadmap.md docs/adr/0003-same-build-determinism.md docs/adr/0004-crate-splitting-rule.md` lists hits before the change, and none after.
- **Verify:** `cargo xtask doc`; `cargo xtask test` (the doctests); `cargo xtask typos`; `cargo xtask ci`.
- **Commit:** `docs(engine): describe one library crate in the ADRs, crate docs and live documents (M3.13)`

### Step 8: the historical records (R14, R17, R18), five commits

Each commit applies the §1 rewrite table to one group. Before committing, `git grep -nwE 'engine_(genid|collections|random|time|math|handle)'` over the group returns nothing, and `git diff --word-diff` is read once to check that every change is about crates and modules.

| Commit | Files | Message |
|---|---|---|
| 8a | `docs/adr/0005-*.md`, `0006-*.md`, `0010-*.md`, `0011-*.md`, `0012-*.md` | `docs(adr): name modules of engine instead of crates (M3.13)` |
| 8b | `docs/design/m0-m1-foundations/{spec,plan}.md` | `docs(m0-m1): describe the foundations as modules of one crate (M3.13)` |
| 8c | `docs/design/m2-infrastructure/*/{spec,plan}.md` | `docs(m2): name modules of engine instead of crates (M3.13)` |
| 8d | `docs/design/m3-ecs-storage/{m3.0-windowed-baseline,m3.1-collections,m3.2-spike-harness,m3.11-genid}/*.md` | `docs(m3): name modules of engine instead of crates (M3.13)` |
| 8e | `docs/mutation-triage.md`, `docs/research/data-structures.md`, `CHANGELOG.md` (earlier entries), `examples/fixed_step_sim/tests/allocations.rs:18` (comment) | `docs: name modules of engine in the records and the changelog (M3.13)` |

- **Verify, after 8e:** `git grep -nwE 'engine_(genid|collections|random|time|math|handle)'` lists only `docs/benchmarks.md` (rewritten in Step 9), this spec and this plan. `cargo xtask typos`. A link check of the edited documents (the relative-link script used for the spec, run over `docs/`).

### Step 9: a fresh baseline and the regenerated `benchmarks.md` (R16)

- **Before:** ask the maintainer to start the run, or to approve starting it in the background. Nothing else may build until it ends.
- **Run:**
  1. A scratchpad script waits for the 1-minute load to fall below 2.5 (up to 300 s).
  2. It writes `docs/data/bench-env.txt` with the same keys as before (`date`, `cpu`, `memory_gb`, `cores`, `os`, `rustc`, `power`, `load_before`, `git`).
  3. It runs `cargo xtask bench --save m3.13`, recording the start and end times.
  4. Afterwards, `pmset -g log` for the run window shows no power-source change and no sleep. If it shows either, the affected groups are re-run with a name filter into the same baseline, and that is recorded, as the M1 run did.
- **Files:** `docs/data/bench-env.txt` (new), `docs/data/bench-env-run1.txt` and `bench-env-run2.txt` (deleted), `docs/benchmarks.md`.
- **Change:**
  - `cargo xtask bench-doc --baseline m3.13 --env docs/data/bench-env.txt` regenerates the three marked sections.
  - The hand-written sections (Summary, Methodology commands and run conditions, Interpretation, Limitations) are rewritten from the new data, for the types that exist now (no `DenseHandleAllocator`). Every figure is checked against the generated tables, and a claim the new run does not support is dropped.
  - The M3.11 note at the top is removed.
  - The status line becomes `Accepted (2026-11-12). Results re-recorded <date> (M3.13).`
- **Verify:**
  - Running `cargo xtask bench-doc` again leaves `docs/benchmarks.md` byte-identical.
  - `git grep -nwE 'engine_(genid|collections|random|time|math|handle)'` lists only this spec and this plan (R18).
  - `cargo xtask typos`.
- **Commit:** `docs(bench): re-record the Criterion results under the type-named subjects (M3.13)`

### Step 10: final verification, reviews, pull request and close-out

- `cargo xtask ci`.
- In the devcontainer, `cargo xtask bench-regress`. Quote the table with the base and head commits (R12).
- `cargo xtask fuzz-build`.
- The clean build time after the change: `cargo build -p engine --timings --target-dir $S/tgt-after`. Put it next to Step 2's figure in the pull request.
- The eight reviews of CONTRIBUTING ("Reviews before a pull request"). Public API, determinism, performance and portability all apply here.
- Ask before pushing and before opening the pull request. The pull request closes #40, says that an assistant was used, quotes the Step 2, 3, 5 and 6 evidence, and lists the eight review results.
- Once the checks pass, one close-out commit:
  - the spec `Implemented (PR #n, <date>)`, with its done-when list ticked with evidence;
  - this plan `Historical: executed in PR #n (<date>); the spec and the code are authoritative`, with §4 filled in;
  - the roadmap row `Done (PR #n)`.

  Commit message: `docs(m3.13): close out the chunk (M3.13)`.

## 3. Final verification

`cargo xtask ci`, `cargo xtask bench-regress` (devcontainer) and `cargo xtask fuzz-build` pass. Each item of the spec's §9 is ticked with the evidence from the steps above.

## 4. Deviations

Filled in during implementation. Planned in advance: the bench-label fix and its test come first, in one commit, rather than as spec §4.10's steps 1 and 5 (see §1).
