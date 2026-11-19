# Stage 2: grill

Run `/grill` on the spec (`.claude/skills/grill/SKILL.md`). It ends with the maintainer's approval or a list of changes.

On approval: set the spec to `Accepted (<date>)`, the roadmap row to `Accepted`, log it, commit (`docs: accept the <title> spec (<id>)`), and go to [stage 3](3-plan.md) without waiting.

If the chunk adds or changes an ADR, the ADR is part of the spec under review and is accepted with it.
