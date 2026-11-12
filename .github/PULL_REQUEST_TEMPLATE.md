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
