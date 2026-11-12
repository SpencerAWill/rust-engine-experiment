# M0+M1 Foundations Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the engine's engineering foundation (workspace, policy, xtask, CI, ADRs) and the first four core crates: `engine_handle`, `engine_random`, `engine_time`, and `engine_math`. Add the `engine` facade, a deterministic headless sample, and comparison benchmarks.

**Architecture:**
- A flat Cargo workspace (`libs/*`, `examples/*`, `benches`, `xtask`).
- Small dependency-free `no_std` leaf crates, a glam-based math crate, and a facade with a prelude.
- Policies (no `unsafe`, determinism bans, no global state) are enforced mechanically through lints, `clippy.toml`, and `xtask` checks.
- `cargo xtask` is the single entry point that CI and humans both run.

**Tech Stack:** Rust 1.97.0 (edition 2024, resolver 3), glam 0.30, proptest (no default features), criterion + critcmp, dhat, syn (xtask), GitHub Actions, cargo-nextest, cargo-deny, wasmtime.

**Spec:** `docs/superpowers/specs/2026-11-11-m0-m1-foundations-design.md`. This plan argues from the spec; read both. "Spec §X" means "copy the exact signatures and semantics from that section".

## Global Constraints

- Toolchain pinned to `1.97.0`. `edition = "2024"`, `resolver = "3"`. `rust-version = "1.97"`.
- Every crate: `version = "0.1.0"`, `publish = false`, `license = "MIT OR Apache-2.0"`, `[lints] workspace = true`.
- Every crate root (`src/lib.rs` / `src/main.rs`) starts with `#![forbid(unsafe_code)]`. `unsafe-allowlist.toml` and `concurrency-allowlist.toml` stay empty.
- **No code is copied from other projects.** Algorithms are implemented from their published descriptions. Competitor crates appear only as dev-dependencies (benchmarks, test oracles).
- No `std::collections::HashMap`/`HashSet`/`RandomState`, `Instant::now`, `SystemTime::now`, `thread_local!`, or inherent transcendental float methods in library code. The only exceptions: `SystemClock` and `engine_math::scalar`, each with an item-level `#[expect(..., reason = "...")]`.
- No `static` with interior mutability and no `thread_local!` (ADR 0008).
- Every API with a documented panic has `#[track_caller]`, a `# Panics` doc section, an actionable message, and a fallible twin (`try_*` / `checked_*`).
- Every public item is documented. Every public type has a runnable doctest example.
- Leaf crates `engine_handle` (`no_std` + `alloc`), `engine_random` (`no_std`), and `engine_time` (`no_std`, default `std` feature) must build for `thumbv7em-none-eabihf` with `--no-default-features`.
- All integer PRNG sampling uses the 64-bit path on every target (spec §5.2). Results never depend on pointer width.
- Every `proptest!` block uses a config with `failure_persistence: None` when `cfg(target_os = "wasi")`, because there is no writable directory under wasmtime. Define it once per test file as a `fn config() -> ProptestConfig`.
- Conventional commits; every commit message ends with the `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>` trailer. **Commit by explicit path, never `git commit -a`.**
- Work on branch `m0-m1-foundations`. Push at the end of each phase; **ask the user before the first push**.
- Explanations for the user (intermediate Rust): each phase summary explains the key Rust/engine decisions, such as sealed traits, `PhantomData` variance, `#[track_caller]`, and ownership choices.

## Review Focus

These inputs and conditions are implied by the spec but not exercised by its listed tests. Each now has a test in the owning task.

1. **Zero-sized values** in `HandlePool<()>` / `HandlePool<ZST>`: insert/remove/iterate work exactly as for sized `T` (Task 6: `zero_sized_values_work`).
2. **Full-width signed and unsigned ranges** (`i64::MIN..=i64::MAX`, `i8::MIN..=i8::MAX`, `0..=u64::MAX`): no overflow, and results cover negatives (Task 11: `full_range_signed_and_unsigned`).
3. **Huge elapsed times** passed to `FixedTimestep::accumulate` (e.g. `Duration::MAX`, twice) must not panic. They yield `max_steps`, and `dropped_time` saturates (Task 14: `accumulate_huge_elapsed_saturates_without_panic`).
4. **A panicking closure inside `HandlePool::retain`** must leave the pool consistent: `assert_invariants` passes after `catch_unwind` (Task 7: `retain_panic_leaves_pool_consistent`).
5. **Degenerate twist in `Transform::to_2d`** (a 180° rotation about X, where the twist about Z is undefined): the result is finite with rotation 0.0, documented (Task 17: `to_2d_of_half_turn_about_x_is_finite`).

---

## File Structure

```
Cargo.toml                        workspace manifest (members, package defaults, deps, lints, profiles)
rust-toolchain.toml               1.97.0 + components + targets
rustfmt.toml  clippy.toml  deny.toml
unsafe-allowlist.toml  concurrency-allowlist.toml
LICENSE-MIT  LICENSE-APACHE  README.md
.devcontainer/install-tools.sh    dev tool install (devcontainer postCreate)
.github/workflows/ci.yml  .github/workflows/nightly.yml  .github/dependabot.yml
docs/adr/0001…0009-*.md  docs/benchmarks.md
xtask/src/main.rs                 arg parsing + command dispatch
xtask/src/cmd.rs                  run-a-cargo/tool-command helpers, install hints
xtask/src/check_unsafe.rs         crate-root forbid check + allowlist parsing
xtask/src/check_statics.rs        syn-based global-state scan
xtask/src/prng_stats.rs           PRNG byte stream to stdout
libs/engine_handle/src/{lib.rs, handle.rs, pool.rs, pool/iter.rs, allocator.rs, error.rs}
libs/engine_handle/tests/{model.rs, auto_traits.rs, panics.rs}
libs/engine_random/src/{lib.rs, prng.rs, splitmix.rs, sample.rs}
libs/engine_random/tests/{oracle.rs, golden.rs, edge_cases.rs, statistics.rs, panics.rs}
libs/engine_time/src/{lib.rs, tick.rs, timestamp.rs, clock.rs, fixed.rs, error.rs}
libs/engine_time/tests/{fixed_props.rs, panics.rs}  libs/engine_time/benches/fixed_timestep.rs
libs/engine_math/src/{lib.rs, scalar.rs, helpers.rs, transform.rs, world.rs, transform2d.rs, axis.rs, convert.rs, layout.rs}
libs/engine_math/tests/{transform_props.rs, world_props.rs, transform2d_props.rs, convert.rs}
libs/engine/src/lib.rs
examples/fixed_step_sim/src/{lib.rs, main.rs}
examples/fixed_step_sim/tests/{determinism.rs, snapshot.rs, allocations.rs}
examples/fixed_step_sim/benches/sim.rs
benches/Cargo.toml  benches/benches/{handles.rs, prng.rs}
```

Each `lib.rs` holds only crate docs, the attributes, `mod` declarations, and re-exports.

---

# Phase 1: Workspace and policy (M0)

### Task 1: Workspace skeleton, configuration, licenses, tools

**Files:**
- Delete: `libs/placeholder/` (already deleted in the working tree; folded in here), `Cargo.lock` (regenerated)
- Create/Modify: `Cargo.toml`, `rust-toolchain.toml`, `rustfmt.toml`, `clippy.toml`, `deny.toml`, `unsafe-allowlist.toml`, `concurrency-allowlist.toml`, `LICENSE-APACHE`
- Rename: `LICENSE` → `LICENSE-MIT`
- Create: `xtask/Cargo.toml`, `xtask/src/main.rs`, `xtask/src/cmd.rs`, `.cargo/config.toml`

**Interfaces:**
- Produces: `cargo xtask <cmd>` alias. Commands in this task: `fmt [--check]`, `clippy`, `test [--release] [--include-ignored]`, `doc`, `deny`, `ci --quick`. Later tasks add commands to the same `match` in `xtask/src/main.rs`.
- Produces in `xtask/src/cmd.rs`: `fn run(program: &str, args: &[&str], env: &[(&str, &str)]) -> Result<(), String>`, which streams output and returns `Err` with the failing command line; and `fn require_tool(binary: &str, install_hint: &str) -> Result<(), String>`.

- [ ] **Step 1: Install host tools**

Run: `cargo install --locked cargo-nextest cargo-deny critcmp wasmtime-cli`
Expected: four binaries on `PATH` (`cargo nextest --version`, `cargo deny --version`, `critcmp --version`, `wasmtime --version` all succeed). The Rust targets come from `rust-toolchain.toml` in Step 2.

- [ ] **Step 2: Write the configuration files**

`rust-toolchain.toml`:
```toml
[toolchain]
channel = "1.97.0"
components = ["rustfmt", "clippy"]
targets = ["thumbv7em-none-eabihf", "wasm32-unknown-unknown", "wasm32-wasip1"]
```

`Cargo.toml`:
```toml
[workspace]
resolver = "3"
members = ["libs/*", "examples/*", "xtask"]   # "benches" is added in Task 9 when that crate exists
exclude = ["fuzz"]

[workspace.package]
version = "0.1.0"
edition = "2024"
rust-version = "1.97"
license = "MIT OR Apache-2.0"
repository = "https://github.com/SpencerAWill/rust-engine-experiment"
publish = false

[workspace.dependencies]
# internal crates are added here as they are created (path + version)
glam = { version = "0.30", default-features = false }
proptest = { version = "1", default-features = false, features = ["std"] }  # no fork/timeout: WASI (spec §4.7)

[workspace.lints.rust]
unsafe_code = "deny"
missing_docs = "warn"
unreachable_pub = "warn"
missing_debug_implementations = "warn"
rust_2018_idioms = { level = "warn", priority = -1 }

[workspace.lints.clippy]
pedantic = { level = "warn", priority = -1 }
module_name_repetitions = "allow"   # engine_handle::HandlePool etc. read better with the prefix
must_use_candidate = "allow"        # #[must_use] is applied deliberately (spec §4.4)
undocumented_unsafe_blocks = "deny"
unwrap_used = "warn"
dbg_macro = "warn"
todo = "warn"
print_stdout = "warn"

[workspace.lints.rustdoc]
broken_intra_doc_links = "deny"
private_intra_doc_links = "warn"

[profile.dev]
opt-level = 1
[profile.dev.package."*"]
opt-level = 3
[profile.bench]
lto = "thin"
codegen-units = 1
debug = true
```

`clippy.toml`: copy spec §4.9 verbatim. Append to `disallowed-methods` one entry per inherent method below, for **both** `f32` and `f64`, each with `reason = "transcendental: use engine_math::scalar::<name> (spec §4.9)"`: `sin cos tan sin_cos asin acos atan atan2 sinh cosh tanh asinh acosh atanh exp exp2 exp_m1 ln ln_1p log log2 log10 powf powi cbrt hypot`. (`asinh`/`acosh`/`atanh`/`log` go beyond the spec's list, in keeping with its "all transcendental math" rule. Task 15 adds them to the funnel too.)

`deny.toml`:
```toml
[advisories]
version = 2
[licenses]
version = 2
allow = ["MIT", "Apache-2.0", "Apache-2.0 WITH LLVM-exception", "BSD-2-Clause", "BSD-3-Clause", "ISC", "Zlib", "Unicode-3.0"]
[bans]
multiple-versions = "warn"
[sources]
unknown-registry = "deny"
unknown-git = "deny"
```

`rustfmt.toml`: `edition = "2024"` and `newline_style = "Unix"` (stable options only).

`unsafe-allowlist.toml` / `concurrency-allowlist.toml`: a header comment explaining the `[[crate]] name = "…" adr = "docs/adr/…"` format (spec §4.3, §4.11), with no entries.

`LICENSE-APACHE`: the standard Apache License 2.0 text. Rename `LICENSE` with `git mv LICENSE LICENSE-MIT`.

- [ ] **Step 3: Write the failing xtask test**

In `xtask/src/main.rs`, add a `#[cfg(test)] mod tests` with:
```rust
#[test]
fn parses_known_commands() {
    assert_eq!(parse(&["fmt", "--check"]), Ok(Command::Fmt { check: true }));
    assert_eq!(parse(&["ci", "--quick"]), Ok(Command::Ci { quick: true }));
    assert_eq!(parse(&["test", "--release", "--include-ignored"]),
               Ok(Command::Test { release: true, include_ignored: true }));
    assert!(parse(&["nope"]).unwrap_err().contains("unknown command `nope`"));
}
```

- [ ] **Step 4: Run it to verify it fails**

Run: `cargo test -p xtask`
Expected: FAIL to compile (`parse`/`Command` not defined).

- [ ] **Step 5: Implement xtask**

- `xtask/Cargo.toml`: package `xtask` with workspace inheritance, no dependencies yet, plus `[lints] workspace = true`.
- `.cargo/config.toml`: `[alias] xtask = "run --package xtask --"`.
- `main.rs`:
  - `#![forbid(unsafe_code)]`, plus a crate-level `#![allow(clippy::print_stdout, clippy::disallowed_types, clippy::disallowed_methods)]` with the comment `// developer tool, not simulation code`;
  - `enum Command`;
  - `fn parse(args: &[&str]) -> Result<Command, String>`, hand-rolled with no argument-parser dependency;
  - `fn main() -> ExitCode`.
- Command lines:

| Command | Runs |
|---|---|
| `fmt` | `cargo fmt --all` (with `--check` when requested) |
| `clippy` | `cargo clippy --workspace --all-targets --all-features -- -D warnings` |
| `test` | `cargo nextest run --workspace --all-features [--release] [--run-ignored all]`, then `cargo test --doc --workspace --all-features` |
| `doc` | `cargo doc --workspace --no-deps --all-features` with env `RUSTDOCFLAGS=-D warnings` |
| `deny` | `cargo deny check` |
| `ci --quick` | fmt-check, clippy, test |

- `require_tool` hints: e.g. `cargo-nextest missing: run 'cargo install --locked cargo-nextest'`.

- [ ] **Step 6: Run tests and checks**

Run: `cargo test -p xtask && cargo xtask fmt --check && cargo xtask clippy && cargo xtask deny`
Expected: all succeed. `Cargo.lock` is regenerated.

- [ ] **Step 7: Commit**

```bash
git add -A libs/placeholder Cargo.lock Cargo.toml rust-toolchain.toml rustfmt.toml clippy.toml deny.toml unsafe-allowlist.toml concurrency-allowlist.toml LICENSE LICENSE-MIT LICENSE-APACHE .cargo xtask
git commit -m "chore: set up workspace, toolchain, lint policy, licenses, and xtask"
```

---

### Task 2: xtask policy and platform checks

**Files:**
- Create: `xtask/src/check_unsafe.rs`, `xtask/src/check_statics.rs`
- Modify: `xtask/src/main.rs`, `xtask/Cargo.toml`

**Interfaces:**
- Produces:
  - `fn crate_root_forbids_unsafe(src: &str) -> bool`;
  - `fn parse_allowlist(toml: &str) -> Vec<String>`;
  - `struct Finding { path: PathBuf, line: usize, what: String, allowed_by_comment: bool }`;
  - `fn scan_statics(path: &Path, src: &str) -> Vec<Finding>`.
- Produces: commands `check-unsafe`, `check-statics`, `no-std`, `wasm`, `bench-build`, `bench [--save <name>] [--compare <a> <b>]`, and full `ci`.

- [ ] **Step 1: Write the failing tests**

```rust
// check_unsafe.rs tests
#[test] fn detects_forbid_attribute() {
    assert!(crate_root_forbids_unsafe("//! docs\n#![forbid(unsafe_code)]\n"));
    assert!(!crate_root_forbids_unsafe("#![deny(unsafe_code)]\n"));
    assert!(!crate_root_forbids_unsafe("// #![forbid(unsafe_code)]\n"));
}
#[test] fn parses_allowlist_names() {
    let toml = "# c\n[[crate]]\nname = \"engine_ffi\"\nadr = \"docs/adr/0010-x.md\"\n";
    assert_eq!(parse_allowlist(toml), vec!["engine_ffi".to_string()]);
    assert!(parse_allowlist("# empty\n").is_empty());
}
// check_statics.rs tests
#[test] fn flags_interior_mutable_statics_and_thread_local() {
    let src = "static A: std::sync::Mutex<u8> = std::sync::Mutex::new(0);\n\
               static B: AtomicU32 = AtomicU32::new(0);\n\
               static C: [u8; 2] = [0, 1];\n\
               thread_local! { static D: u8 = 0; }\n";
    let f = scan_statics(Path::new("x.rs"), src);
    assert_eq!(f.iter().map(|f| f.line).collect::<Vec<_>>(), vec![1, 2, 4]);
}
#[test] fn honors_allow_comment() {
    let src = "// engine:allow-global-state: diagnostics facade\nstatic A: OnceLock<u8> = OnceLock::new();\n";
    assert!(scan_statics(Path::new("x.rs"), src)[0].allowed_by_comment);
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p xtask`
Expected: FAIL (functions not defined).

- [ ] **Step 3: Implement**

- **Dependencies:** `syn = { version = "2", features = ["full", "visit"] }` and `proc-macro2 = { version = "1", features = ["span-locations"] }`. syn drops comments, so the line numbers come from proc-macro2 spans; the allow comment is then found by checking the source text of the preceding line.
- **`crate_root_forbids_unsafe`:** true iff a line, after trimming, equals `#![forbid(unsafe_code)]`.
- **`parse_allowlist`:** collect the values of `name = "…"` lines. This is a hand parse with no TOML dependency.
- **`scan_statics`:** walk with `syn::visit`:
  - flag an `ItemStatic` whose type tokens contain any of `Cell RefCell UnsafeCell Mutex RwLock OnceCell OnceLock LazyLock LazyCell`, or an identifier starting with `Atomic`;
  - flag any `ItemMacro` / `Macro` whose path ends in `thread_local`.
- **`check-unsafe`:** for each workspace crate (`libs/*`, `examples/*`, `benches`, `xtask`), check `src/lib.rs` and `src/main.rs` where they exist. Fail with `"<path>: missing #![forbid(unsafe_code)] (add it, or list the crate in unsafe-allowlist.toml with an ADR)"`.
- **`check-statics`:** scan `libs/*/src/**/*.rs`. Fail on unallowed findings, and print the allowed ones.
- **`no-std`:** `cargo build -p engine_handle -p engine_random -p engine_time --no-default-features --target thumbv7em-none-eabihf`. Skip with a notice for crates that don't exist yet; this skip is removed in Task 19.
- **`wasm`:**
  - `cargo build -p engine --target wasm32-unknown-unknown`;
  - then `cargo test --target wasm32-wasip1 --lib --tests -p engine_handle -p engine_random -p engine_time -p engine_math` with env `CARGO_TARGET_WASM32_WASIP1_RUNNER=wasmtime`;
  - skip crates that don't exist yet (same rule as `no-std`).
- **`bench-build`:** `cargo bench --workspace --no-run`.
- **`bench`:** with `--save`, runs `cargo bench --workspace -- --save-baseline <name>`; with `--compare a b`, runs `critcmp a b` (`require_tool("critcmp", …)`). Extend the `parses_known_commands` test with `parse(&["bench", "--compare", "a", "b"]) == Ok(Command::Bench { save: None, compare: Some(("a".into(), "b".into())) })`.
- **`ci`:** fmt-check, clippy, test, doc, no-std, wasm, check-unsafe, check-statics, deny, bench-build, in that order, stopping at the first failure.

- [ ] **Step 4: Verify**

Run: `cargo test -p xtask && cargo xtask check-unsafe && cargo xtask check-statics && cargo xtask ci`
Expected: PASS. Crates that don't exist yet are reported as skipped.

- [ ] **Step 5: Commit**

```bash
git add xtask Cargo.lock
git commit -m "feat(xtask): add unsafe/global-state checks, no_std and wasm checks, full ci"
```

---

### Task 3: CI workflows, dependabot, devcontainer tooling

**Files:**
- Create: `.github/workflows/ci.yml`, `.github/workflows/nightly.yml`, `.devcontainer/install-tools.sh`
- Modify: `.github/dependabot.yml`, `.devcontainer/devcontainer.json`

**Interfaces:** Consumes the `cargo xtask` commands from Tasks 1–2.

- [ ] **Step 1: Write `ci.yml`**

- **Triggers:** `push` (all branches) and `pull_request` (branches: `main`).
- **Concurrency:** `concurrency: { group: ci-${{ github.ref }}, cancel-in-progress: true }`.
- **Environment:** `RUSTFLAGS: -D warnings`, `CARGO_TERM_COLOR: always`.
- **Every job** runs:
  - `actions/checkout@v4`;
  - `rustup show` (installs the toolchain from `rust-toolchain.toml`);
  - `Swatinem/rust-cache@v2`;
  - `taiki-e/install-action@v2` (`tool: cargo-nextest,cargo-deny,wasmtime` as needed);
  - one `cargo xtask <cmd>`.
- **Ubuntu jobs on every event:** `fmt` (`fmt --check`), `clippy`, `test` (plus a second step, `test --release --include-ignored`), `doc`, `no-std`, `wasm`, `check-unsafe`, `check-statics`, `deny`, `bench-build`.
- **`clippy` and `test` legs on `macos-latest` and `windows-latest`:** gated by `if: github.event_name == 'pull_request'`.
- Add the comment `# When the repo is public: remove this if (spec §4.7).`

- [ ] **Step 2: Write `nightly.yml`**

`schedule: cron "0 6 * * *"` plus `workflow_dispatch`. Two jobs:
1. ubuntu: `rustup toolchain install stable && rustup override set stable`, then `cargo xtask ci`, with `continue-on-error: true`. This is an early warning for toolchain bumps.
2. The macOS/Windows clippy+test legs on the pinned toolchain.

- [ ] **Step 3: Update dependabot and devcontainer**

- `dependabot.yml`: add a `cargo` ecosystem (weekly, `groups: { all: { patterns: ["*"] } }`) and a `github-actions` ecosystem (weekly), keeping the existing devcontainers entry.
- `install-tools.sh`: `set -euo pipefail; cargo install --locked cargo-nextest cargo-deny critcmp wasmtime-cli`.
- `devcontainer.json`: append `&& bash .devcontainer/install-tools.sh` to `postCreateCommand`.

- [ ] **Step 4: Verify**

Run: `cargo xtask ci` locally (unchanged from Task 2).
Expected: PASS. The workflows themselves are validated by the Phase 1 push (Task 4, Step 4). If `actionlint` is installed, also run `actionlint` and expect no findings.

- [ ] **Step 5: Commit**

```bash
git add .github .devcontainer
git commit -m "ci: add tiered CI, nightly workflow, dependabot ecosystems, devcontainer tools"
```

---

### Task 4: README and ADRs 0001–0004, 0007–0009

**Files:** Create: `README.md` and `docs/adr/0001-unsafe-policy.md`, `0002-originality-and-dependency-licensing.md`, `0003-same-build-determinism.md`, `0004-crate-splitting-rule.md`, `0007-host-owns-the-loop.md`, `0008-no-global-state.md`, `0009-tooling-uses-public-apis.md`

- [ ] **Step 1: Write the ADRs**

Each ADR has `Status: Accepted (2026-11-12)`, plus Context, Decision, and Consequences sections, at roughly half a page to one page. Source sections:

| ADR | Spec source |
|---|---|
| 0001 | §4.3 |
| 0002 | §4.5 |
| 0003 | §4.9, including thread-count independence, handle allocation order, the math funnel, the upgrade path, and the snapshot-able state principle |
| 0004 | §3.1, including the crate justification table |
| 0007 | Loop ownership row of §1 |
| 0008 | §4.10 |
| 0009 | §4.4, tooling bullet |

Each ADR cites the sources listed for it in spec §12.

- [ ] **Step 2: Write `README.md`**

Sections:
- purpose;
- status (experimental, 0.x);
- crate map (with the justification table);
- quick start (`cargo xtask ci`, `cargo xtask ci --quick`);
- "Using the engine in your project": the dev-profile snippet (`[profile.dev] opt-level = 1`, `[profile.dev.package."*"] opt-level = 3`) and "glam's `fast-math`/`scalar-math` features are unsupported";
- "Platform support", which holds the WASM test notes from spec §4.7;
- License: dual MIT/Apache with the standard contribution clause ("Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.").

- [ ] **Step 3: Verify and commit**

Run: `cargo xtask ci --quick`.
Expected: PASS (docs-only change).

```bash
git add README.md docs/adr
git commit -m "docs: add README and ADRs 0001-0004, 0007-0009"
```

- [ ] **Step 4: Phase 1 push**

Ask the user for permission before the first push. Then run `git push -u origin m0-m1-foundations`.
Expected: the CI ubuntu jobs go green (check with `gh run watch`).

---

# Phase 2: `engine_handle`

### Task 5: `Handle<T>`

**Files:**
- Create: `libs/engine_handle/Cargo.toml`, `src/lib.rs`, `src/handle.rs`, `tests/auto_traits.rs`
- Modify: root `Cargo.toml` (`[workspace.dependencies] engine_handle = { path = "libs/engine_handle", version = "0.1.0" }`)

**Interfaces:**
- Produces: `Handle<T>` exactly as in spec §5.1.
- Produces, crate-internal: `pub(crate) const fn Handle::new(index: u32, generation: NonZeroU32) -> Self`.

- [ ] **Step 1: Write the failing tests**

```rust
// handle.rs #[cfg(test)]
#[test] fn bits_layout_is_index_high_generation_low() {
    let h = Handle::<u8>::new(3, NonZeroU32::new(2).unwrap());
    assert_eq!(h.to_bits(), (3u64 << 32) | 2);
    assert_eq!(Handle::<u8>::from_bits(h.to_bits()), Some(h));
    assert_eq!(Handle::<u8>::from_bits(0), None);
    assert_eq!(Handle::<u8>::from_bits(7u64 << 32), None); // generation half zero
}
#[test] fn ord_matches_bits_order() { /* handles (1,9),(2,1),(2,3): sorted order == sorted by to_bits */ }
#[test] fn debug_uses_short_type_name() {
    struct Particle;
    let h = Handle::<Particle>::new(3, NonZeroU32::new(2).unwrap());
    assert_eq!(format!("{h:?}"), "Handle<Particle>(3v2)");
}
// tests/auto_traits.rs
#[test] fn handle_is_send_sync_copy_even_for_non_send_t() {
    fn assert_traits<T: Send + Sync + Copy + Eq + Ord + core::hash::Hash>() {}
    assert_traits::<Handle<std::rc::Rc<u8>>>();
    assert_eq!(size_of::<Handle<u8>>(), 8);
    assert_eq!(size_of::<Option<Handle<u8>>>(), 8);
}
```

Plus a `compile_fail` doctest on `Handle`: `fn takes_b(_: Handle<B>) {} takes_b(Handle::<A>::from_bits(1).unwrap());`.

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p engine_handle`
Expected: FAIL (crate/type missing).

- [ ] **Step 3: Implement `handle.rs`**

- Manual impls of `Clone`, `Copy`, `PartialEq`, `Eq`, `PartialOrd`, `Ord` (by `(index, generation)`), `Hash`, and `Debug`, with no bounds on `T`.
- **Short type name:** `core::any::type_name::<T>()`, with every `seg::` path prefix removed while writing. For example, `fixed_step_sim::Particle` → `Particle` and `alloc::vec::Vec<a::B>` → `Vec<B>`. Write directly to the formatter, with no allocation.
- `#[inline]` on `index`, `generation`, `to_bits`, `from_bits`.
- **Crate root:** `#![no_std]`, `extern crate alloc;`, `#![forbid(unsafe_code)]`. The crate docs' first paragraph states its justification (spec §3.1 table) and the documented cross-pool limitation.
- **Cargo.toml:** no dependencies; dev-dependency `proptest = { workspace = true }`.

- [ ] **Step 4: Verify**

Run: `cargo test -p engine_handle && cargo xtask clippy`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add libs/engine_handle Cargo.toml Cargo.lock
git commit -m "feat(handle): add typed generational Handle<T> with stable bit encoding"
```

---

### Task 6: `HandlePool<T>` core operations

**Files:**
- Create: `libs/engine_handle/src/pool.rs`, `src/error.rs`, `tests/panics.rs`
- Modify: `src/lib.rs`

**Interfaces:**
- Consumes: `Handle::new`.
- Produces, from spec §5.1:
  - `HandlePool<T>`: `new`, `with_capacity`, `insert`, `try_insert`, `get`, `get_mut`, `remove`, `contains`, `len`, `is_empty`, `capacity`, `clear`, plus `Index`/`IndexMut`, `Default`, and `Clone` (when `T: Clone`);
  - `CapacityError<T>`.
- Produces, test-only: `pub fn assert_invariants(&self)`, behind `#[cfg(any(test, feature = "test-invariants"))]`. Add feature `test-invariants = []` so the integration tests in Task 7 can use it.
- Produces, crate-internal: `pub(crate) fn with_slot_limit(limit: u32) -> Self` (normal limit `u32::MAX`), and `#[cfg(test)] fn set_generation_for_test(index: u32, generation: u32)`.

- [ ] **Step 1: Write the failing tests** (in `pool.rs` `#[cfg(test)]`)

```rust
#[test] fn insert_get_remove_roundtrip() { /* insert 5, get == Some(&5), remove == Some(5), get == None, len 0 */ }
#[test] fn removed_handle_is_stale_and_slot_reused_lifo() {
    let mut p = HandlePool::new();
    let a = p.insert('a'); let b = p.insert('b');
    p.remove(a); p.remove(b);
    let c = p.insert('c');               // LIFO: reuses b's slot
    assert_eq!(c.index(), b.index());
    assert_eq!(c.generation().get(), b.generation().get() + 1);
    assert!(!p.contains(b));
}
#[test] fn generation_exhaustion_retires_slot() {
    let mut p = HandlePool::new(); let h = p.insert(1);
    p.set_generation_for_test(h.index(), u32::MAX);
    let h = Handle::new(h.index(), NonZeroU32::new(u32::MAX).unwrap());
    assert_eq!(p.remove(h), Some(1));
    let n = p.insert(2);
    assert_ne!(n.index(), h.index());    // retired, never reused
    p.remove(n); let m = p.insert(3);
    assert_ne!(m.index(), h.index());    // still never reused after more churn
    p.assert_invariants();
}
#[test] fn clear_staleness_and_ascending_reuse() { /* insert 3, clear, all stale; next inserts get indices 0,1,2 in order */ }
#[test] fn try_insert_returns_value_when_full() {
    let mut p = HandlePool::with_slot_limit(1);
    let _ = p.insert(1);
    assert_eq!(p.try_insert(2).unwrap_err().into_inner(), 2);
}
#[test] fn zero_sized_values_work() { /* HandlePool<()>: insert 3, remove middle, len 2, contains semantics hold, assert_invariants() */ }
```

In `tests/panics.rs`, which uses a panic hook to capture `Location`:
```rust
#[test] fn index_on_stale_handle_panics_at_caller() {
    // install a hook recording location; catch_unwind(|| pool[stale]);
    // assert location.file() ends with "tests/panics.rs"
    // and the message contains "stale handle" and "use get()"
}
#[test] fn insert_past_limit_panics_with_hint() { /* message contains "try_insert" */ }
```
(`with_slot_limit` is `pub(crate)`, so cover the insert-limit case with a unit test in `pool.rs` using `#[should_panic(expected = "try_insert")]`; the `tests/panics.rs` file covers `Index`.)

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p engine_handle`
Expected: FAIL.

- [ ] **Step 3: Implement**

- **Slot layout:** `struct Slot<T> { generation: u32, state: SlotState<T> }` with `enum SlotState<T> { Occupied(T), Vacant { next_free: Option<u32> }, Retired }`.
  - For an occupied slot, `generation` is the live generation. For a vacant slot, it is the generation the next occupant receives.
  - New slots start at 1. `remove` bumps the generation, or retires the slot if it is at `u32::MAX`.
- **Free list:** a LIFO stack through `next_free`.
- **`clear`:** bump or retire every occupied slot, then rebuild the free list so that pops yield ascending indices.
- **`capacity`:** `slots.capacity() - retired_count`.
- **`CapacityError<T>`:**
  - `Display`: `"HandlePool is full: all {limit} slot indices are in use or retired; remove values or use another pool"`;
  - `core::error::Error` (when `T: Debug`);
  - `#[derive(Debug, Clone, PartialEq, Eq)]`.
- **Panicking APIs:** `insert`, `Index`, and `IndexMut` are `#[track_caller]` with `# Panics` docs.
- **`assert_invariants`:**
  - `len` equals the occupied count;
  - the free list visits exactly the vacant slots, with no cycles;
  - no occupied slot has generation 0.
- **Doc comment:** the slot overhead (spec §4.11), e.g. "each slot stores a `u32` generation plus the enum discriminant alongside `T`".

- [ ] **Step 4: Verify**

Run: `cargo test -p engine_handle --all-features`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add libs/engine_handle
git commit -m "feat(handle): add HandlePool core with LIFO reuse, retirement, fallible insert"
```

---

### Task 7: `HandlePool` iteration, bulk operations, model test

**Files:**
- Create: `libs/engine_handle/src/pool/iter.rs`, `tests/model.rs`
- Modify: `src/pool.rs`

**Interfaces:**
- Produces, from spec §5.1:
  - `iter`, `iter_mut`, and `drain`, with named types `Iter`, `IterMut`, `Drain`, and `IntoIter`;
  - `IntoIterator` for `&`, `&mut`, and owned pools;
  - `retain`, `get_disjoint_mut`, and `Debug` as a map.
- All iterators implement `ExactSizeIterator` + `FusedIterator`.

- [ ] **Step 1: Write the failing tests**

```rust
// pool.rs tests
#[test] fn iteration_is_ascending_and_exact_size() { /* insert 0..5, remove 1 and 3; iter() yields indices [0,2,4]; len() == 3 at start and decrements */ }
#[test] fn drain_dropped_early_equals_clear() {
    // pool A: insert 4, take 1 from drain then drop it
    // pool B: same inserts, clear()
    // both pools then insert identically and must produce equal handles; A is empty after the drop
}
#[test] fn get_disjoint_mut_rules() {
    // [a, b] distinct live -> Some; [a, a] -> None; [a, stale] -> None; mutation through both is visible
}
#[test] fn debug_prints_as_map() {
    // HandlePool<u8> with values 7 at slot 0 and 9 at slot 2 (slot 1 removed)
    // format!("{p:?}") == "{Handle<u8>(0v1): 7, Handle<u8>(2v1): 9}"
}
#[test] fn retain_panic_leaves_pool_consistent() {
    // insert 0..10; catch_unwind(AssertUnwindSafe(|| p.retain(|_, v| { if *v == 5 { panic!() }; *v % 2 == 0 })))
    // then p.assert_invariants() passes and every remaining handle resolves
}
```

`tests/model.rs`: proptest over `Vec<Op>`, where `Op = Insert(u16) | Remove(usize) | Get(usize) | GetDisjoint(usize, usize) | Retain(u16 /*keep if v % k != 0*/) | Clear | Iterate`. Handle indices point into a vec of all handles ever issued, so stale handles get exercised. The reference model is a `BTreeMap<u64 /*to_bits*/, u16>`. After each op:
- `assert_invariants()`;
- `len` matches the model;
- `iter()` matches the model's values in ascending index order;
- every issued handle resolves iff the model contains it.

Additionally, replaying the same op sequence twice yields identical handle bits. On `target_os = "wasi"`, use `ProptestConfig { failure_persistence: None, ..Default::default() }`, because there's no writable directory there.

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p engine_handle --all-features`
Expected: FAIL.

- [ ] **Step 3: Implement**

- **Iterators:** wrap `slice::Iter`/`IterMut` over the slots, skip non-occupied slots, and track a `remaining` count for `ExactSizeIterator`.
- **`Drain`:** holds `&mut HandlePool` and a cursor. `Drop` finishes the clear (removing the remaining items) and then rebuilds the free list exactly as `clear` does.
- **`retain`:** visit in index order and remove where `keep` returns false, keeping `len` and the free list consistent after each removal, so that a panic midway leaves a valid pool.
- **`get_disjoint_mut`:** validate liveness and distinct indices first, then call `self.slots.get_disjoint_mut(indices)` and map each slot to `&mut T`. Use `unreachable!` for non-occupied slots; the validation above guarantees they can't appear.
- **`Debug`:** `f.debug_map().entries(self.iter())`.

- [ ] **Step 4: Verify**

Run: `cargo test -p engine_handle --all-features`
Expected: PASS (proptest default of 256 cases).

- [ ] **Step 5: Commit**

```bash
git add libs/engine_handle
git commit -m "feat(handle): add HandlePool iteration, drain, retain, get_disjoint_mut, model tests"
```

---

### Task 8: `HandleAllocator<T>`

**Files:**
- Create: `libs/engine_handle/src/allocator.rs`
- Modify: `src/lib.rs`, `tests/model.rs`

**Interfaces:**
- Produces, from spec §5.1: `HandleAllocator<T>` with `new`, `with_capacity`, `allocate`, `try_allocate`, `free`, `is_alive`, `len`, `is_empty`, `clear`, `iter` (`AllocIter`), `retain`, and `assert_invariants`.
- Implementation: `struct HandleAllocator<T> { pool: HandlePool<()>, _marker: PhantomData<fn() -> T> }`. Handles are re-typed via `Handle::new(h.index(), h.generation())`. This reuses the pool, so both have identical semantics by construction.

- [ ] **Step 1: Write failing tests**

- Unit tests: `free` returns `false` for a stale handle; `allocate` after `free` reuses the slot in LIFO order with generation + 1; `try_allocate` returns `None` at the slot limit (`with_slot_limit` is reachable through `pub(crate)`).
- Model test: extend `tests/model.rs` with an allocator model: `BTreeSet<u64>`, ops `Allocate | Free(usize) | IsAlive(usize) | Retain(u8) | Clear`.

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p engine_handle --all-features`
Expected: FAIL.

- [ ] **Step 3: Implement as described in Interfaces**

`Debug` prints `HandleAllocator<T> [Handle<T>(0v1), ...]` via `debug_list` on the live handles, with the type name prefix written as in Task 5.

- [ ] **Step 4: Verify**

Run: `cargo test -p engine_handle --all-features && cargo xtask no-std`
Expected: PASS; `engine_handle` builds for thumbv7em.

- [ ] **Step 5: Commit**

```bash
git add libs/engine_handle
git commit -m "feat(handle): add HandleAllocator sharing HandlePool semantics"
```

---

### Task 9: Handle comparison benchmarks

**Files:**
- Create: `benches/Cargo.toml` (package `engine_benches`), `benches/src/lib.rs` (crate docs only, `#![forbid(unsafe_code)]`), `benches/benches/handles.rs`
- Modify: root `Cargo.toml` (add `"benches"` to `members`)

**Interfaces:** Consumes `HandlePool`, `HandleAllocator`.

- [ ] **Step 1: Set up the crate and write the benchmarks**

- **Dev-dependencies** (latest versions via `cargo add`): `criterion`, `slotmap`, `thunderdome`, and `engine_handle` from the workspace.
- `[[bench]] name = "handles" harness = false`.
- The crate root has `#![allow(clippy::disallowed_types, clippy::disallowed_methods, clippy::unwrap_used)]` with the comment `// benchmark harness, not simulation code`.
- **Groups**, each parameterized over N ∈ {1_000, 100_000, 1_000_000} and both `T = u64` and `T = [u64; 8]` (64 bytes):
  - `insert`;
  - `get_sequential`;
  - `get_random` (handle order shuffled once with a fixed seed);
  - `remove_all`;
  - `iterate`;
  - `churn_then_get` (remove a seeded-random 50%, reinsert, then get all);
  - for `HandleAllocator`: `allocate`, `free_realloc_churn`, `is_alive`.
- Each group benchmarks `engine_handle`, `slotmap::SlotMap`, and `thunderdome::Arena` side by side. `slotmap` with `()` values is the allocator comparison.
- Use a seeded `engine_random::Prng` once it exists. Until then, shuffle with a small inline xorshift64 in the bench file, documented as bench-only.

- [ ] **Step 2: Verify**

Run: `cargo xtask bench-build && cargo bench -p engine_benches --bench handles -- --quick`
Expected: compiles and runs; criterion prints results.

- [ ] **Step 3: Commit, then push the phase**

```bash
git add benches Cargo.toml Cargo.lock
git commit -m "perf(bench): add handle pool/allocator comparisons vs slotmap and thunderdome"
git push
```
Expected: the CI ubuntu jobs go green.

---

# Phase 3: `engine_random`

### Task 10: `Prng` core: SplitMix64, xoshiro256++, state, streams

**Files:**
- Create: `libs/engine_random/Cargo.toml`, `src/lib.rs`, `src/splitmix.rs`, `src/prng.rs`, `tests/oracle.rs`, `tests/golden.rs`
- Modify: root `Cargo.toml` (`[workspace.dependencies] engine_random = { path = "libs/engine_random", version = "0.1.0" }`)

**Interfaces:**
- Produces: `Prng` with `from_seed`, `next_u32`, `next_u64`, `next_f32`, `next_f64`, `next_bool`, `stream`, `to_state`, and `from_state` (spec §5.2).
- `#[derive(Clone, Debug, PartialEq, Eq, Hash)] pub struct Prng { state: [u64; 4] }`. The field is named `state` so `Debug` reads `Prng { state: [..] }`.
- Produces, crate-internal: `pub(crate) const fn mix64(z: u64) -> u64` and `pub(crate) struct SplitMix64 { state: u64 }` with `next()`.

These are the published algorithms, with the constants that define them:
```text
SplitMix64.next: state = state.wrapping_add(0x9E37_79B9_7F4A_7C15); return mix64(state)
mix64(z): z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9)
          z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB)
          return z ^ (z >> 31)
xoshiro256++ next: result = (s0.wrapping_add(s3)).rotate_left(23).wrapping_add(s0)
          t = s1 << 17; s2 ^= s0; s3 ^= s1; s1 ^= s2; s0 ^= s3; s2 ^= t; s3 = s3.rotate_left(45)
from_seed(seed): sm = SplitMix64{state: seed}; state = [sm.next(), sm.next(), sm.next(), sm.next()]
stream(id): Prng::from_seed(mix64(s0 ^ s1.rotate_left(17) ^ s2.rotate_left(31) ^ s3.rotate_left(47) ^ mix64(id)))
next_u32 = (next_u64() >> 32) as u32; next_f32 = (next_u64() >> 40) as f32 * 2^-24
next_f64 = (next_u64() >> 11) as f64 * 2^-53; next_bool = next_u64() >> 63 == 1
```

- [ ] **Step 1: Write the failing tests**

`tests/oracle.rs` uses the dev-dependency `rand_xoshiro` (with `rand_core`) as an independent oracle. It is a dependency, not copied code.
```rust
#[test] fn xoshiro_matches_independent_implementation() {
    for seed in [0u64, 1, 42, u64::MAX, 0xDEAD_BEEF] {
        let ours = Prng::from_seed(seed);
        let bytes: Vec<u8> = ours.to_state().iter().flat_map(|w| w.to_le_bytes()).collect();
        let mut theirs = rand_xoshiro::Xoshiro256PlusPlus::from_seed(bytes.try_into().unwrap());
        let mut ours = ours;
        for _ in 0..1000 { assert_eq!(ours.next_u64(), theirs.next_u64()); }
    }
}
#[test] fn splitmix_seeding_matches_independent_implementation() {
    // rand_xoshiro::SplitMix64::from_seed(seed.to_le_bytes()): its first four next_u64() == Prng::from_seed(seed).to_state()
}
```

Unit tests in `prng.rs`:
- `from_state([0; 4]) == None`;
- `from_state(p.to_state())` continues identically for 100 draws;
- `stream` does not advance `self`;
- `stream(1) != stream(2)`;
- `next_f32`/`next_f64` lie in [0, 1);
- `format!("{:?}", Prng::from_state([1,2,3,4]).unwrap()) == "Prng { state: [1, 2, 3, 4] }"`.

`tests/golden.rs`: for seeds `{0, 1, 42, u64::MAX}`, the first 16 outputs of `next_u64`, `next_u32`, `next_f32` (as `to_bits`), `next_f64` (as `to_bits`), `next_bool`, and `stream(7).next_u64()`. Hardcode them as array literals.

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p engine_random`
Expected: FAIL.

- [ ] **Step 3: Implement** the algorithms exactly as above.

- `#![no_std]`, no dependencies, `#[inline]` on all `next_*`.
- The crate docs cover: the justification (spec §3.1), "not cryptographically secure", the stream stability guarantee, the deliberate non-impls (no `Copy`, no `Default`, with reasons), and citations: Blackman & Vigna, "Scrambled Linear Pseudorandom Number Generators" (2018); Steele, Lea & Flood, "Fast Splittable Pseudorandom Number Generators" (2014).

- [ ] **Step 4: Generate and freeze the golden values**

After `oracle.rs` passes:
- Add a test, `#[ignore] fn print_goldens()`, that prints the arrays in Rust literal syntax.
- Run `cargo test -p engine_random --test golden print_goldens -- --ignored --nocapture`.
- Paste the output into the golden constants, then delete the printer.

Run: `cargo test -p engine_random && cargo xtask wasm`
Expected: PASS, including the goldens on `wasm32-wasip1`.

- [ ] **Step 5: Commit**

```bash
git add libs/engine_random Cargo.toml Cargo.lock
git commit -m "feat(random): add Prng (xoshiro256++/SplitMix64) with state, streams, frozen goldens"
```

---

### Task 11: Sampling: ranges, `chance`, `shuffle`, `choose`, `fill_bytes`

**Files:**
- Create: `libs/engine_random/src/sample.rs`, `tests/edge_cases.rs`, `tests/statistics.rs`, `tests/panics.rs`
- Modify: `src/prng.rs`, `tests/golden.rs`

**Interfaces:**
- Produces: `Prng::range`, `try_range`, `chance`, `shuffle`, `choose`, and `fill_bytes`; `pub trait SampleRange: sealed::Sealed { type Output; }` implemented for `Range`/`RangeInclusive` over `u8 u16 u32 u64 usize i8 i16 i32 i64 isize f32 f64`.
- The trait's sampling method is private: `fn sample(self, prng: &mut Prng) -> Option<Self::Output>`, sealed via `mod sealed { pub trait Sealed {} }`.
- Produces, crate-internal: `pub(crate) fn bounded_u64(&mut self, n: u64) -> u64`.

Every algorithm follows spec §5.2's "Normative algorithms" exactly. Implement the integer impls with one `macro_rules!` covering the unsigned and signed widening rules.

- [ ] **Step 1: Write the failing tests**

`tests/edge_cases.rs`:
```rust
#[test] fn full_range_signed_and_unsigned() {
    let mut p = Prng::from_seed(9);
    let v: Vec<i64> = (0..64).map(|_| p.range(i64::MIN..=i64::MAX)).collect();
    assert!(v.iter().any(|x| *x < 0) && v.iter().any(|x| *x > 0));
    let _ = p.range(0..=u64::MAX); let _ = p.range(i8::MIN..=i8::MAX);
}
#[test] fn invalid_ranges_return_none_without_drawing() {
    let mut p = Prng::from_seed(1); let before = p.clone();
    assert_eq!(p.try_range(5..5), None);
    assert_eq!(p.try_range(1.0f32..1.0), None);
    assert_eq!(p.try_range(-f32::MAX..f32::MAX), None);   // hi - lo overflows
    assert_eq!(p.try_range(f64::NAN..1.0), None);
    assert_eq!(p, before);
}
#[test] fn inclusive_degenerate_and_bounds() {
    let mut p = Prng::from_seed(2);
    assert_eq!(p.range(3.5f32..=3.5), 3.5);
    assert!((0..10_000).all(|_| { let x = p.range(0.0f32..1.0); (0.0..1.0).contains(&x) }));
}
#[test] fn chance_edges() { /* chance(0.0)=false, chance(1.0)=true, chance(f64::NAN)=false, chance(-1.0)=false, chance(2.0)=true */ }
#[test] fn choose_and_shuffle_edges() { /* choose(&[]) None and draws nothing; shuffle of len 0/1 draws nothing */ }
#[test] fn fill_bytes_is_little_endian_prefix() {
    // for len in 0..=17: fill a buffer from clone A; from clone B compute words via next_u64().to_le_bytes()
    // and take the first len bytes; they are equal
}
```

`tests/golden.rs` additions:
- 16 `range` outputs each for `0u8..10`, `-5i16..=5`, `0usize..1000`, `isize::MIN..=isize::MAX` (as `i64`), `0.0f32..1.0` (bits), `-1.0f64..=1.0` (bits);
- `shuffle` of `[0..10]`;
- `choose` over a 7-element slice ×16;
- `fill_bytes` for 13 bytes.

Generate and freeze these exactly as in Task 10, Step 4. The wasm job then proves width independence.

`tests/panics.rs`: `range(5..5)` panics with location in `tests/panics.rs` and a message containing `"empty range 5..5"` and `"try_range"`.

A `compile_fail` doctest on `Prng::range`: `let mut p = Prng::from_seed(0); let _ = p.range(0u128..5);` (`u128` is not a `SampleRange`).

`tests/statistics.rs`, every test `#[ignore]` (run by `test --release --include-ignored`), all with fixed seeds:
- chi-square over 10 buckets for `range(0..10)` and `range(0..7)` with 1_000_000 samples, threshold 27.88 (df=9, p=0.001; df=6: 22.46);
- `next_f32`/`next_f64` over 16 buckets (df=15: 37.70);
- `next_bool` balance within 0.5 ± 0.002 over 1e6;
- `shuffle` of 4 elements: all 24 permutations with chi-square df=23 threshold 49.73 over 240_000 shuffles.

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p engine_random`
Expected: FAIL.

- [ ] **Step 3: Implement** per spec §5.2, with `#[track_caller]` on `range` and an actionable message naming the bounds.

- [ ] **Step 4: Verify**

Run: `cargo test -p engine_random && cargo nextest run -p engine_random --release --run-ignored all && cargo xtask wasm && cargo xtask no-std`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add libs/engine_random
git commit -m "feat(random): add normative range/chance/shuffle/choose/fill_bytes with goldens and stats"
```

---

### Task 12: PRNG benchmarks and `xtask prng-stats`

**Files:**
- Create: `benches/benches/prng.rs`, `xtask/src/prng_stats.rs`
- Modify: `benches/Cargo.toml`, `benches/benches/handles.rs` (replace the inline xorshift with `Prng`), `xtask/src/main.rs`, `xtask/Cargo.toml`

**Interfaces:** Consumes `Prng`. Produces the command `cargo xtask prng-stats [--seed N] [--bytes N]` (default: unlimited).

- [ ] **Step 1: Write the benchmarks**

- **Dev-dependencies:** `rand` (feature `small_rng`), `rand_xoshiro`, `fastrand`.
- **Groups**, each comparing `Prng` vs `Xoshiro256PlusPlus` vs `SmallRng` vs `fastrand::Rng`:
  - `next_u64` ×1M;
  - `range_u32_0_1000` ×1M;
  - `range_f32_0_1` ×1M;
  - `shuffle_10k`.
- `[[bench]] name = "prng" harness = false`.

- [ ] **Step 2: Implement `prng-stats`**

Write `Prng::from_seed(seed)` output with `fill_bytes` in 64 KiB chunks to locked stdout until `--bytes` is reached or stdout closes; treat a broken pipe as success. The command's help text documents piping into PractRand: `cargo xtask prng-stats | RNG_test stdin64`. A unit test: `--bytes 16` writes exactly 16 bytes, equal to `fill_bytes` of 16.

- [ ] **Step 3: Verify**

Run: `cargo test -p xtask && cargo bench -p engine_benches --bench prng -- --quick && cargo xtask prng-stats --bytes 16 | wc -c`
Expected: PASS; prints `16`.

- [ ] **Step 4: Commit, then push the phase**

```bash
git add benches xtask Cargo.lock
git commit -m "perf(bench): add PRNG comparisons; feat(xtask): add prng-stats"
git push
```

---

# Phase 4: `engine_time`

### Task 13: `Tick`, `Timestamp`, `TimeSource`, clocks

**Files:**
- Create: `libs/engine_time/Cargo.toml`, `src/lib.rs`, `src/tick.rs`, `src/timestamp.rs`, `src/clock.rs`, `src/error.rs`, `tests/panics.rs`
- Modify: root `Cargo.toml` (workspace dep `engine_time = { path = "libs/engine_time", version = "0.1.0", default-features = false }`)

**Interfaces:**
- Produces: `Tick` (`ZERO`, `new`, `next`, `checked_next`, `get`), `Timestamp`, `TimeSource` with the blanket impls for `&T`/`&mut T`, `ManualClock` (`new`, `starting_at`, `advance`, `set`, `try_set`), `SystemClock` (`new`, `Default`), and `NonMonotonicError { current: Timestamp, requested: Timestamp }`, all as in spec §5.3.
- `Cargo.toml`: `[features] default = ["std"]`, `std = []`.
- Crate root: `#![no_std]`, then `#[cfg(feature = "std")] extern crate std;`.
- Decision beyond the spec: **`ManualClock::advance` saturates** at the maximum `Duration` rather than panicking (documented).

- [ ] **Step 1: Write the failing tests**

- **Unit tests:**
  - `Tick::new(u64::MAX).checked_next() == None`;
  - `Tick::ZERO.next() == Tick::new(1)`;
  - `ManualClock::advance(Duration::MAX)` twice does not panic;
  - `try_set` earlier than now returns `Err(NonMonotonicError{..})` and leaves the clock unchanged;
  - `Timestamp::saturating_duration_since` of a later timestamp from an earlier one is zero;
  - `TimeSource` works through `&clock` and `&mut clock` (a generic `fn read(t: impl TimeSource)`).
- **SystemClock tests:** two consecutive `now()` calls are monotonic.
- **`tests/panics.rs`:** `ManualClock::set` backwards panics at the caller with a message containing `"try_set"`; `Tick::new(u64::MAX).next()` panics with a message containing `"checked_next"`.

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p engine_time`
Expected: FAIL.

- [ ] **Step 3: Implement**

- `SystemClock` uses the exact `cfg` from spec §5.3. Its `new`/`now` carry `#[expect(clippy::disallowed_methods, reason = "SystemClock is the sanctioned wall-clock TimeSource (ADR 0003)")]`.
- `NonMonotonicError` `Display`: `"ManualClock::try_set: requested {requested:?} is earlier than current {current:?}; clocks are monotonic"`.

- [ ] **Step 4: Verify**

Run: `cargo test -p engine_time && cargo xtask no-std && cargo xtask clippy`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add libs/engine_time Cargo.toml Cargo.lock
git commit -m "feat(time): add Tick, Timestamp, TimeSource, ManualClock, SystemClock"
```

---

### Task 14: `FixedTimestep`

**Files:**
- Create: `libs/engine_time/src/fixed.rs`, `tests/fixed_props.rs`, `benches/fixed_timestep.rs`
- Modify: `src/lib.rs`, `Cargo.toml` (dev-deps `proptest`, `criterion`; `[[bench]] name = "fixed_timestep" harness = false`)

**Interfaces:**
- Produces: `FixedTimestep` (`DEFAULT_MAX_STEPS_PER_UPDATE`, `new`, `try_new`, `from_hz`, `with_max_steps_per_update`, `accumulate`, `alpha`, `step`, `current_tick`, `dropped_time`, `to_state`, `from_state`), `FixedTimestepState`, and `Steps` (`Iterator<Item = Tick>`, `ExactSizeIterator`, `DoubleEndedIterator`, `FusedIterator`), all as in spec §5.3.

Accumulate semantics, decided here where the spec leaves them open:
```text
acc = acc.saturating_add(elapsed)
n   = min(acc_ns / step_ns, max_steps)            // u128 nanos
acc -= n * step
if n == max_steps && acc >= step:                 // capped: keep the fractional part only
    excess = acc - (acc % step); dropped = dropped.saturating_add(excess); acc -= excess
steps = Steps { next: tick, remaining: n }; tick = Tick(tick.0.saturating_add(n))
```

- [ ] **Step 1: Write the failing tests**

```rust
// unit
#[test] fn from_hz_60_is_exact() { assert_eq!(FixedTimestep::from_hz(NonZeroU32::new(60).unwrap()).step(), Duration::from_nanos(16_666_667)); }
#[test] fn alpha_stays_below_one() {
    for step in [Duration::from_nanos(16_666_667), Duration::from_millis(100)] {
        let mut t = FixedTimestep::new(step); let _ = t.accumulate(step - Duration::from_nanos(1));
        assert!(t.alpha() < 1.0);
    }
}
#[test] fn cap_drops_excess_keeps_fraction() {
    // step 10ms, cap 2: accumulate 35ms -> 2 steps, dropped 10ms, alpha ~0.5, current_tick == 2
}
#[test] fn accumulate_huge_elapsed_saturates_without_panic() {
    let mut t = FixedTimestep::new(Duration::from_millis(10));
    assert_eq!(t.accumulate(Duration::MAX).len(), 8);
    assert_eq!(t.accumulate(Duration::MAX).len(), 8);
    assert!(t.alpha() < 1.0);
}
#[test] fn state_roundtrip_and_validation() {
    // from_state(t.to_state()) behaves identically; from_state with step=0 -> None; accumulator >= step -> None
}
#[test] fn try_new_zero_is_none() { assert!(FixedTimestep::try_new(Duration::ZERO).is_none()); }
```

- `tests/fixed_props.rs` (proptest): for a step in 1..=50 ms and any split of a total T ≤ 2 s into ≤ 64 positive chunks, each chunk smaller than `cap * step` (so the cap is never hit), the concatenated ticks equal those from a single `accumulate(T)`, and the final `to_state()` values are equal.
- Panic test: `FixedTimestep::new(Duration::ZERO)` panics at the caller with a message containing `"try_new"`.
- Bench: `accumulate` of a 16 ms frame with a 1/60 s step, draining the `Steps`, ×1M.

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p engine_time`
Expected: FAIL.

- [ ] **Step 3: Implement** per spec §5.3 and the block above. `alpha` follows the spec formula, clamped to `1.0_f32.next_down()`.

- [ ] **Step 4: Verify**

Run: `cargo test -p engine_time && cargo bench -p engine_time -- --quick && cargo xtask wasm`
Expected: PASS.

- [ ] **Step 5: Commit, then push the phase**

```bash
git add libs/engine_time Cargo.lock
git commit -m "feat(time): add FixedTimestep with capped accumulation, from_hz, state snapshots"
git push
```

---

# Phase 5: `engine_math`

### Task 15: Crate setup, re-exports, scalar funnel, helpers, layout assertions

**Files:**
- Create: `libs/engine_math/Cargo.toml`, `src/lib.rs`, `src/scalar.rs`, `src/helpers.rs`, `src/layout.rs`
- Modify: root `Cargo.toml` (workspace dep `engine_math`)

**Interfaces:**
- Produces:
  - re-exports per spec §5.4, plus `pub use glam;`;
  - `scalar::{Float, sin, cos, tan, sin_cos, asin, acos, atan, atan2, sinh, cosh, tanh, asinh, acosh, atanh, exp, exp2, exp_m1, ln, ln_1p, log, log2, log10, powf, powi, cbrt, hypot}`, generic over the sealed `Float` (for `f32`, `f64`);
  - `lerp`, `inverse_lerp`, `remap`, `approx_eq`, re-exported at the crate root.
- `Cargo.toml`: `glam = { workspace = true, features = ["std"] }`.

- [ ] **Step 1: Write the failing tests**

```rust
#[test] fn scalar_matches_std_bitwise() {
    // tests may opt out: #[allow(clippy::disallowed_methods)]
    for x in [0.0f32, 0.5, 1.0, -2.25, 100.0] { assert_eq!(scalar::sin(x).to_bits(), x.sin().to_bits()); }
    assert_eq!(scalar::powi(2.0f64, 10), 1024.0);
    let (s, c) = scalar::sin_cos(1.0f32); assert_eq!((s, c), 1.0f32.sin_cos());
}
#[test] fn helpers() {
    assert_eq!(lerp(0.0, 10.0, 0.25), 2.5);
    assert_eq!(inverse_lerp(0.0, 10.0, 2.5), 0.25);
    assert!(inverse_lerp(1.0, 1.0, 1.0).is_nan());
    assert_eq!(remap(5.0, (0.0, 10.0), (100.0, 200.0)), 150.0);
    assert!(approx_eq(1.0, 1.0 + 1e-7, 1e-6) && !approx_eq(1.0, 1.1, 1e-6));
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p engine_math`
Expected: FAIL.

- [ ] **Step 3: Implement**

- **`scalar.rs`:** each function is `#[inline]` and delegates to the std inherent method, under one module-level `#![expect(clippy::disallowed_methods, reason = "engine_math::scalar is the sanctioned transcendental funnel (ADR 0003)")]`. The `Float` trait carries one private method per function.
- **`layout.rs`:** holds the `const` size/align assertions. Transforms come in Tasks 16–17; their assertions are added there.
- **Crate docs:** the justification, the feature-unification warning (glam `fast-math`/`scalar-math`), and a pointer to ADR 0005/0006.

- [ ] **Step 4: Verify**

Run: `cargo test -p engine_math && cargo xtask clippy`
Expected: PASS. Clippy must also flag a deliberate `1.0f32.sin()` added temporarily to `helpers.rs`; remove it after seeing the warning.

- [ ] **Step 5: Commit**

```bash
git add libs/engine_math Cargo.toml Cargo.lock
git commit -m "feat(math): add glam re-exports, transcendental funnel, scalar helpers"
```

---

### Task 16: `Transform` and `WorldTransform`

**Files:**
- Create: `libs/engine_math/src/transform.rs`, `src/world.rs`, `tests/transform_props.rs`, `tests/world_props.rs`
- Modify: `src/layout.rs`, `src/lib.rs`

**Interfaces:**
- Produces: `Transform` and `WorldTransform` exactly as in spec §5.4 (`#[repr(C)]`, the per-component `mul_transform` contract), including `Transform::to_world`, `Mul`, and `Default`.

- [ ] **Step 1: Write the failing tests**

`tests/transform_props.rs` (proptest; uniform scale in [0.1, 10], unit quaternions from normalized random axis+angle, translations in ±1e3):
- `(a * b).to_matrix() ≈ a.to_matrix() * b.to_matrix()` (eps 1e-3 relative);
- `t * t.inverse() ≈ IDENTITY`;
- `t.transform_point(p) ≈ t.to_matrix().transform_point3(p)`;
- `IDENTITY * t == t`.

`tests/world_props.rs`:
```rust
proptest! {
  #[test] fn relative_roundtrip_within_1mm(
      abs in prop::array::uniform3(-1e9f64..1e9), off in prop::array::uniform3(-1e4f64..1e4)) {
      let origin = DVec3::from(abs); let pos = origin + DVec3::from(off);
      let w = WorldTransform::from_translation(pos);
      let back = w.relative_to(origin).to_world(origin);
      prop_assert!((back.translation - pos).length() <= 1e-3);
  }
  #[test] fn transform_point_matches_daffine(/* random world + local point */) { /* within 1e-6 relative */ }
}
```

Layout tests in `layout.rs` `#[cfg(test)]` print the `size_of`/`align_of` of all four transform types (`--nocapture`), so CI logs show the values for each target.

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p engine_math`
Expected: FAIL.

- [ ] **Step 3: Implement** per spec §5.4. `relative_to` narrows via `(self.translation - origin).as_vec3()`.

- [ ] **Step 4: Pin the layouts**

Add `const` assertions in `layout.rs`:
- under `#[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]`: `Transform` size 48 / align 16, and `WorldTransform` size 64 / align 16;
- under `#[cfg(target_arch = "wasm32")]`: the values printed by Step 1's layout test when run via `cargo xtask wasm`.

Record all values in ADR 0006 (Task 18).

Run: `cargo test -p engine_math && cargo xtask wasm`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add libs/engine_math
git commit -m "feat(math): add Transform and large-world WorldTransform with pinned layouts"
```

---

### Task 17: `Transform2d`, `WorldTransform2d`, 2D/3D bridges

**Files:**
- Create: `libs/engine_math/src/transform2d.rs`, `tests/transform2d_props.rs`
- Modify: `src/transform.rs` (`Transform::to_2d`), `src/world.rs` (`WorldTransform::to_2d`), `src/layout.rs`, `src/lib.rs`

**Interfaces:** Produces `Transform2d` and `WorldTransform2d` exactly as in spec §5.4, plus `Transform::to_2d` and `WorldTransform::to_2d`. Twist extraction: take the quaternion's `(z, w)` components; if `hypot(z, w) < 1e-6` the twist is undefined, and rotation = 0.0 (documented); otherwise `rotation = 2 * atan2(z, w)`, wrapped into (−π, π]. Use `scalar::*` throughout.

- [ ] **Step 1: Write the failing tests**

- `tests/transform2d_props.rs`:
  - compose/inverse round trips (uniform scale);
  - `transform_point` agrees with `to_affine` and `to_mat3`;
  - `mul_transform` wraps the rotation into (−π, π] (e.g. 3.0 + 3.0 → 6.0 − 2π);
  - `t.to_3d(z).to_2d() ≈ t` (eps 1e-5);
  - a `WorldTransform2d` round trip within 1 mm, as in 3D.
- Unit tests:
```rust
#[test] fn to_2d_drops_tilt_keeps_twist() {
    let q = Quat::from_rotation_z(0.7) * Quat::from_rotation_x(0.3);
    assert!(approx_eq(Transform::from_rotation(q).to_2d().rotation, 0.7, 1e-5));
}
#[test] fn to_2d_of_half_turn_about_x_is_finite() {
    let t2 = Transform::from_rotation(Quat::from_rotation_x(core::f32::consts::PI)).to_2d();
    assert!(t2.rotation.is_finite()); assert_eq!(t2.rotation, 0.0);
}
```
- Layout: pin `Transform2d` 20/4 and `WorldTransform2d` 32/8 on 64-bit targets, plus the printed wasm32 values.

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p engine_math`
Expected: FAIL.

- [ ] **Step 3: Implement** per spec §5.4 and the twist rule above. Add the swing–twist explanation to the `to_2d` docs.

- [ ] **Step 4: Verify**

Run: `cargo test -p engine_math && cargo xtask wasm`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add libs/engine_math
git commit -m "feat(math): add Transform2d/WorldTransform2d and 2D/3D bridges"
```

---

### Task 18: `axis`, `convert`, ADRs 0005–0006

**Files:**
- Create: `libs/engine_math/src/axis.rs`, `src/convert.rs`, `tests/convert.rs`, `docs/adr/0005-coordinate-system-and-units.md`, `docs/adr/0006-large-world-precision.md`
- Modify: `src/lib.rs`

**Interfaces:**
- Produces: `axis` (`RIGHT LEFT UP DOWN VIEW_FORWARD ASSET_FRONT RIGHT_2D UP_2D`) and `convert` (`CoordinateSystem { x, y, z }`, `SignedAxis`, presets `ENGINE GLTF BLENDER UNITY UNREAL`, `new`, `is_right_handed`, `to_engine_matrix`, `vec3_to_engine`, `dvec3_to_engine`, `quat_to_engine`, `scale_to_engine`, `transform_to_engine` and their `*_from_engine` inverses), exactly as in spec §5.4.
- **Preset values must be verified first.** For each tool, read its documentation and its glTF importer/exporter's axis handling. Then apply the preset rule: the source's up maps to engine +Y, the source's asset-front maps to engine +Z, and on a handedness flip the asset-right axis is negated. Record the cited source and resulting mapping per preset in ADR 0005. **Omit any preset you can't verify**, and say so in ADR 0005.

- [ ] **Step 1: Write the failing tests** (`tests/convert.rs`)

```rust
#[test] fn engine_is_gltf_and_identity() {
    assert_eq!(CoordinateSystem::ENGINE, CoordinateSystem::GLTF);
    assert_eq!(vec3_to_engine(Vec3::new(1.0, 2.0, 3.0), CoordinateSystem::GLTF), Vec3::new(1.0, 2.0, 3.0));
    assert_eq!(vec3_to_engine(Vec3::Z, CoordinateSystem::GLTF), axis::ASSET_FRONT);
}
#[test] fn new_rejects_repeated_axis() { assert!(CoordinateSystem::new(SignedAxis::PosX, SignedAxis::NegX, SignedAxis::PosZ).is_none()); }
// For every verified preset P, with the documented source up U_P and asset-front F_P:
//   vec3_to_engine(U_P, P) == axis::UP and vec3_to_engine(F_P, P) == axis::ASSET_FRONT;
//   is_right_handed(P) matches the documentation.
// Property tests: *_from_engine(*_to_engine(x)) == x for vec3, quat, scale, transform;
//   a converted quaternion stays unit length and its rotation matrix has det ≈ +1.
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p engine_math --test convert`
Expected: FAIL.

- [ ] **Step 3: Implement**

- `to_engine_matrix` builds the signed permutation matrix: column `i` is the engine axis that source axis `i` maps to.
- Quaternion conversion: `Quat::from_mat3(&(M * Mat3::from_quat(q) * M.transpose()))`.
- `scale_to_engine` permutes the absolute components.

- [ ] **Step 4: Write ADRs 0005 and 0006**

- **ADR 0005:**
  - the convention;
  - "view forward vs asset front";
  - why glTF's convention;
  - the units and the screen-space (+Y down) convention;
  - the preset table with citations.
- **ADR 0006:**
  - the f64 world / f32 local split;
  - the precision guarantee;
  - the measured layout table per target, taken from Tasks 16–17;
  - the `scalar-math` hazard and upgrade path.

- [ ] **Step 5: Verify, commit, then push the phase**

Run: `cargo test -p engine_math && cargo xtask ci`
Expected: PASS.

```bash
git add libs/engine_math docs/adr/0005-coordinate-system-and-units.md docs/adr/0006-large-world-precision.md
git commit -m "feat(math): add axis constants and coordinate conversion; docs: ADRs 0005-0006"
git push
```

---

# Phase 6: Facade, sample, benchmarks, docs

### Task 19: `engine` facade and prelude

**Files:**
- Create: `libs/engine/Cargo.toml`, `libs/engine/src/lib.rs`
- Modify: `xtask/src/main.rs` (remove the "skip missing crates" notices from `no-std`/`wasm`, since all crates now exist)

**Interfaces:**
- Produces: the module re-exports `engine::{handle, random, time, math}` and the `engine::prelude` list exactly as in spec §5.5.
- `[features] default = ["std"]`, `std = ["engine_time/std"]`.
- Workspace deps for the leaf crates are `default-features = false`, so the facade enables `std` explicitly.

- [ ] **Step 1: Write the failing doctest**

The crate-root example is the "start here" page:
```rust
use engine::prelude::*;
let mut pool = HandlePool::new();
let h = pool.insert(Transform::IDENTITY);
let mut prng = Prng::from_seed(42);
let mut clock = ManualClock::new();
let mut step = FixedTimestep::from_hz(core::num::NonZeroU32::new(60).unwrap());
clock.advance(core::time::Duration::from_millis(20));
assert_eq!(step.accumulate(core::time::Duration::from_millis(20)).len(), 1);
assert!(pool.contains(h) && prng.range(0..10) < 10);
```

Also create `libs/engine/tests/auto_traits.rs`, which asserts `Send + Sync` (and `Debug`) for every public type: `Handle<Rc<u8>>`, `HandlePool<u8>`, `HandleAllocator<Rc<u8>>`, `CapacityError<u8>`, `Prng`, `Tick`, `Timestamp`, `FixedTimestep`, `FixedTimestepState`, `Steps`, `ManualClock`, `SystemClock`, `NonMonotonicError`, `Transform`, `WorldTransform`, `Transform2d`, `WorldTransform2d`, `CoordinateSystem` (spec §4.4).

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p engine --doc`
Expected: FAIL (crate missing).

- [ ] **Step 3: Implement**

`lib.rs` contains crate docs (crate map, guarantees: determinism, no global state, host owns the loop), `#![forbid(unsafe_code)]`, the re-exports, and `pub mod prelude`.

- [ ] **Step 4: Verify**

Run: `cargo test -p engine && cargo xtask wasm`
Expected: PASS; `engine` builds for `wasm32-unknown-unknown`, which also proves `SystemClock` is excluded there.

- [ ] **Step 5: Commit**

```bash
git add libs/engine Cargo.toml Cargo.lock xtask
git commit -m "feat(engine): add facade crate and prelude"
```

---

### Task 20: `fixed_step_sim` sample

**Files:**
- Create: `examples/fixed_step_sim/Cargo.toml`, `src/lib.rs`, `src/main.rs`, `tests/determinism.rs`, `tests/snapshot.rs`, `tests/allocations.rs`, `benches/sim.rs`

**Interfaces:**
- Consumes: the `engine` prelude.
- Produces, in `lib.rs`:
  - `#[derive(Clone, Debug, PartialEq, Eq)] pub struct SimConfig { pub seed: u64, pub jitter_seed: u64, pub ticks: u64, pub particles: usize }`, with `Default` = (42, 7, 600, 10_000);
  - `#[derive(Clone, Debug, Copy, PartialEq)] pub struct Particle { pub position: Vec3, pub velocity: Vec3, pub ttl: u32 }`;
  - `#[derive(Clone, Debug)] pub struct Sim { /* pool, sim_prng, jitter_prng, timestep, clock, last: Timestamp, pending: Steps, particles: usize */ }` with `Sim::new(&SimConfig) -> Sim`, `Sim::run_ticks(&mut self, n: u64)`, `Sim::checksum(&self) -> u64`, `Sim::summary(&self) -> SimSummary`, plus accessors `prng(&self) -> &Prng`, `timestep(&self) -> &FixedTimestep`, and `clock(&self) -> &ManualClock` for the snapshot test;
  - `#[derive(Clone, Copy, Debug, PartialEq, Eq)] pub struct SimSummary { pub tick: u64, pub live: usize, pub capacity: usize, pub checksum: u64 }`;
  - `pub fn run(config: &SimConfig) -> SimSummary`.

The sim rules decide what the checksum means, so they are fixed here:
```text
new: pool = HandlePool::with_capacity(particles); sim_prng = Prng::from_seed(seed)
     jitter_prng = Prng::from_seed(jitter_seed); timestep = FixedTimestep::from_hz(60)
     for _ in 0..particles: spawn()
spawn: h = pool.insert(Particle::ZERO); mut r = sim_prng.stream(h.to_bits())
       pool[h] = Particle { position: Vec3::new(r.range(-100.0f32..100.0), …×3), velocity: Vec3::new(r.range(-5.0f32..5.0), …×3), ttl: r.range(60u32..=600) }
       (components drawn in x, y, z order: position first, then velocity, then ttl)
step (dt = 0.016_666_667_f32, i.e. the 16_666_667 ns step as f32 seconds, computed once in Sim::new):
       for (_, p) in pool.iter_mut(): p.velocity.y -= 9.81 * dt; p.position += p.velocity * dt; p.ttl -= 1
       pool.retain(|_, p| p.ttl > 0); while pool.len() < particles: spawn()
run_ticks(n): done = 0
       loop: while done < n and pending.next() is Some(_): step(); done += 1
             if done == n: return            // unconsumed steps stay in `pending` for the next call
             frame = jitter_prng.range(4_000_000u64..=40_000_000); clock.advance(Duration::from_nanos(frame))
             pending = timestep.accumulate(clock.now().saturating_duration_since(last)); last = clock.now()
       (so run_ticks(300) followed by run_ticks(300) is identical to run_ticks(600))
checksum: FNV-1a 64 (offset 0xcbf2_9ce4_8422_2325, prime 0x0000_0100_0000_01b3) over, for each (h, p) in pool.iter():
       h.to_bits().to_le_bytes(), then position.x/y/z and velocity.x/y/z as f32::to_bits().to_le_bytes()
```
Only `Vec3`/`f32` arithmetic is allowed: no `Quat`/`Vec4`/`Mat4`, no transcendental functions (spec §5.6 item 8). Name `Particle::ZERO` explicitly as a `const`.

`main.rs`: hand-rolled parsing of `--seed`, `--jitter-seed`, `--ticks`, and `--particles` into `SimConfig`. It prints `tick=… live=… capacity=… checksum=0x…` and carries `#![allow(clippy::print_stdout)]` (example crate).

- [ ] **Step 1: Write the failing tests**

```rust
// determinism.rs
#[test] fn same_seed_same_checksum() { assert_eq!(run(&SimConfig::default()), run(&SimConfig::default())); }
#[test] fn different_seed_different_checksum() { /* seed 43 vs default: checksums differ */ }
#[test] fn jitter_split_does_not_matter() { /* jitter_seed 8 and 9999 give the default checksum */ }
#[test] fn golden_checksum() { assert_eq!(run(&SimConfig::default()).checksum, GOLDEN /* frozen in Step 4 */); }
// snapshot.rs
#[test] fn clone_mid_run_continues_identically() { /* run 300, clone, run both 300, checksums equal */ }
#[test] fn split_runs_equal_single_run() { /* Sim::new(default): run_ticks(300)+run_ticks(300) checksum == run_ticks(600) checksum */ }
#[test] fn raw_state_roundtrips_mid_run() {
    // after 300 ticks: Prng::from_state(sim.prng().to_state()).unwrap() == *sim.prng();
    // FixedTimestep::from_state(sim.timestep().to_state()) yields equal to_state();
    // ManualClock::starting_at(sim.clock().now()).now() == sim.clock().now()
}
// allocations.rs (own binary)
#[global_allocator] static ALLOC: dhat::Alloc = dhat::Alloc;
#[test] fn zero_allocations_in_steady_state() {
    let _p = dhat::Profiler::builder().testing().build();
    let mut sim = Sim::new(&SimConfig::default()); sim.run_ticks(120);
    let before = dhat::HeapStats::get().total_blocks; sim.run_ticks(600);
    dhat::assert_eq!(dhat::HeapStats::get().total_blocks, before);
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p fixed_step_sim`
Expected: FAIL.

- [ ] **Step 3: Implement** per the rules above. Dev-dependencies: `dhat` and `criterion`. Add `benches/sim.rs`, which runs the full default configuration (`[[bench]] name = "sim" harness = false`).

- [ ] **Step 4: Freeze the golden checksum**

Run `cargo run -p fixed_step_sim --release` and put the printed checksum into `GOLDEN`.

Run: `cargo test -p fixed_step_sim && cargo test -p fixed_step_sim --release`
Expected: PASS in both profiles. The sim only uses basic IEEE operations, so the profile can't change the result.

- [ ] **Step 5: Commit**

```bash
git add examples/fixed_step_sim Cargo.lock
git commit -m "feat(example): add deterministic fixed_step_sim with snapshot and allocation tests"
```

---

### Task 21: Benchmark run, `docs/benchmarks.md`, final verification, PR

**Files:**
- Create: `docs/benchmarks.md`
- Modify: `README.md` (finalize the crate map, link to `docs/benchmarks.md`)

- [ ] **Step 1: Run the comparison benchmarks**

Run: `cargo xtask bench --save m1` on the M4 Pro, plugged in, with other apps closed.
Expected: criterion baselines saved under `target/criterion`.

- [ ] **Step 2: Write `docs/benchmarks.md`**

- **Methodology:** toolchain, the `[profile.bench]` settings, CPU model and cache sizes (`sysctl -a | grep -E 'hw.(l1d|l2|perflevel)'`), macOS version, and power state.
- **Results tables** per subject from spec §8, using criterion's medians.
- **Interpretation:** honest, including where we lose.
- **Note:** wall-clock numbers come from aarch64 macOS; instruction-count gating starts in M2.

- [ ] **Step 3: Full verification**

Run: `cargo xtask ci`
Expected: every gating step passes locally.

- [ ] **Step 4: Commit and push**

```bash
git add docs/benchmarks.md README.md
git commit -m "docs: record M1 benchmark results and finalize README"
git push
```
Expected: the CI ubuntu jobs are green, **including `golden_checksum` on x86_64**, which proves the cross-architecture checksum (spec §5.6 item 8).

- [ ] **Step 5: Open the PR**

Run: `gh pr create --base main --title "M0+M1: engineering foundations and core primitives" --body "<summary of phases, DoD checklist from spec §9 with each item ticked, link to spec and plan>

🤖 Generated with [Claude Code](https://claude.com/claude-code)"`
Expected: the PR exists and the macOS/Windows legs run. **Ask the user before merging.**
