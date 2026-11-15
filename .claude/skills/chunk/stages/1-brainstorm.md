# Stage 1: brainstorm

Goal: a `Draft` spec the maintainer recognizes as what they want.

1. Read the roadmap row, any linked issue, and the code and docs the chunk touches, until you can say how things work today and what would change. Do not ask what the repository answers.
2. Copy `docs/chunks/_template/spec.md` and `log.md` into `docs/chunks/<id>-<slug>/`. Set both statuses (`Draft (<date>)`, `Living (<date>)`); set the roadmap row to `Drafting`.
3. Talk with the maintainer: play back what you understood, then ask about what would change what gets built, one decision at a time, each with your recommendation. Lay out the real alternatives for how to build it and recommend one before writing it up. Log each decision as it is made.
4. Write the spec. Mark anything you decided yourself as your call.
5. Commit (`docs: draft the <title> spec (<id>)`) and go to [stage 2](2-grill.md). Do not ask for approval yet.
