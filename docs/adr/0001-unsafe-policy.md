# ADR 0001: `unsafe` policy

Status: Accepted (2026-11-12)

## Context

The engine will eventually need `unsafe` in a few places: type-erased component storage, the C ABI layer, and possibly lock-free primitives. Until then, every line of `unsafe` is risk without payoff. Cargo cannot override one workspace lint per crate, so a workspace-wide `deny` alone cannot express "forbidden except in these named crates".

## Decision

- `unsafe_code = "deny"` at the workspace level, and `#![forbid(unsafe_code)]` at every crate root. The `forbid` in source is the real enforcement.
- `cargo xtask check-unsafe` fails if any workspace crate root lacks the `forbid` attribute, unless the crate is listed in `unsafe-allowlist.toml` with an ADR that justifies it.
- Allowlisted crates must:
  1. put a `// SAFETY:` comment on every `unsafe` block (clippy `undocumented_unsafe_blocks = "deny"`);
  2. document invariants on every `unsafe fn` and `unsafe trait` in a `# Safety` section;
  3. confine `unsafe` to the smallest possible modules and `#[deny(unsafe_code)]` the rest;
  4. run under Miri in CI;
  5. have at least one fuzz target.
- Miri and fuzzing infrastructure is built at the start of M2, together with the first allowlisted crate. M0+M1 contains no `unsafe`, so these requirements have nothing to apply to yet.

## Consequences

- Safe Rust is the default, and each exception is a named, reviewed, tested crate.
- A crate that needs `unsafe` pays for it up front with an ADR, Miri, and fuzzing.
- The allowlist is empty in M0+M1, which `check-unsafe` verifies on every CI run.
