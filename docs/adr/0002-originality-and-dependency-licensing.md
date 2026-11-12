# ADR 0002: Originality and dependency licensing

Status: Accepted (2026-11-12)

## Context

The project may become open source. It also uses AI assistance in development. Both raise provenance questions: whose code is this, and under what terms can it be reused? Answering them later is far more expensive than recording the rules now.

## Decision

- The engine is dual-licensed `MIT OR Apache-2.0` (`LICENSE-MIT`, `LICENSE-APACHE`), with the standard contribution clause in the README.
- **No code is copied from other projects**, Bevy included. Designs, papers, talks, and blog posts may be studied and cited. All implementation is original.
- Published algorithms (xoshiro256++, SplitMix64, Lemire's bounded random integers, Fisher–Yates) are implemented from their papers and specifications. Their published test vectors may be used as golden values.
- Reading another crate's source to understand its behavior is fine. Using other crates as test oracles or as benchmark dev-dependencies is fine. Transcribing code is not.
- Where an implementation closely follows a known published design, the docs cite that design.
- AI-assisted code is written from specifications. Code from other repositories is never pasted in.
- `deny.toml` allows only `MIT`, `Apache-2.0`, `Apache-2.0 WITH LLVM-exception`, `BSD-2-Clause`, `BSD-3-Clause`, `ISC`, `Zlib`, and `Unicode-3.0`. Unknown registries and git sources are denied, duplicate versions warn, and security advisories deny. `cargo xtask deny` enforces this in CI.

## Consequences

- Provenance is clean for a future release, with no relicensing or rewrite needed.
- Dependencies with copyleft or unusual licenses are rejected by CI, not by memory.
- Comparison benchmarks against competing crates remain possible, because they are dev-dependencies only.
