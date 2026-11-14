---
name: Feature request
about: Propose an engine feature, an unmeasured performance idea, or a change to an API or a design decision
labels: enhancement
---

This is an experimental engine and some decisions are deliberately deferred (see `docs/roadmap.md`). Please open an issue before writing a large change. For CI, tooling or repository work, use the **Infrastructure or chore** template; for a question, start a discussion.

**The problem** (what are you trying to do, and what gets in the way?)

**Proposal**

**Alternatives you considered**

**Impact:** does it change a public API, a frozen format (PRNG streams, `Handle::to_bits`), or a policy (`unsafe`, determinism, no global state)? Those need an ADR.
