# Contributing

Thanks for your interest. This is an experimental engine: APIs change, and some decisions are deliberately deferred (see `docs/superpowers/specs/`). For anything beyond a small fix, **open an issue first** so we can agree on the direction before you spend time on it.

## Licence and originality

The project is dual-licensed `MIT OR Apache-2.0`. Unless you state otherwise, a contribution you submit is licensed under both, with no additional terms (see the README).

The rules that make that safe are in [ADR 0002](docs/adr/0002-originality-and-dependency-licensing.md):

- **Do not copy code from other projects**, including other engines. You may study designs, papers, talks, and blog posts, and cite them. Reading another crate's source to understand its behaviour is fine; transcribing it is not.
- Published algorithms are implemented from their papers and specifications; their published test vectors may be used as golden values.
- Other crates may be used as test oracles or benchmark dev-dependencies, but a new dependency must pass `cargo xtask deny` (allowed licences, no unknown registries or git sources).
- If you used an AI assistant, say so in the pull request. The same rule applies: write from specifications, never paste code from other repositories.

## Setting up

```sh
rustup show                 # installs the pinned toolchain from rust-toolchain.toml
cargo xtask ci              # everything CI runs, in order
cargo xtask ci --quick      # fmt check, clippy, and tests only
```

`cargo xtask` prints an install hint when a tool is missing. The extra tools are `cargo-nextest`, `cargo-deny`, `critcmp`, and `wasmtime`; `.devcontainer/install-tools.sh` installs them.

## What CI enforces

These are policy, not preferences. A pull request that breaks one will not be merged:

- **No `unsafe`.** Every crate root has `#![forbid(unsafe_code)]` (`check-unsafe`). An exception needs an ADR and an entry in `unsafe-allowlist.toml`.
- **Determinism.** No `std` `HashMap`/`HashSet`/`RandomState`, `Instant::now`, `SystemTime::now`, `thread_local!`, or inherent transcendental float methods in library code (see `clippy.toml`; transcendentals go through `engine_math::scalar`). Simulation state must be snapshot-able plain data.
- **No global mutable state** (`check-statics`, [ADR 0008](docs/adr/0008-no-global-state.md)).
- **Frozen formats.** The PRNG output streams and `Handle::to_bits` are frozen for all of `0.x`. Changing one needs an ADR and updated golden tests, not just a code change.
- **Documentation.** Every public item is documented and every public type has a runnable doctest.
- **Panics.** A documented panic needs `#[track_caller]`, a `# Panics` section, an actionable message, and a fallible `try_` twin.
- **Workflows** (`check-workflows`): actions pinned to commit hashes, a read-only token, no persisted checkout credentials, no `pull_request_target`.
- Formatting (`cargo fmt`), clippy with warnings denied, docs without warnings, and `no_std` and WebAssembly builds.

## Tests

Changes come with tests. For a bug fix, write the test first and confirm it fails for the reason you expect, then fix the code. Prefer testing behaviour over implementation. The layers in use are unit tests, proptest model tests against a simple reference, golden values, and doctests. Do not weaken or delete a failing test without saying so in the pull request.

## Benchmarks

Performance claims need measurements. Wall-clock numbers from a laptop or shared CI runner are noisy (we have measured about 5% drift between runs on a quiet machine), so:

- Run on AC power with other apps closed, and record the machine and conditions.
- Compare against a saved baseline: `cargo xtask bench --save NAME`, then `cargo xtask bench-report --baseline NEW --against OLD`. The report only calls a change real if it exceeds 5% and the confidence intervals do not overlap.
- Say what you measured and what you did not.

## Commits and pull requests

- Use conventional commit messages (`feat:`, `fix:`, `perf:`, `refactor:`, `test:`, `docs:`, `ci:`, `chore:`), and split work into small logical commits that each pass `cargo xtask ci`.
- Explain why, not just what. Mention any change to a public API, a frozen format, or a policy explicitly.
- Add or update an ADR in `docs/adr/` for a decision that is hard to reverse.

## Security

Report vulnerabilities privately, as described in [SECURITY.md](SECURITY.md), not in public issues.
