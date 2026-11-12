# M2.1 plan: Fuzzing

- **Status:** Historical: executed in PR #14 (2026-11-12); the spec and the code are authoritative
- **Spec:** [spec.md](spec.md). This plan argues from the spec; read both.

## 1. Approach

Seven commits on branch `m2.1-fuzzing`. The `xtask` commands come first, with unit tests, so every later step can be verified through `cargo xtask fuzz`. The `fuzz/` package follows, one target per commit, starting with the smallest (`handle_from_bits`), which also settles the `unsafe` lint question (R16) and the nightly pin before the larger targets depend on them. Workflows come after the targets exist, then the devcontainer and docs.

Choices the spec leaves to implementation:

- **The pin.** Step 2 picks the most recent `nightly-YYYY-MM-DD` that builds `libfuzzer-sys` and the sanitizer runtime on both the local macOS machine and `ubuntu-latest`. Step 1 ships a placeholder date that Step 2 replaces (the `xtask` unit tests do not need a real toolchain).
- **`xtask` fuzz code is pure where it can be.** Command lines are built by functions that return `Vec<String>` and are unit-tested; only a thin wrapper calls `run`. This follows the `caffeinate_args` pattern in `xtask/src/cmd.rs`.
- **Model checks in the targets.** Each target keeps its model small and inline. No helper is shared across target files, because each `fuzz_targets/*.rs` is its own binary (spec §7).
- **Mutation checks** (spec §8, R2 to R5, R19, R20) use temporary edits to `libs/engine_handle/src`, reverted with `git checkout -- libs/` before the commit. A plant is never committed. If a fuzz target does not find its plant within 60 seconds, the target is too weak: strengthen it, do not extend the time.

**Global constraints**

- Every commit passes `cargo xtask ci --quick`; Steps 2 onward also pass `cargo xtask fuzz-build`.
- `cargo xtask check-workflows` passes at every commit that touches a workflow.
- Commit messages are conventional, end with ` (M2.1)`, and end with the attribution lines the session requires.
- No `HashMap`, `HashSet`, clocks or `thread_local!` in `fuzz/` (R6), checked by the grep in the spec's §8.
- Stop and ask the maintainer before the first push of the branch (Step 6 needs a pushed branch to run the nightly job through `workflow_dispatch`).

**Review focus** (failure modes no step's check fully covers)

1. A model that mirrors the implementation's bugs instead of checking behaviour (for example, computing "alive" from the subject). Every model must be derived only from the ops it has seen. The mutation checks cover the cases they plant, not this in general, so review the three model blocks by hand.
2. `Arbitrary` input shapes that make stale handles rare. Check by printing a coverage counter in a local run (not committed) that stale and probe ops reach the subject at least a few percent of the time.
3. A cache key that never restores, so the corpus never accumulates. Covered by the second `workflow_dispatch` run in §3.
4. The pin drifting between `xtask`, CI and the devcontainer. Covered by R7's grep.
5. The lenient local skip hiding a real breakage: a contributor without `cargo-fuzz` never sees a `fuzz/` compile failure locally. Accepted because CI is strict (`CI` set), but check that the skip note is loud and names the install line.
6. Targets that pass but never reach the capacity-limit and drop-accounting paths. Check with the temporary counter from focus 2 that fixed-capacity inputs and full-pool `TryInsert` calls occur often.

## 2. Steps

### Step 1: `xtask` fuzz commands (R7, R8, R9, R15, R21)

- **Files:** `xtask/src/fuzz.rs` (new), `xtask/src/main.rs` (module, `Command` variants, `parse`, `execute`, help text).
- **Test first:** in `xtask/src/fuzz.rs` add unit tests for: `parse_fuzz` (a named target defaults to 60 s; `all` defaults to 600 s; `--time 5` overrides; `--replay <path>` is accepted only with a named target; an unknown target is an error listing the valid ones; `--time` without a number is an error); the argument builders (`run_args("handle_pool_ops", 60)` is `["+<pin>", "fuzz", "run", "--sanitizer", "none", "--debug-assertions", "handle_pool_ops", "--", "-max_total_time=60"]`; the replay builder has the same flags and ends with the path and no time flag; `build_args()` is `["+<pin>", "fuzz", "build", "--sanitizer", "none", "--debug-assertions"]`; R21); `TARGETS` equals the sorted `fuzz/fuzz_targets/*.rs` stems (this test fails until Step 2 creates the directory: mark it `#[ignore = "enabled in Step 5"]` here and remove the attribute in Step 5); the Windows message function returns text containing "devcontainer". In `main.rs`, a `parse(&["fuzz-toolchain"])` test. Run `cargo xtask test`: it fails to compile because `fuzz` does not exist.
- **Change:** add `const FUZZ_NIGHTLY: &str = "nightly-2026-11-10"` (placeholder, replaced in Step 2) and `const TARGETS: [&str; 4]`; implement `parse_fuzz`, the pure argument builders, a `fuzz(...)` function that checks `cfg!(windows)`, then `require_cargo_subcommand("fuzz", "cargo install --locked cargo-fuzz")`, then the pinned toolchain (`rustup run <pin> rustc --version`, hint `rustup toolchain install <pin> --profile minimal`), then runs `cargo` with the built arguments from the `fuzz/` directory (set the working directory through a small `run_in(dir, ...)` addition to `cmd.rs`, with a test of the argument handling if it is pure). `fuzz_build(strict: bool)` runs the build and, when not strict, returns a skip note instead of an error for a missing tool or a Windows host; the decision is a pure function `build_mode(tools_present: bool, windows: bool, ci_env: bool) -> BuildMode` (`Run`, `Skip(reason)`, `Fail(reason)`) with its own unit tests. `fuzz all` loops over `TARGETS`, collects failures and returns one error naming every failed target. `fuzz-toolchain` prints the pin. Add the three commands to the `Command` enum, `parse`, `execute` and the usage text. Do **not** touch `ci_steps` yet (`fuzz/` does not exist).
- **Verify:** `cargo xtask test` passes; `cargo xtask clippy` and `cargo xtask fmt --check` pass; `cargo xtask fuzz-toolchain` prints the placeholder; `cargo xtask fuzz nope` prints the valid target names; `cargo xtask fuzz handle_pool_ops` on a machine without `cargo-fuzz` (or with `PATH` stripped of it) prints the install line.
- **Commit:** `feat(xtask): add fuzz, fuzz-build and fuzz-toolchain commands (M2.1)`

### Step 2: `fuzz/` package, `handle_from_bits`, the pin, and `ci` wiring (R1, R5, R6, R7, R10, R16)

- **Files:** `fuzz/Cargo.toml`, `fuzz/Cargo.lock`, `fuzz/.gitignore`, `fuzz/fuzz_targets/handle_from_bits.rs`, `xtask/src/fuzz.rs` (the pin only; the on-disk `TARGETS` test stays ignored until Step 5, when all four files exist), `xtask/src/main.rs` (`ci_steps`).
- **Test first:** write `handle_from_bits.rs` with the checks from spec §4.3 (the `(u64, u64)` input, so the ordering claim can be checked). Before the package exists, `cargo xtask fuzz-build` fails (no `fuzz/Cargo.toml`). Add the `ci_steps` unit tests (`FuzzBuild` is in the full list and absent from `--quick`) and watch them fail.
- **Change:**
  1. Install the toolchain and tool locally. Choose the pin: try the newest dated nightly; if `cargo +<date> fuzz build` fails because of the sanitizer runtime or `libfuzzer-sys`, step back day by day. Set `FUZZ_NIGHTLY`.
  2. Create the package as `cargo fuzz init` lays it out (then edit): `name = "engine-fuzz"`, `publish = false`, `edition = "2024"`, dependencies `libfuzzer-sys`, `arbitrary` (feature `derive`), and `engine_handle = { path = "../libs/engine_handle", features = ["test-invariants"] }`; `[package.metadata] cargo-fuzz = true`; a `[workspace]` table so Cargo does not look for a parent workspace; each `[[bin]]` with `test = false`, `doc = false`, `bench = false`. Commit `fuzz/Cargo.lock`.
  3. **R16 experiment.** Put `#![forbid(unsafe_code)]` in `handle_from_bits.rs` and build. If it compiles, keep it. If it fails, record the compiler message, switch to `#![deny(unsafe_code)]` with a scoped `#[allow(unsafe_code)]` and a comment, and note it in §4 Deviations. Repeat the choice for every later target file so all four agree.
  4. Wire `FuzzBuild` into `ci_steps` (full run only). Inside `ci` it executes through `fuzz_build(strict: std::env::var_os("CI").is_some())`; run directly as `cargo xtask fuzz-build` it is always strict.
- **Verify:** `cargo xtask fuzz-build` succeeds; `cargo xtask fuzz handle_from_bits --time 30` runs for 30 s and exits 0, and the output shows executed runs and coverage growth; `cargo xtask ci --quick` and `cargo xtask test` pass; `cargo xtask ci` lists `fuzz-build`; with `cargo-fuzz` hidden from `PATH`, `cargo xtask ci` prints the skip note and continues, and with `CI=1` it fails with the install hint (R10). **Mutation check (R5):** temporarily change `Handle::from_bits` in `libs/engine_handle/src/handle.rs` to accept a zero generation (use `NonZeroU32::MIN` when it is zero), run `cargo xtask fuzz handle_from_bits --time 60`, confirm a crash within the minute, then `git checkout -- libs/`. Run `git diff --stat -- libs/ rust-toolchain.toml` and confirm it is empty (R14).
- **Commit:** `feat(fuzz): add the fuzz package and the handle_from_bits target (M2.1)`

### Step 3: `handle_pool_ops` (R2, R3, R4, R6, R19, R20)

- **Files:** `fuzz/fuzz_targets/handle_pool_ops.rs`.
- **Test first:** the target is the test. Write it, then run it against a known-correct pool and confirm it survives 60 s (a target that crashes at once on correct code has a wrong model). Then the mutation checks below must fail it.
- **Change:** implement spec §4.3 for the pool: the `Input { capacity, ops }` shape and the `Tracked` drop-counting value (R19, R20), with ops `Insert(u16)`, `TryInsert(u16)`, `Remove(usize)`, `Get(usize)`, `GetMut(usize, u16)`, `GetDisjoint(usize, usize)`, `Retain(u16)`, `Drain`, `Clear`, `Iterate`, `Probe(u64)`; subject `HandlePool<Tracked>`, model `BTreeMap<u64, u16>` keyed by `to_bits`; an `issued: Vec<Handle<u16>>` for modulo selection; after each op, assert `inserted == model.len() + drops.get()`, compare `len()`, call `check_invariants().expect(...)`, and check the `iter()` set against the model. `GetDisjoint` is skipped when the two selections pick the same handle (the pool documents that it fails for duplicates; read the API doc and make the target assert that failure instead if it returns an error value). Every `Probe` asserts that `contains`, `get` and `remove` on bits the model does not hold report absent and leave the pool unchanged.
- **Verify:** `cargo xtask fuzz handle_pool_ops --time 60` exits 0. **Mutation checks (R2, R3, R4), one at a time, each reverted with `git checkout -- libs/` before the next:** (a) make `HandlePool::remove` return `None` for a live handle whose index is 0; (b) skip the generation bump when a slot is freed, so a stale handle becomes valid again; (c) corrupt an internal link in the slot table after `retain` (the kind of bug `check_invariants` exists to catch, so the plant must be one that the model comparison alone would not catch: pick one by reading `slots.rs`). (d) make `HandlePool::clear` forget its values with `mem::forget` (R19); (e) let `try_insert` accept one value past a fixed capacity (R20). Each must be found within 60 s. Record the five plants and the time-to-find in the pull request description.
- **Commit:** `feat(fuzz): fuzz HandlePool against a model (M2.1)`

### Step 4: `handle_allocator_ops` (R2, R3, R4, R6, R20)

- **Files:** `fuzz/fuzz_targets/handle_allocator_ops.rs`.
- **Test first:** as Step 3 (run against correct code for 60 s first).
- **Change:** the `Input { capacity, ops }` shape (R20) with ops `Allocate`, `TryAllocate`, `AllocateMany(u8)` (count capped at 64), `TryAllocateMany(u8)`, `Free(usize)`, `IsAlive(usize)`, `Clear`, `Probe(u64)`; subject `HandleAllocator<()>`, model `BTreeSet<u64>`; also assert that no `(index, generation)` pair is ever issued twice between `Clear`s that the model has seen (keep a `BTreeSet<u64>` of every bits value issued; `clear` must still never reissue a handle that is currently alive, and the allocator's docs say what `clear` does to generations, so read them and encode exactly that rule). `check_invariants` after every op.
- **Verify:** `cargo xtask fuzz handle_allocator_ops --time 60` exits 0. **Mutation check (R2, R4):** make `free` not bump the generation; the target finds it within 60 s; revert.
- **Commit:** `feat(fuzz): fuzz HandleAllocator against a model (M2.1)`

### Step 5: `dense_allocator_ops` (R2, R3, R4, R6, R20)

- **Files:** `fuzz/fuzz_targets/dense_allocator_ops.rs`, `xtask/src/fuzz.rs` (remove the `#[ignore]` on the `TARGETS`-matches-disk test).
- **Test first:** run the previously ignored `TARGETS` test with `--include-ignored`: it passes only once the fourth file exists. Before the target file is added it fails because `dense_allocator_ops` is listed but missing.
- **Change:** the `Input { capacity, ops }` shape (R20) with ops `Allocate`, `TryAllocate`, `Free(usize)`, `IsAlive(usize)`, `Retain(u8)`, `Clear`, `Iterate`, `Probe(u64)`; subject `DenseHandleAllocator<()>`, model `BTreeSet<u64>`; `iter()` is compared as a set, and two consecutive `iter()` calls must yield the same order. `Retain(k)` keeps a handle when `index % k != 0` (`k` forced to at least 1).
- **Verify:** `cargo xtask fuzz dense_allocator_ops --time 60` exits 0; `cargo xtask test` passes with the test enabled; `cargo xtask fuzz all --time 10` runs all four targets and exits 0. **Mutation check (R2, R3):** make `DenseHandleAllocator::free` leave the freed handle in the dense list; found within 60 s; revert.
- **Commit:** `feat(fuzz): fuzz DenseHandleAllocator against a model (M2.1)`

### Step 6: Workflows (R11, R12, R13)

- **Files:** `.github/workflows/ci.yml` (job `fuzz-build`), `.github/workflows/nightly.yml` (job `fuzz`).
- **Test first:** `cargo xtask check-workflows` passes before the edit (baseline). After adding the jobs with an unpinned `uses:` on purpose, it fails; replace each with the full commit hash and a version comment, copying the hashes already used in these files for `actions/checkout`, `Swatinem/rust-cache`, `taiki-e/install-action` and `actions/upload-artifact`. For `actions/cache` (one hash serves both the `restore` and `save` sub-actions, since they are paths in the same repository), look up the current release's full commit hash with `gh api repos/actions/cache/git/ref/tags/<tag>` (following tag objects to the commit) and cite the version in the comment.
- **Change:** the two jobs exactly as in spec §4.6 and §4.7: `fuzz-build` runs `cargo xtask fuzz-build` after installing the pinned nightly through `cargo xtask fuzz-toolchain` and `cargo-fuzz` through `taiki-e/install-action`, with the Rust cache keyed on `fuzz/Cargo.lock`; `fuzz` has `timeout-minutes: 75`, a corpus restore step (`actions/cache/restore`; `path: fuzz/corpus`, `key: fuzz-corpus-${{ github.run_id }}`, `restore-keys: fuzz-corpus-`), `cargo xtask fuzz all`, a corpus save step (`actions/cache/save`, same path and key) under `if: always()`, and an `if: failure()` upload of `fuzz/artifacts` with `retention-days: 30`.
- **Verify:** `cargo xtask check-workflows` and `cargo xtask typos` pass. Ask the maintainer to approve pushing the branch, then: `gh workflow run nightly.yml --ref m2.1-fuzzing` and read the run: the job is green, the save step logs a save. Run it a second time and confirm the restore step logs a hit on the `fuzz-corpus-` prefix (review focus 3). On the scratch-branch run with the deliberate crash, also confirm the save step still ran (the corpus is kept on failure). For R11's failure path and R12, use a **scratch branch** from this one with a deliberate error in a target (an `assert!(false)` for the nightly check; a compile error for R12), dispatch/push, confirm red plus the uploaded artifact and a failed `fuzz-build` job, then delete the scratch branch. Record the run links for the pull request description.
- **Commit:** `ci: build the fuzz targets on pull requests and fuzz nightly (M2.1)`

### Step 7: Devcontainer, Dependabot and docs (R7, R17, R18)

- **Files:** `.devcontainer/install-tools.sh`, `.github/dependabot.yml`, `CONTRIBUTING.md`, `CHANGELOG.md`, `docs/roadmap.md`.
- **Test first:** `cargo xtask typos` passes before the change; `grep -n "Fuzzing" CONTRIBUTING.md` and `grep -n "/fuzz" .github/dependabot.yml` find nothing (the checks that fail before the change).
- **Change:** add a `cargo` entry for `directory: "/fuzz"` to `dependabot.yml`, weekly and grouped like the root entry (R18). Add to `install-tools.sh` a line installing `cargo-fuzz` and `rustup toolchain install "$(cargo xtask fuzz-toolchain)" --profile minimal` (this runs after the workspace exists, so it can call `xtask`; if the script runs before the repository is mounted, use a postCreate step instead and say which in Deviations). CONTRIBUTING.md: a "Fuzzing" section covering the commands, replaying a crash, test-first handling of a finding, bumping the pin in two sentences, and the R17 note on why `fuzz/` is outside `cargo xtask deny`. CHANGELOG.md under `Unreleased` `### Added`: one line for the fuzz targets and the xtask commands. Roadmap: M2.1 row to `In progress` while the branch is open, with the spec and plan links (the row becomes `Done (PR #n)` in the final commit made after the pull request exists, as M2.0 did).
- **Verify:** `cargo xtask typos` and `cargo xtask fmt --check` pass; the `dependabot.yml` diff is read (R18); every relative link in the new CONTRIBUTING.md text resolves; run `git ls-files | xargs grep -n "nightly-20"` and confirm the date appears only in `xtask/src/fuzz.rs` and the unit-test expectations that derive from the constant (R7).
- **Commit:** `docs: document fuzzing; add it to the devcontainer and Dependabot (M2.1)`

## 3. Final verification

- `cargo xtask ci` on macOS passes, and the output lists `fuzz-build`.
- All CI legs pass on the pull request, including the new `fuzz-build` job.
- The `workflow_dispatch` runs from Step 6 are linked in the pull request: green with a cache save, green with a cache restore, and the red scratch-branch run with its artifact.
- The mutation checks from Steps 2 to 5 are listed with the time each took to be found; all plants are reverted (`git diff --stat main -- libs/ rust-toolchain.toml` is empty).
- Every item in the spec's §9 checklist is ticked, then the spec status becomes `Implemented (PR #n, date)`, the plan status `Historical: executed in PR #n (date); the spec and the code are authoritative`, and the roadmap row `Done (PR #n)`, in one last commit after the pull request exists.
- The pull request title is `feat(fuzz): fuzz the handle crate against a model (M2.1)`, links the spec and plan, and says an assistant was used.

## 4. Deviations

Differences from the plan above (and, where noted, from the spec), in step order.

- **Step 1.** `build_mode` takes `(preflight: Result<(), String>, strict: bool)` instead of `(tools_present, windows, ci_env)`: the skip or fail message then carries the install line for whichever tool is missing, and Windows is reported through the same path (`preflight(windows, tools)`). `TARGETS` is in spec §4.1 order (the order `fuzz all` runs), not sorted; the on-disk test compares sorted copies. `--replay` combined with `--time` is rejected, since the time limit would be ignored.
- **Steps 1 to 5, the `TARGETS` test.** The plan said to `#[ignore]` it until Step 5. The full `ci` also runs ignored tests (`--run-ignored all`), so that failed it. Steps 2 to 4 used a weaker test (every file on disk is listed in `TARGETS`); Step 5 made it an exact match.
- **Step 2.** The pin is `nightly-2026-11-12`, the newest nightly tried, so no stepping back was needed. R16: `#![forbid(unsafe_code)]` compiles with the `fuzz_target!` expansion, so there is no fallback and no `allow`. The pin was first verified only on macOS; push CI run 43301765069 and nightly run 43301837579 verified it on `ubuntu-latest`.
- **Steps 3 to 5, the targets.**
  - A `ProbeNear(usize, i8)` op (an issued handle's slot with a nearby generation) is in all three operation targets, because a random `u64` almost never lands on an in-range index.
  - The pool target's issued list is `Vec<Handle<Tracked>>` (handles are typed by the value type), and `Retain` and the allocators' `Retain` use `k % 8 + 1`.
  - `Drain` became `Drain(u8)` after review: an even value drains fully, an odd one stops early and drops the iterator.
  - `handle_allocator_ops` checks every successful batch against the same number of single allocations replayed on a clone, and, after a refused batch, that the next allocation matches the untouched clone. Batch counts are `n % 65`.
  - `dense_allocator_ops` checks that `retain` visits each live handle exactly once.
  - Measured on a temporary counter (not committed): stale handles were about 1% of ops and full-pool rejects about 0.9%, a little under the plan's "few percent" (review focus 2 and 6).
- **Step 6.** Both jobs install `cargo-fuzz` with `cargo install --locked cargo-fuzz`, not `taiki-e/install-action`, which has no `cargo-fuzz` manifest at the pinned commit. The Rust cache uses `workspaces: . fuzz` (which hashes `fuzz/Cargo.lock`) and comes before the toolchain install, since `cargo xtask fuzz-toolchain` compiles `xtask`. The nightly `stable-toolchain` job installs the same two tools, because the full `ci` it runs includes a `fuzz-build` that is strict under `CI` (found in review).
- **Step 7.** The devcontainer placement is unchanged (`postCreateCommand` runs with the workspace mounted), but the fuzz tool install is non-fatal so a failure there does not abort the container. The roadmap row was already `In progress`. The README's command list was also updated. The "Reviews before a pull request" section in CONTRIBUTING.md and its AGENTS.md pointer were added at the maintainer's request and are outside this spec.
- **Known gaps, accepted.** `cargo xtask fmt --check` does not cover `fuzz/`, which is outside the workspace; the target files are formatted with `rustfmt` by hand. The corpus is not minimised (spec §4.6 relies on cache eviction); the saved corpus was about 0.5 MiB after one night. `cargo xtask ci` on a stable toolchain newer than the pin (the nightly `stable-toolchain` job) currently fails earlier, at a Rust 1.100 clippy lint in code from M0+M1.
