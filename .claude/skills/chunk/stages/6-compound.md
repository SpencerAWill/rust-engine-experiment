# Stage 6: compound

Turn what this chunk learned into something the next chunk will find. Lessons were captured all along in `log.md`; this stage promotes them.

For each `lesson?` line in `log.md`, and anything the reviewers reported as `lesson?`:

1. **Gate.** Keep it only if it is non-obvious from the code, likely to recur, and costly to miss. If a test, lint or `xtask` check can enforce it, write that instead (through a builder) and skip the lesson.
2. **Overlap.** Search `docs/lessons/` for a lesson on the same point. If there is one, edit that bullet (refresh *Confirmed*, sharpen it) instead of adding.
3. **Write** the new bullet in the right topic file (create the file, with `applies_to`, and add it to `docs/lessons/README.md` if no topic fits), in the format of `docs/lessons/README.md`. Edit single bullets; never rewrite a file.
4. Note the ID and whether it was added, edited or deleted, for the PR's Lessons section.

A lesson may also belong somewhere other than `docs/lessons/`: a fix to a template, a skill, an agent file or `AGENTS.md`. Make that edit directly and list it the same way. Commit: `docs: record the lessons of <id> (<id>)`. Go to [stage 7](7-close-out.md).
