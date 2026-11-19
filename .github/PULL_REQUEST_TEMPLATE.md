## Chunk

<!-- Roadmap work: the chunk ID and links (see docs/roadmap.md). Otherwise "None", for example for a dependency update or a small fix. The title ends with the chunk ID, for example "(M2.1)". -->

- Chunk:
- Spec:
- Plan:

## What and why

<!-- What does this change, and why? Link the issue if there is one. -->

## Reviews

<!-- One line each: the result, and the agent file that did it (.claude/agents/), or `n/a: <reason>` for a conditional review that does not apply. See CONTRIBUTING.md, "Reviews before a pull request". -->

- Code (`review-code`):
- Security (`review-security`):
- Documentation (`review-docs`):
- Spec conformance (`review-spec`):
- Public API (`review-conditional`):
- Determinism (`review-conditional`):
- Performance (`review-conditional`):
- Portability (`review-conditional`):

## Lessons

<!-- Lessons added, edited or deleted in docs/lessons/, by ID, one line each. "None" if none. -->

## Follow-ups

<!-- Issues opened for work found during this chunk, by number. "None" if none. -->

## Checklist

- [ ] `cargo xtask ci` passes locally.
- [ ] New behaviour has tests; for a bug fix, the test failed first for the expected reason.
- [ ] I called out any change to a **public API, a frozen format** (PRNG streams, `UntypedGenId::to_bits`), **or a policy**, and added or updated an ADR where the decision is hard to reverse.
- [ ] No code was copied from another project (see ADR 0002). If an AI assistant was used, I say so here.
- [ ] Performance claims come with a saved baseline and the noise floor (`cargo xtask bench-report`).
- [ ] User-visible changes are in `CHANGELOG.md`.
- [ ] For a chunk: the spec is `Implemented`, the plan and log are `Historical` with the plan's Deviations filled in, and the roadmap row is `Done`.
