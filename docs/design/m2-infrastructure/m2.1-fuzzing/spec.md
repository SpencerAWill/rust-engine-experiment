# M2.1: Fuzzing

- **Status:** Implemented (PR #14, 2026-11-12)
- **Milestone:** M2 (process and verification infrastructure) · **Depends on:** M2.0 · **Decides:** no ADR
- **Estimated size:** about 800 changed lines, excluding tests, lockfiles and generated files (`fuzz/` targets about 540, `xtask` about 160, workflows, devcontainer and dependabot about 80, docs about 30)

Requirement keywords (MUST, MUST NOT, SHOULD, MAY) follow [RFC 2119](https://www.rfc-editor.org/rfc/rfc2119) as clarified by [RFC 8174](https://www.rfc-editor.org/rfc/rfc8174): they are normative only in uppercase.

## 1. Summary

This chunk adds coverage-guided fuzzing for `engine_handle`: four `cargo-fuzz` targets, a `cargo xtask fuzz` command, a pinned nightly toolchain, and a nightly CI job that runs each target for 10 minutes. It comes now because it is the first M2 infrastructure item and the M2 exit criterion "nightly fuzzing green" depends on it. At the end, `cargo xtask fuzz handle_pool_ops --time 30` runs locally, the nightly workflow runs all four targets with a cached corpus, and a pull request that breaks `fuzz/` fails CI.

## 2. Goals and non-goals

**Goals**

- Fuzz targets that drive `HandlePool`, `HandleAllocator` and `DenseHandleAllocator` with arbitrary operation sequences, compare each against a reference model, and call `check_invariants` after every operation.
- A fuzz target for `Handle::from_bits` and `to_bits`.
- One pinned nightly toolchain, defined in one place, used by local runs and CI.
- A nightly CI job that fails on a finding, uploads the crash input, and caches the corpus between runs.
- A PR-time check that `fuzz/` still compiles, so the targets cannot rot between nightly runs.

**Non-goals**

- No Miri, coverage, mutation testing or Gungraun. They are M2.2 to M2.4 (Miri is deferred, see the roadmap).
- No fuzz targets for `engine_random`, `engine_time` or `engine_math`. The roadmap names only the handle crate; other crates get targets when they gain parse-like or stateful code.
- No shared test-support crate. The op enums and models are duplicated from `libs/engine_handle/tests/model.rs` on purpose (§7).
- No committed corpus and no committed crash artifacts (§4.5).
- No Windows support for `cargo xtask fuzz` (libFuzzer does not work with the MSVC toolchain).
- No automated bumping of the nightly pin.
- No coverage of generation exhaustion (slot retirement): reaching it takes about 4 billion free/allocate cycles on one slot, which fuzzing cannot do and the library offers no test hook for (adding one would be a public API change). Deterministic unit tests already cover it: `generation_exhaustion_retires_slot` in `libs/engine_handle/src/pool.rs` and `exhausted_generation_retires_the_slot` in `allocator.rs` (the dense allocator shares the same slot table). The fuzz models therefore never see a retired slot, and a bug that only retirement exposes is out of reach of this chunk.
- No change to `engine_handle`'s public API, its frozen `Handle::to_bits` layout, or `rust-toolchain.toml`.

## 3. Guide-level explanation

To fuzz one target for a minute:

```sh
cargo xtask fuzz handle_pool_ops --time 60
```

To run every target, 10 minutes each (what the nightly job does):

```sh
cargo xtask fuzz all
```

The first run needs two tools, and the command prints the install line for whichever is missing:

```sh
cargo install --locked cargo-fuzz
rustup toolchain install nightly-2026-11-10 --profile minimal
```

(The actual date is the constant in `xtask/src/fuzz.rs`; §4.2.)

If a target finds a bug, libFuzzer writes the failing input to `fuzz/artifacts/<target>/crash-<hash>` and the command exits non-zero. Reproduce it with `cargo fuzz run <target> fuzz/artifacts/<target>/crash-<hash>` (through `cargo xtask fuzz <target> --replay <path>`). Following the test-first rule in `AGENTS.md`, turn the finding into a failing test in `libs/engine_handle` first (a unit test or a case in `tests/model.rs`), watch it fail, then fix the code. Crash files are not committed.

In CI, the nightly workflow's `fuzz` job fails when a target finds a crash and uploads `fuzz/artifacts/` as a workflow artifact. A new contributor never has to run it: pull requests only check that `fuzz/` compiles.

To bump the nightly, change the one constant in `xtask/src/fuzz.rs`, run `cargo xtask fuzz-build`, and mention the bump in the pull request. CONTRIBUTING.md says this in two sentences.

## 4. Reference-level design

### 4.1 Layout

```text
fuzz/
  Cargo.toml            standalone package `engine-fuzz`; depends on engine_handle (path, feature
                        `test-invariants`), libfuzzer-sys, arbitrary (derive)
  Cargo.lock            committed
  .gitignore            corpus/, artifacts/, target/
  fuzz_targets/
    handle_pool_ops.rs
    handle_allocator_ops.rs
    dense_allocator_ops.rs
    handle_from_bits.rs
xtask/src/fuzz.rs       the `fuzz` and `fuzz-build` commands and the nightly pin
```

`fuzz/` stays excluded from the workspace (`Cargo.toml` already says so), so its dependencies do not enter the main `Cargo.lock` and workspace lints, `check-statics` and `check-unsafe` do not see it. Each target is a `[[bin]]` with `test = false`, `doc = false`, `bench = false`, as `cargo fuzz init` generates.

### 4.2 The nightly pin

`xtask/src/fuzz.rs` holds `const FUZZ_NIGHTLY: &str = "nightly-YYYY-MM-DD";`, a dated nightly. Every `cargo-fuzz` invocation by `xtask` passes `+<FUZZ_NIGHTLY>`. CI does not duplicate the date: the workflow step runs `cargo xtask fuzz-toolchain`, which prints the pin, and installs that toolchain (`rustup toolchain install "$(cargo xtask fuzz-toolchain)" --profile minimal`). The date is chosen at implementation time as the most recent nightly that builds `libfuzzer-sys` and the sanitizer runtime; it is not `rust-toolchain.toml`'s channel (that file pins stable 1.99.1 and stays so).

### 4.3 Targets

All four targets MUST be deterministic given their input bytes: they use `BTreeMap`/`BTreeSet`/`Vec` only, never `HashMap`, clocks or thread-locals, so a crash reproduces exactly (ADR 0003).

**Operation targets** (`handle_pool_ops`, `handle_allocator_ops`, `dense_allocator_ops`). Each defines `#[derive(Arbitrary, Debug)] enum Op` and `#[derive(Arbitrary, Debug)] struct Input { capacity: Option<u8>, ops: Vec<Op> }`, and a `fuzz_target!(|input: Input| { ... })` body. The body creates the subject (`new()` when `capacity` is `None`, otherwise `with_fixed_capacity(c % 17)`, so the limit is between 0 and 16 and the full-pool paths are reached quickly), a model that knows the limit, and a `Vec<Handle<_>>` of every handle issued so far. For each op it applies the operation to both the subject and the model, asserts they agree, then calls `check_invariants().expect(...)`. Handle selection ops take an arbitrary `usize` and index into the issued list modulo its length (skipped when empty), which makes stale handles common. Fallible ops (`TryInsert`, `TryAllocate`, `TryAllocateMany`) always run: at the limit they MUST fail exactly as documented (for the pool, `CapacityError` carrying the rejected value and the limit; for the allocators, `None`) and leave the subject and model unchanged. Infallible ops that panic at the limit (`Insert`, `Allocate`, `AllocateMany`) are skipped when the model is at the limit, so the target never trips a documented panic. Ops that take a raw `u64` build a handle with `Handle::from_bits` (skipped on `None`) to probe handles the subject never issued.

| Target | Subject | Model | Ops |
|---|---|---|---|
| `handle_pool_ops` | `HandlePool<Tracked>` | `BTreeMap<u64, u16>` keyed by `to_bits` | `Insert(u16)`, `TryInsert(u16)`, `Remove(usize)`, `Get(usize)`, `GetMut(usize, u16)`, `GetDisjoint(usize, usize)`, `Retain(u16)`, `Drain`, `Clear`, `Iterate`, `Probe(u64)` |
| `handle_allocator_ops` | `HandleAllocator<()>` | `BTreeSet<u64>` | `Allocate`, `TryAllocate`, `AllocateMany(u8)`, `TryAllocateMany(u8)`, `Free(usize)`, `IsAlive(usize)`, `Clear`, `Probe(u64)` |
| `dense_allocator_ops` | `DenseHandleAllocator<()>` | `BTreeSet<u64>` | `Allocate`, `TryAllocate`, `Free(usize)`, `IsAlive(usize)`, `Retain(u8)`, `Clear`, `Iterate`, `Probe(u64)` |

In `handle_pool_ops` the value type is `Tracked { value: u16, drops: Rc<Cell<u32>> }` whose `Drop` increments the shared counter (an `Rc`, not a `thread_local!`, to keep the determinism rules). After every op the target asserts `inserted == model.len() + drops.get()` (every value ever inserted is either still in the pool or was dropped exactly once), which catches values leaked or dropped twice by `remove`, `retain`, `drain` and `clear`. When the subject is dropped at the end of the input the same equation with `model.len() == 0` holds. The model stores plain `u16`s; the comparison reads `Tracked::value`.

Agreement means: returned values match the model's (an issued handle is alive exactly when the model holds its bits), `len()` equals the model size, `iter()` yields exactly the model's handles (as a set; order is checked only for being stable across two consecutive `iter()` calls, because the dense order is an implementation detail), and no handle ever repeats a `(index, generation)` pair the model has already seen as issued. `AllocateMany`'s count is capped at 64 and `Vec<Op>` length is whatever `arbitrary` yields; libFuzzer's `-max_len` bounds total size. A `Probe` of bits the subject never issued MUST NOT panic and MUST report "not alive" unless the bits equal an issued live handle.

**`handle_from_bits`.** `fuzz_target!(|bits: u64| { ... })` asserts: `Handle::<u8>::from_bits(bits)` is `None` exactly when the low 32 bits are 0; when `Some(h)`, `h.to_bits() == bits`, `h.index() == (bits >> 32) as u32`, `h.generation().get() == bits as u32`, `Handle::<u8>::try_from(bits)` and `From`/`TryFrom<u64>` agree with `from_bits`, and `Handle::<u8>::from_bits(h.to_bits()) == Some(h)`. It also checks the ordering claim in the docs: for two inputs derived from the same bytes, `Handle` order equals bit order. (The target takes `(u64, u64)` for that.)

### 4.4 `cargo xtask fuzz`

```text
cargo xtask fuzz <target|all> [--time <secs>] [--replay <path>]
cargo xtask fuzz-build
cargo xtask fuzz-toolchain
```

- `fuzz <target>`: runs `cargo +<pin> fuzz run --sanitizer none --debug-assertions <target> -- -max_total_time=<secs>` in `fuzz/`. The sanitizer is off because the code under test is entirely safe Rust, so AddressSanitizer finds nothing it could not otherwise, and costs throughput; debug assertions and overflow checks are turned on explicitly so logic and arithmetic bugs still fail. Revisit this when the first `unsafe` crate arrives (the roadmap's Miri trigger). `--time` defaults to 60 for a named target and to 600 for `all` (the nightly default, per the M0+M1 spec §4.7 and §4.8).
- `fuzz all`: runs every target in the order of §4.1; it runs all of them even if one fails, then exits non-zero if any failed, so one night reveals all findings.
- `fuzz <target> --replay <path>`: runs `cargo +<pin> fuzz run --sanitizer none --debug-assertions <target> <path>` (a single input, no time limit).
- `fuzz-build`: runs `cargo +<pin> fuzz build --sanitizer none --debug-assertions`, which compiles all targets without running them.
- `fuzz-toolchain`: prints the pin and exits 0. It exists only for the workflow.
- An unknown target name is an error that lists the valid names, which come from one `const TARGETS: [&str; 4]` that a unit test checks against `fuzz/fuzz_targets/*.rs` on disk.
- The commands check for `cargo-fuzz` (`require_cargo_subcommand("fuzz", "cargo install --locked cargo-fuzz")`) and for the pinned toolchain (`rustup run <pin> rustc --version`, hint `rustup toolchain install <pin> --profile minimal`). On Windows the command returns `fuzzing is not supported on Windows (libFuzzer needs a non-MSVC toolchain); use the devcontainer or WSL`. The flag parser follows the `parse`/`only` style in `xtask/src/main.rs` and has unit tests like the existing commands.

`fuzz-build` is also added to the **full** `cargo xtask ci` (not `--quick`) on Linux and macOS, so `cargo xtask ci` stays the single entry point; the `fuzz` run itself is never part of `ci`. Inside `ci` the step is lenient about missing tools: if `cargo-fuzz` or the pinned nightly is absent, or on Windows, it prints a loud `skipped fuzz-build: <reason and install line>` note and continues, so a contributor is not forced to install a nightly for unrelated work. When the `CI` environment variable is set (GitHub Actions sets it), a missing tool is an error instead, so CI cannot silently skip it. Run directly as `cargo xtask fuzz-build`, the command is always strict.

### 4.5 Corpus and findings

- The corpus lives in `fuzz/corpus/<target>/`, is git-ignored and is not committed. There is no seed corpus: the targets are structure-aware (`Arbitrary`), so random bytes already decode to valid op sequences.
- Crash inputs are not committed either. A finding becomes a unit or property test in `libs/engine_handle` before the fix (the `AGENTS.md` test-first rule); the fuzz artifact is evidence for the bug report, and is retained as a workflow artifact for 30 days.

### 4.6 Nightly CI

A new `fuzz` job in `.github/workflows/nightly.yml`:

```yaml
fuzz:
  runs-on: ubuntu-latest
  timeout-minutes: 75
  steps:
    - uses: actions/checkout@<pinned>            # persist-credentials: false
    - run: rustup show                           # installs the stable toolchain from rust-toolchain.toml
    - run: rustup toolchain install "$(cargo xtask fuzz-toolchain)" --profile minimal
    - uses: taiki-e/install-action@<pinned>      # tool: cargo-fuzz
    - uses: actions/cache/restore@<pinned>       # path: fuzz/corpus
        # key: fuzz-corpus-${{ github.run_id }}; restore-keys: fuzz-corpus-
    - run: cargo xtask fuzz all                  # 10 min x 4 targets
    - uses: actions/cache/save@<pinned>          # if: always(); same path and key
    - uses: actions/upload-artifact@<pinned>     # if: failure(); path: fuzz/artifacts
```

- A finding fails the job (red nightly) and uploads `fuzz/artifacts`. The job is not a required check, so it never blocks a pull request. It does not use `continue-on-error`, unlike `stable-toolchain`, because a red job is how a finding becomes visible.
- The corpus cache uses a per-run key with a prefix restore, the usual way to get a rolling cache: each run restores the newest corpus and saves its own. Restore and save are separate steps because the combined `actions/cache` action saves only when the job succeeds, and the runs that find a crash are exactly the ones whose corpus is worth keeping; the save step runs under `if: always()`. The per-repository cache quota evicts old entries, so no cleanup step is needed.
- All actions are pinned to full commit hashes, the workflow keeps its top-level `permissions: contents: read`, and the checkout sets `persist-credentials: false`, so `cargo xtask check-workflows` keeps passing.

### 4.7 PR-time compile check

`ci.yml` gets a `fuzz-build` job (Ubuntu): checkout, `rustup show`, install the pinned nightly as above, `taiki-e/install-action` for `cargo-fuzz`, then `cargo xtask fuzz-build`. It uses the usual Rust cache action keyed on `fuzz/Cargo.lock`. It is a normal job, so the branch ruleset may add it as a required check (a separate Pulumi change in `infra/github`, out of scope here and noted in §6).

### 4.8 Devcontainer and docs

`.github/dependabot.yml` gets a `cargo` entry for `directory: "/fuzz"` (weekly, grouped like the root entry), so `fuzz/Cargo.lock` is updated and advisories on its dependencies appear as Dependabot alerts. This is the substitute for `cargo xtask deny` (R17). `.devcontainer/install-tools.sh` installs `cargo-fuzz` and the pinned nightly (read through `cargo xtask fuzz-toolchain`, so the date is not duplicated). `CONTRIBUTING.md` gets a short "Fuzzing" section (run, replay, bump the pin). `docs/roadmap.md` M2.1 moves to `Drafting`, then `In progress`, then `Done` as the process in M2.0 says. `CHANGELOG.md` gets one line.

## 5. Requirements

- **M2.1-R1** The repository MUST contain a `fuzz/` cargo-fuzz package, excluded from the workspace, with targets `handle_pool_ops`, `handle_allocator_ops`, `dense_allocator_ops` and `handle_from_bits`.
- **M2.1-R2** Each operation target MUST apply every op to both the subject and a reference model and MUST assert agreement after each op.
- **M2.1-R3** Each operation target MUST call `check_invariants()` after every op and MUST fail on `Err`.
- **M2.1-R4** The operation targets MUST exercise handles the subject did not issue (via `Probe(u64)`) and stale handles (via modulo selection over every handle issued so far), and the subject MUST NOT panic on either.
- **M2.1-R5** `handle_from_bits` MUST check that `from_bits` returns `None` exactly when the low 32 bits are 0, that `to_bits` inverts `from_bits`, and that `Handle` order equals bit order.
- **M2.1-R6** The fuzz targets MUST be deterministic given their input: no `HashMap`, `HashSet`, clock or thread-local use.
- **M2.1-R7** The nightly toolchain MUST be defined once, in `xtask/src/fuzz.rs`, and every other consumer (local runs, CI, devcontainer) MUST obtain it from there.
- **M2.1-R8** `cargo xtask fuzz <target> [--time <secs>] [--replay <path>]` MUST run the named target for the given time (default 60 s, or 600 s for `all`), and `fuzz all` MUST run every target even if one fails and exit non-zero if any failed.
- **M2.1-R9** `cargo xtask fuzz` MUST print an actionable install hint when `cargo-fuzz` or the pinned nightly is missing, and an actionable unsupported message on Windows.
- **M2.1-R10** `cargo xtask fuzz-build` MUST compile all targets and MUST be strict about missing tools when run directly. It MUST be part of the full `cargo xtask ci` on Linux and macOS, where a missing `cargo-fuzz`, a missing pinned nightly or a Windows host MUST print a skip note and continue unless the `CI` environment variable is set, in which case it MUST fail.
- **M2.1-R11** The nightly workflow MUST run `cargo xtask fuzz all`, MUST fail when a target finds a crash, MUST upload `fuzz/artifacts` on failure with 30-day retention, and MUST cache `fuzz/corpus` between runs, saving it even when the job fails.
- **M2.1-R12** A pull request that makes `fuzz/` fail to compile MUST fail the `fuzz-build` job in `ci.yml`.
- **M2.1-R13** The workflow changes MUST keep `cargo xtask check-workflows` passing (pinned action hashes, top-level `permissions`, `persist-credentials: false`).
- **M2.1-R14** The chunk MUST NOT change any public item of `engine_handle`, the `Handle::to_bits` layout, or `rust-toolchain.toml`.
- **M2.1-R15** The `xtask` change MUST include unit tests for argument parsing and for the `TARGETS` constant matching the files in `fuzz/fuzz_targets/`.
- **M2.1-R16** The fuzz crate root MUST carry `#![forbid(unsafe_code)]`. If the `fuzz_target!` expansion makes that impossible on the pinned nightly, the root MUST instead carry `#![deny(unsafe_code)]` with a `#[allow(unsafe_code)]` scoped to the macro invocations only and a comment saying why, and the plan's Deviations section MUST record it. Either way there is no ADR 0001 allowlist entry, because the allowlist covers workspace crates and `fuzz/` is excluded.
- **M2.1-R17** `fuzz/` MUST NOT be added to `cargo xtask deny`; the reason (a developer tool that is never built into or shipped with an engine artifact, and `libfuzzer-sys`'s `NCSA` licence is outside ADR 0002's allow list) MUST be written in `CONTRIBUTING.md`'s Fuzzing section.
- **M2.1-R18** `.github/dependabot.yml` MUST watch `fuzz/Cargo.lock` with a weekly `cargo` entry for `/fuzz`.
- **M2.1-R19** `handle_pool_ops` MUST use a drop-counting value type and MUST assert after every op that every inserted value is either in the model or has been dropped exactly once.
- **M2.1-R20** The operation targets MUST run against both unbounded and fixed-capacity (0 to 16) subjects, and MUST check that fallible ops at the limit fail as documented without changing state.
- **M2.1-R21** `cargo xtask fuzz` and `fuzz-build` MUST pass `--sanitizer none --debug-assertions` to `cargo fuzz`.

## 6. Cross-cutting concerns

- **Determinism:** the targets are deterministic given their input (R6). The nightly pin does not affect simulation output: `fuzz/` is not part of any built engine artifact.
- **`unsafe`:** none written. The fuzz crate root carries `#![forbid(unsafe_code)]`, with the fallback in R16 if the `fuzz_target!` expansion forbids it.
- **Concurrency:** n/a, because the targets are single-threaded and add no shared state.
- **Heap allocation:** n/a, because fuzz code may allocate freely and library code is unchanged.
- **`no_std` and wasm:** n/a, because `fuzz/` is a host-only package. `engine_handle`'s `no_std` and wasm builds are unaffected since the `test-invariants` feature is enabled only from `fuzz/`.
- **Frozen formats:** `Handle::to_bits` is unchanged. `handle_from_bits` adds a check that the layout still holds, which complements the golden tests.
- **Public API changes:** none (R14). `xtask` gains `fuzz`, `fuzz-build` and `fuzz-toolchain`, which are developer-tool commands, not engine API.
- **FFI exposure:** n/a.
- **Performance:** not measured. The sanitizer is off for throughput (§4.4). Cost is CI time: about 41 minutes a night on one Ubuntu runner, and about 3 to 5 minutes for the PR-time `fuzz-build` job (cold nightly toolchain install, warm cache after the first run).
- **Documentation:** CONTRIBUTING.md "Fuzzing" section, roadmap row, `xtask` help text, CHANGELOG line, doc comments on every `xtask` item (`#![warn(missing_docs)]` already applies to library code; `xtask` follows the same habit).
- **Policy changes:** none to existing ADRs. Two decisions made at acceptance: `fuzz/` stays out of `cargo xtask deny` (R17, so ADR 0002 is not amended), and the `unsafe` lint fallback in R16. A follow-up outside this chunk is to make `fuzz-build` a required check in `infra/github`.

## 7. Alternatives considered

- **Share the op enums and model with `tests/model.rs` through a new test-support crate.** Rejected for this chunk: it adds a crate (ADR 0004 weighs in), couples the proptest suite to the fuzz targets' needs, and the fuzz model is small. Revisit if a third consumer of the same model appears.
- **Add `Arbitrary` derives to `engine_handle` behind a feature.** Rejected: it puts a fuzzing dependency in the library's manifest and feature surface to save about 30 lines of duplicated enums.
- **A single combined ops target.** Rejected: a crash would not say which type failed, and the corpus for one type would be diluted by inputs for the others.
- **Upload artifacts but keep the nightly job green.** Rejected: a green job hides findings.
- **Fresh corpus every night.** Rejected: each night would re-discover the shallow coverage; the cache makes depth accumulate.
- **Run the fuzz targets on pull requests for a short time.** Rejected for now: it makes PR time non-deterministic and flaky, and 30 seconds finds little. A compile check is enough to stop rot.
- **Automated nightly bump (Dependabot-style job).** Rejected for now: more machinery than a constant that changes a few times a year.
- **Commit crash artifacts as regression inputs.** Rejected: the proptest and unit tests are the regression suite, run on stable on every platform, including wasm.

## 8. Verification

| Requirement | Verified by |
|---|---|
| R1 | `ls fuzz/fuzz_targets` shows the four files; `cargo metadata` for the workspace lists no `engine-fuzz` member; unit test on `TARGETS` (R15) |
| R2, R3 | Review of each target body; mutation check: temporarily break `HandlePool::remove` (return `None` for a live handle) and confirm `handle_pool_ops` finds it within 60 s, then revert; same for each allocator |
| R4 | Review of the `Probe` and modulo-selection ops; the same mutation check using a stale-handle bug (skip the generation bump in `free`) |
| R5 | `cargo xtask fuzz handle_from_bits --time 30` passes; mutation check on `from_bits` (accept generation 0) is found |
| R6 | Review plus `grep -rnE "HashMap|HashSet|Instant|SystemTime|thread_local" fuzz/` returns nothing |
| R7 | `grep -rn "nightly-20" .` finds the date only in `xtask/src/fuzz.rs` (plus any lockfile or cache metadata) |
| R8 | Unit tests for the parser (defaults 60/600, `--replay`, unknown target, missing value); run `cargo xtask fuzz handle_pool_ops --time 10` and `fuzz all --time 5` locally and read the output |
| R9 | Run with `cargo-fuzz` hidden from `PATH` and read the hint; run with an uninstalled pin and read the hint; Windows branch covered by a unit test on the pure message function |
| R10 | `cargo xtask ci` output lists `fuzz-build`; unit tests on `ci_steps(false)` and `ci_steps(true)` and on the pure skip-or-fail decision (tools present or absent, `CI` set or unset, Windows); run `cargo xtask ci` with `cargo-fuzz` hidden from `PATH` and read the skip note, then again with `CI=1` and read the failure |
| R11 | `workflow_dispatch` run of nightly on the branch: green run with cache saved; a second run restores it (read the cache step log); a deliberately broken target on a scratch branch turns the job red and the artifact appears |
| R12 | A scratch commit that introduces a compile error in a target fails `fuzz-build` in `ci.yml` |
| R13 | `cargo xtask check-workflows` |
| R14 | `git diff --stat main -- libs/ rust-toolchain.toml` is empty; `cargo xtask test` passes; `cargo public-api` is not available here, so the diff is read |
| R15 | `cargo xtask test` runs the new unit tests |
| R16 | Experiment on the pinned nightly (plan Step 2); `grep -n "unsafe_code" fuzz/fuzz_targets/*.rs` and read any `allow` and its comment |
| R17 | `cargo xtask deny` output is unchanged; read the CONTRIBUTING.md section |
| R18 | Read the `dependabot.yml` diff; after merge, a Dependabot run lists the `/fuzz` directory |
| R19 | Mutation check: make `HandlePool::clear` forget its values (`mem::forget`), and confirm `handle_pool_ops` finds the leak within 60 s |
| R20 | Review of the `Input` struct and fallible ops; mutation check: let `try_insert` accept one value past the fixed capacity, found within 60 s |
| R21 | Unit tests on the argument builders |

## 9. Done when

- [x] All requirements in §5 are verified as in §8, with the evidence (command output, run links) in the pull request description. R18's check (a Dependabot run listing `/fuzz`) can only happen after merge and is still pending; R9's Windows branch is covered by a unit test only.
- [x] `cargo xtask ci` passes locally on macOS and in CI on all legs.
- [x] One `workflow_dispatch` run of the nightly `fuzz` job is green, and a second run shows the corpus restored.
- [x] The mutation checks in §8 (R2 to R5, R19, R20) each found their planted bug, and the plants are reverted.
- [x] CONTRIBUTING.md, the roadmap row (to `Done`), CHANGELOG and this spec's status (`Implemented (PR #n, date)`) are updated.
- [x] The pull request title is `feat(fuzz): fuzz the handle crate against a model (M2.1)` and says an assistant was used.

## 10. Open questions

None. Decided at acceptance (2026-11-12): the `unsafe` lint fallback (R16), leaving `fuzz/` out of `cargo xtask deny` (R17), and 30-day retention for nightly crash artifacts (§4.5). Decided in the gap review (same day): generation exhaustion stays an acknowledged blind spot (§2), the targets cover fixed capacities (R20), the pool target counts drops (R19), `ci` skips a missing `cargo-fuzz` locally but not under `CI` (R10), `/fuzz` joins Dependabot (R18), and the sanitizer is off (R21).

## 11. Prior art and sources

- [The Rust Fuzz Book: cargo-fuzz](https://rust-fuzz.github.io/book/cargo-fuzz.html) for the package layout and commands.
- [`arbitrary` crate documentation](https://docs.rs/arbitrary) for structure-aware inputs.
- [LLVM libFuzzer documentation](https://llvm.org/docs/LibFuzzer.html) for `-max_total_time` and `-max_len`.
- [Model-based (stateful) testing, as in the existing `libs/engine_handle/tests/model.rs`](../../../../libs/engine_handle/tests/model.rs) for the op and model structure reused here.
- [M0+M1 spec §4.7, §4.8 and §4.12](../../m0-m1-foundations/spec.md) for the original fuzzing plan; [ADR 0001](../../../adr/0001-unsafe-policy.md), [ADR 0002](../../../adr/0002-originality-and-dependency-licensing.md), [ADR 0003](../../../adr/0003-same-build-determinism.md).
- No code copied from other projects (ADR 0002).
