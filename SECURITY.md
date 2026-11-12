# Security policy

## Reporting a vulnerability

Please report suspected vulnerabilities **privately**, using GitHub's private vulnerability reporting: open the repository's **Security** tab and choose **Report a vulnerability**. Do not open a public issue or pull request for a security problem.

Include what you found, how to reproduce it (a minimal program or test is ideal), and the version or commit you tested. This is a small experimental project, so responses are best-effort and there is no fixed response time. We will acknowledge a report, say whether we consider it a vulnerability, and credit you in the fix if you want that.

## Supported versions

The project is experimental. All crates are `0.x`, `publish = false`, and their APIs are explicitly unstable. Only the latest commit on `main` is supported; fixes are not backported.

## What counts as a vulnerability here

The engine is a set of libraries with no network, file-format parsing, authentication, or cryptography. Every crate has `#![forbid(unsafe_code)]`, and CI fails if one does not (`cargo xtask check-unsafe`). That shapes what is in scope:

- **In scope:** a memory-safety or soundness bug in any crate, which would also break the `unsafe` policy (this should be impossible in safe Rust, so it would point at a compiler or dependency issue); a way to make a public API read or write outside its own data; supply-chain problems in this repository's build and CI configuration (for example an unpinned action, an over-broad workflow token, or a malicious dependency).
- **Not vulnerabilities, by design:**
  - `Prng` (`engine_random`) is a fast, deterministic, **non-cryptographic** generator. Do not use it for keys, tokens, or anything an attacker must not predict. Its output is predictable from a small amount of state, and its streams are frozen on purpose.
  - A `Handle` is an identifier, not a capability. Using a handle on a different pool than the one that issued it can resolve to an unrelated value. This is a documented limitation (see the `engine_handle` crate docs).
  - Determinism is a correctness and reproducibility property, not a security boundary.
  - Panics on documented misuse (each documented panic has a non-panicking `try_` twin) and resource exhaustion from huge requests are reliability issues. They are welcome as ordinary bug reports.

## Our own safeguards

- Third-party GitHub Actions are pinned to commit hashes, workflows use a read-only token, and `cargo xtask check-workflows` enforces this in CI.
- `cargo-deny` checks advisories, licences, and sources on every push.
- Dependabot proposes dependency and action updates weekly.
