# Stage 7: close out

1. **Follow-ups.** For work found during the chunk that belongs elsewhere, open an issue (labelled, linking the chunk, with the headings and the `ai-assisted` label of [CONTRIBUTING.md](../../../../CONTRIBUTING.md#issues)) without asking. Keep the numbers.
2. **Pull request.** `cargo xtask ci` passes. Push the branch (`git push -u origin <branch>`) and open the PR from `.github/PULL_REQUEST_TEMPLATE.md`: title a conventional commit ending with the chunk ID; Reviews (eight lines, each naming its agent file), Lessons, Follow-ups; say that an assistant was used.
3. **Close-out commit**, at once, without waiting for CI: spec `Implemented (PR #n, <date>)`, plan `Historical: executed in PR #n (<date>); the spec and the code are authoritative` with Deviations filled in, log `Historical: PR #n (<date>)`, roadmap row `Done (PR #n)`, the spec's done-when list ticked with evidence (leave a pending item, such as "CI green", unticked with a note), `CHANGELOG.md` current. Push.
4. **Wait** for the merge go-ahead. Tell the maintainer what is in the PR in a few lines, with its link.
5. **After the go-ahead**, in the same turn:
   - wait for the required checks (`gh pr checks <n> --required --watch` exits 0; the admin can bypass the ruleset, so this wait is the only thing enforcing them); if one fails, fix it as in stage 5 and ask for the go-ahead again;
   - note the head commit the checks passed on (`gh pr view <n> --json headRefOid -q .headRefOid`) and merge exactly that: `gh pr merge <n> --squash --delete-branch --match-head-commit <sha>` (never `--admin`, never bypass a check);
   - if the PR changed `infra/github/`, remind the maintainer to run `pulumi preview` and `pulumi up` there;
   - `git checkout main && git pull`;
   - name the chunk that comes next (the rule in `SKILL.md`, `next` step 2) and stop. Do not start it: the maintainer runs `/chunk next` in a new session.
