# Guide for AI coding assistants

This file is a short entry point. The rules are in [CONTRIBUTING.md](CONTRIBUTING.md); read it first. Design background is in [docs/README.md](docs/README.md).

## Commands

```sh
cargo xtask ci          # everything CI runs; run it before saying work is done
cargo xtask ci --quick  # fmt check, clippy, and tests only
cargo xtask test        # nextest plus doctests
                        # the other commands are listed in the README
```

## Rules that fail CI (do not work around them)

- No `unsafe` (`#![forbid(unsafe_code)]` in every crate root).
- Determinism: no `HashMap`/`HashSet`/`RandomState`, `Instant::now`, `SystemTime::now`, `thread_local!`, or inherent transcendental float methods in library code. No global mutable state.
- Frozen formats: PRNG output streams and `Handle::to_bits` do not change without an ADR and updated golden tests.
- Every public item is documented and public types have runnable doctests. Every documented panic has `#[track_caller]`, a `# Panics` section, an actionable message, and a fallible `try_` twin.
- No code copied from other projects (see [ADR 0002](docs/adr/0002-originality-and-dependency-licensing.md)).

## How to work here

- **Test first.** For a bug fix, write the test, watch it fail for the reason you expect, then fix the code. Never weaken or delete a failing test without saying so.
- **Verify before claiming.** Run the command and read its output; do not infer success from the diff.
- **Stop and ask** before changing a public API, a frozen format, or a policy, and before anything outward-facing (a push to a shared branch, a merge, a publish).
- **Small logical commits**, conventional messages, explicit file paths when staging. Say in the pull request that an assistant was used.
- **Report honestly**: if a test fails, a step was skipped, or a measurement is noisy, say so with the evidence. Benchmark claims need a saved baseline and the noise floor (see `cargo xtask bench-report`).
- **Docs**: give new documents a status line, add an ADR for hard-to-reverse decisions, and keep spec section numbers stable.
