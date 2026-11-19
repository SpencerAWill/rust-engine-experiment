# Stage 4: work

For each unticked plan step, in order:

1. **Dispatch.** A `builder` step: brief the builder with the step, the spec, the chunk folder and the step's lesson IDs. A main-session step: do it yourself.
2. **Check.** `git status --short` and `git diff --stat`: only the step's files changed (plus `log.md`). Read the diff. Re-run the step's verify command yourself; do not take the report's word for it. If something is off, send it back to a builder with what is wrong.
3. **Record.** Tick the step's checkbox in `plan.md`; make sure `log.md` has the step's `done` line and any surprises or `lesson?` lines from the report.
4. **Commit** by explicit path with the step's message (conventional, ending with the chunk ID).

Anything that differs from the plan goes in its Deviations section as it happens. When every step is ticked, run `cargo xtask ci` (the full one) and go to [stage 5](5-review.md).
