# Stage 5: review

1. **Simplify.** Dispatch one `builder` to run `/simplify` over `main...HEAD` (behaviour-preserving cleanups only), reading every changed source file in full; a report that lists files it skipped or only skimmed goes back. Check and commit its changes on their own: `refactor: simplify (<id>)`. No commit if nothing changed.
2. **Review in parallel.** Dispatch fresh reviewers, each with the spec path, the diff range `main...HEAD` and the plan's lesson IDs, and nothing of your reasoning:
   - `review-code`, `review-security`, `review-docs`, `review-spec`, always;
   - `review-conditional`, naming which of public API, determinism, performance and portability apply (see the table in CONTRIBUTING.md, "Reviews before a pull request"); the others are `n/a: <reason>`.
   Performance claims need a saved baseline and the noise floor; `/bench-run` for wall-clock runs.
3. **Triage** each finding: fix, or record why not (for the PR). Log the triage.
4. **Fix.** Send the fixes to a `builder`; check and commit as in stage 4 (`fix: <what> (<id>)`).
5. **Re-review** each fixed finding with a **fresh** reviewer of the same kind, never a continued one. After two rounds on the same finding without agreement, stop and ask the maintainer.
6. **Re-verify.** After the last fix, run the full `cargo xtask ci` again on the final code commit, with nothing else running `cargo`: a run recorded before the simplify pass or the fixes no longer covers the code (M3.5). The same goes for the chunk's measurements (benchmark runs, allocation counts): re-run them on the final code commit, or say in the pull request which commit each ran on (M3.6).
7. Write each review's one-line result and agent file into `log.md` for the PR's Reviews section. Go to [stage 6](6-compound.md).
