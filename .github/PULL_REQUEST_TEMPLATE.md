## Chunk

<!-- Roadmap work: the chunk ID and links (see docs/roadmap.md). Otherwise "None", for example for a dependency update or a small fix. The title ends with the chunk ID, for example "(M2.1)". -->

- Chunk:
- Spec:
- Plan:

## What and why

<!-- What does this change, and why? Link the issue if there is one. -->

## Checklist

- [ ] `cargo xtask ci` passes locally.
- [ ] New behaviour has tests; for a bug fix, the test failed first for the expected reason.
- [ ] Public items are documented, with a runnable doctest for new public types; documented panics have a `try_` twin.
- [ ] I called out any change to a **public API, a frozen format** (PRNG streams, `Handle::to_bits`), **or a policy**, and added or updated an ADR where the decision is hard to reverse.
- [ ] No code was copied from another project (see ADR 0002). If an AI assistant was used, I say so here.
- [ ] Performance claims come with a saved baseline and the noise floor (`cargo xtask bench-report`).
- [ ] User-visible changes are in `CHANGELOG.md`.
- [ ] For a chunk: the spec is `Implemented`, the plan is `Historical` with its Deviations filled in, and the roadmap row is `Done`.
