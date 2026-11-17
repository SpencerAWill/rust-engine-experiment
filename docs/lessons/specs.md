---
applies_to:
  - "docs/chunks/**"
---
# Lessons: writing specs and plans

- Status: Living (2026-11-15)

- **specs-1.** A §8 check written as "finds nothing" names the paths it covers. *Why:* `git grep 'design/m[0-9]'` also matched historical records that quote the old paths on purpose, so the check could never pass as written (M3.15). *From:* M3.15 · *Confirmed:* 2026-11-15
- **specs-2.** Evidence that §8 says will be "noted in `log.md`" is its own plan step, or it is forgotten. *Why:* M3.15's R7 follow-up note was missing until the spec review asked for it. *From:* M3.15 · *Confirmed:* 2026-11-15
- **specs-3.** When a spec asks for a test per row of a table (every panic of §4.x, every operation), the plan names one test per row. *Why:* M3.17's plan named the panic tests loosely, and four of §4.5's rows (`ensure_capacity`, both `Extend`s, `From<[usize; N]>`) shipped untested until review. *From:* M3.17 · *Confirmed:* 2026-11-16
- **specs-4.** Evidence recorded in `log.md` names the commit it ran on; a library change after it (a review fix, a simplify pass) means re-running it or saying in the pull request that it predates the change. *Why:* M3.17's fuzz, wasm, full `ci` and `bench-regress` entries predated three later library commits, and the spec review could not tell which code they covered. *From:* M3.17 · *Confirmed:* 2026-11-16
- **specs-5.** When a spec rejects wrapping a crate but builds our own version of its design, the plan has a step that cites the design in the shipped rustdoc (ADR 0002), not only in the spec. *Why:* M3.17 followed `hibitset`'s design and cited it only in the spec until the security review. *From:* M3.17 · *Confirmed:* 2026-11-16
