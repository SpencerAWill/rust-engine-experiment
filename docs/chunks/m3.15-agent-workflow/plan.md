# M3.15 plan: Agent workflow and lessons

- **Status:** Accepted (2026-11-15). No plan stop under the new loop (spec §4.13); the maintainer reviews it with the pull request.
- **Spec:** [spec.md](spec.md). This plan argues from the spec; read both. Progress and surprises go in [log.md](log.md).

## 1. Approach

Paths are made final first (step 2 moves the chunk folders), so every later file is written at its final location. The main session (Opus) writes the Markdown that needs judgement: agent files, skills, lessons, workflow docs. Builders (Sonnet) write the code: the push guard in `xtask`, the hook script, the mutants container scripts, and the folder move. That is the split of spec §4.1 ("the main session MAY edit documentation directly"); skills and agent files count as documentation.

Until Claude Code registers the new agent files, builders and reviewers are dispatched as general-purpose agents with the agent file's `model` and its body as the prompt (spec §4.13); `log.md` records which way each dispatch went.

Choices the spec leaves open, decided here:

- **Push guard contract.** `cargo -q xtask guard-push` reads the hook JSON on stdin and exits 0 to allow, or 2 with the reason on stderr to block (Claude Code treats exit 2 from a `PreToolUse` hook as a block and shows stderr to the agent). Input it cannot read is blocked, with a message saying the maintainer can push with `! git push` (user `!` commands do not pass through hooks). A guard that fails open would be no guard.
- **Command parsing.** Split the command on `&&`, `||`, `;`, `|` and newlines; split each segment into words with single and double quotes respected; a segment is a push when its words are `git`, then any global options (`-C <dir>`, `-c <k=v>`, `--git-dir=…`, `--work-tree=…`), then `push`. Text inside quotes never starts a segment, so `echo "git push origin main"` is allowed.
- **Mutants container.** `debian:trixie` with rustup installing the toolchain from `rust-toolchain.toml`, prebuilt `cargo-nextest`, `cargo install --locked cargo-mutants --version 27.1.0`, named volumes for the cargo registry, rustup and `target/`; `docker run --memory=4g --memory-swap=4g`; `-j 1`. The watchdog polls every 5 s and `docker kill`s the container when it uses over 3.5 GB or macOS reports fewer than 1 GB free.
- **Link check.** A throwaway script in the session scratchpad: for every tracked `*.md`, resolve each relative link target against the file's directory and report the ones that do not exist. Its output goes in `log.md`; it is not committed (M3.16's `check-docs` is the lasting check).

## 2. Steps

### Step 1: Accept the spec and start the plan (R13)

- [x] **Who:** main session.
- **Files:** `spec.md` (status `Accepted (2026-11-15)`), `plan.md` (this file), `log.md`, `docs/roadmap.md` (M3.15 row `In progress`), `.gitignore` (`.claude/worktrees/`, `.claude/settings.local.json`, `.claude/agent-memory-local/`).
- **Test first:** n/a (documentation).
- **Verify:** `git status --short` shows only these files; `git check-ignore .claude/worktrees/x` prints the path.
- **Commit:** `docs: accept the M3.15 spec and add its plan (M3.15)`

### Step 2: Move chunk folders to `docs/chunks/` (R14)

- [x] **Who:** builder.
- **Files:** every folder under `docs/design/` (`git mv docs/design/_template docs/chunks/_template`, `git mv docs/design/m0-m1-foundations docs/chunks/m0-m1-foundations`, and each `docs/design/m<N>-<topic>/m<N>.<n>-<slug>` to `docs/chunks/m<N>.<n>-<slug>`); the 8 files outside the folders that link into them (`git grep -l 'design/m[0-9]\|design/_template'`); relative links inside the moved files that now point one level wrong; `docs/README.md` (table row and Conventions bullet for `docs/chunks/`); `CONTRIBUTING.md` "How work is organized" (template path); `spikes/ecs_storage/harness/src/lib.rs` (a doc comment path).
- **Test first:** run the link-check script (§1) before the move and record the count of broken links (expected 0); after `git mv` alone, it reports the moved targets as broken.
- **Change:** the moves, then fix every reported link. Do not edit anything in an ADR except link paths.
- **Verify:** `git grep -n 'docs/design\|design/m[0-9]'` prints nothing except historical prose that names the old path on purpose (none expected); the link-check script reports 0 broken links; `git log --follow --oneline docs/chunks/m3.14-bitset/spec.md | wc -l` is greater than 1; `cargo xtask ci --quick` passes (the doc comment).
- **Commit:** `docs: move the chunk folders from docs/design to docs/chunks (M3.15)`

### Step 3: Templates and the pull request template (R6)

- [x] **Who:** main session.
- **Files:** `docs/chunks/_template/log.md` (new, spec §4.3), `docs/chunks/_template/plan.md` (checkbox per step, `Who:` line, Lessons section, length note), `docs/chunks/_template/spec.md` (length note; §3, §7, §11 optional for small chunks), `.github/PULL_REQUEST_TEMPLATE.md` (Reviews, Lessons, Follow-ups sections; drop the checklist lines they duplicate).
- **Test first:** n/a (documentation).
- **Verify:** link-check script reports 0; each template's status line still says `Template`.
- **Commit:** `docs: add the log template and the plan's lessons section (M3.15)`

### Step 4: Agent roles (R3)

- [x] **Who:** main session.
- **Files:** `.claude/agents/{builder,review-code,review-security,review-conditional,review-docs,review-spec,lessons-researcher,spec-reader}.md`.
- **Content:** frontmatter `name`, `description`, `model` and `tools` exactly as spec §4.2 (reviewers, researcher and cold reader: `Read, Grep, Glob, Bash`; builder: every tool but `Agent`). Each body says what the role gets, what it returns (findings as `file:line`, severity, one-line fix; or for the builder, the step result plus `log.md` entries), the rule against editing (reviewers), and links its checklist (`CONTRIBUTING.md#reviews-before-a-pull-request` and the spec) instead of copying it. Every body tells the agent to append candidate lessons to the chunk's `log.md` (builder) or to report them (read-only roles).
- **Test first:** n/a (configuration). 
- **Verify:** `grep -h '^model:' .claude/agents/*.md | sort | uniq -c` gives 4 `opus`, 3 `sonnet`, 1 `haiku`; no read-only role lists `Edit` or `Write`.
- **Commit:** `feat(claude): add the agent roles (M3.15)`

### Step 5: Push guard in `xtask` (R8)

- [ ] **Who:** builder.
- **Files:** `xtask/src/guard_push.rs` (new), `xtask/src/main.rs` (`Command::GuardPush`, `"guard-push"` in `parse`, exit code 2 on block), `.claude/hooks/guard-push.sh` (new).
- **Interfaces:** `pub(crate) enum Decision { Allow, Block(String) }`; `pub(crate) fn decide(command: &str, current_branch: Option<&str>) -> Decision`; `pub(crate) fn run() -> ExitCode` (reads stdin, takes `tool_input.command` and `cwd` with `serde_json`, gets the branch with `git -C <cwd> rev-parse --abbrev-ref HEAD`, calls `decide`).
- **Test first:** unit tests in `guard_push.rs`, written and run before `decide` exists (expected: compile error, then failures against a stub that returns `Allow`):
  - `blocks` (each `Block`): `git push origin main`; `git push origin HEAD:main`; `git push origin x:refs/heads/main`; `git push origin refs/heads/main`; `git push --force origin x`; `git push -f origin x`; `git push -uf origin x`; `git push --force-with-lease origin x`; `git push --force-with-lease=x:abc origin x`; `git push origin +x`; `git push --all origin`; `git push --mirror origin`; `git push origin --delete main`; `git push origin :main`; `git add a && git push origin main`; `cd r; git push origin main`; `git -C /r push origin main`; `git -c a=b push origin main`; `(cd r && git push origin main)`; `git push` with branch `main`; `git push origin` with branch `main`; malformed quoting (`git push "origin main`).
  - `allows` (each `Allow`): `git push -u origin m3.15-agent-workflow`; `git push` with branch `m3.15-agent-workflow`; `git push origin maintenance`; `git push origin main-docs`; `git status`; `echo "git push origin main"`; `git commit -m "push to main"`; `git log --grep push`.
  - `run_reads_hook_json`: `run`'s input parsing on `{"tool_input":{"command":"git push origin main"},"cwd":"."}` yields a block; on `{}` yields a block that names the missing field.
- **Change:** implement per §1 ("Command parsing"). `guard-push.sh`: POSIX `sh`; read stdin into a variable; if it does not contain `push`, exit 0; else pipe it to `cargo -q xtask guard-push` and exit with its status.
- **Verify:** `cargo xtask test` passes; `cargo xtask ci --quick` passes; `printf '%s' '{"tool_input":{"command":"git push origin main"},"cwd":"."}' | sh .claude/hooks/guard-push.sh; echo $?` prints the reason and `2`; the same with `git status` prints `0` and is instant (time both; record in `log.md`).
- **Commit:** `feat(xtask): add guard-push, a hook that blocks pushes to main and force pushes (M3.15)`

### Step 6: Settings: hook wiring and read-only allowlist (R8)

- [ ] **Who:** main session.
- **Files:** `.claude/settings.json` (new).
- **Change:** `hooks.PreToolUse` with matcher `Bash` running `sh "$CLAUDE_PROJECT_DIR/.claude/hooks/guard-push.sh"`; `permissions.allow` built by running `/fewer-permission-prompts`, then cut by hand to read-only commands (nothing that writes, pushes, deletes or installs). No `model` key (spec §2).
- **Verify:** hook trial in this session: run `git push --dry-run origin main` through Bash and confirm it is blocked with the guard's reason; run `git push --dry-run origin m3.15-agent-workflow` and confirm it is not. Record both in `log.md`.
- **Commit:** `feat(claude): wire the push guard and add a read-only allowlist (M3.15)`

### Step 7: Lessons (R7, R9)

- [ ] **Who:** main session.
- **Files:** `docs/lessons/README.md` (what a lesson is, the format, the capture gate, the index), `docs/lessons/testing.md` (`testing-1`, spec §4.4).
- **Verify:** `git ls-files 'libs/engine/src/**/tests/**' | head -1` prints a file (the `applies_to` pattern matches); link-check reports 0.
- **Commit:** `docs: add docs/lessons with its first lesson (M3.15)`

### Step 8: The `chunk` and `grill` skills (R4, R5, R12, R15)

- [ ] **Who:** main session.
- **Files:** `.claude/skills/chunk/SKILL.md` (under 150 lines: argument dispatch for `next`, `resume`, `<stage>`; the stop points and escalations; the plugin rule), `.claude/skills/chunk/stages/{1-brainstorm,2-grill,3-plan,4-work,5-review,6-compound,7-close-out}.md`, `.claude/skills/grill/SKILL.md` (spec §4.10).
- **Content notes:** `3-plan` dispatches `lessons-researcher` with the paths the spec names; `4-work` is the dispatch, check, tick, commit cycle with `log.md`; `5-review` runs the simplify builder first, then the reviewers in parallel, a fresh reviewer per re-review, at most two fix rounds; `7-close-out` covers the PR sections, follow-up issues, the close-out commit, and after the go-ahead the merge (`gh pr merge --squash --delete-branch` once checks pass, never `--admin`), the `pulumi` reminder and `/chunk next`. `resume` is in `SKILL.md` (spec §4.3).
- **Test first (skill trial, the `writing-skills` method):** before writing, dispatch a general-purpose agent on this repository with the task "resume chunk M3.15 and say what you would do next" and no skill; record what it misses. After writing, repeat with the skill text; it must name the first unticked step of this plan and the uncommitted changes, if any. Do the same for `grill` against `docs/chunks/m3.15-agent-workflow/spec.md` (read-only: it reports its first round of questions and stops). Record both trials in `log.md`.
- **Verify:** `wc -l .claude/skills/chunk/SKILL.md` under 150; the trials pass as above.
- **Commit:** `feat(claude): add the chunk and grill skills (M3.15)`

### Step 9: The resume trial (R4, R5)

- [ ] **Who:** main session dispatches a fresh `spec-reader`-style read-only agent with only: "Follow `.claude/skills/chunk/SKILL.md` `resume` for the chunk on the current branch. Do not edit anything. Report the step you would continue from and any uncommitted changes."
- **Verify:** its answer is step 10 (the true state); recorded in `log.md` as pass or fail with its answer. On fail, fix the skill and repeat.
- **Commit:** with step 10.

### Step 10: `lessons-refresh` and `bench-run` skills (R9)

- [ ] **Who:** main session.
- **Files:** `.claude/skills/lessons-refresh/SKILL.md` (spec §4.4 upkeep), `.claude/skills/bench-run/SKILL.md` (the protocol of spec §4.6, citing `cargo xtask bench`'s own battery check and `pmset -g log`).
- **Verify:** a read-only agent given `lessons-refresh` reports `testing-1` as keep with its evidence; recorded in `log.md`.
- **Commit:** `feat(claude): add the lessons-refresh and bench-run skills (M3.15)`

### Step 11: `mutants-run` skill and its trial run (R9)

- [ ] **Who:** builder (scripts), main session (`SKILL.md`, the announcement, the run).
- **Files:** `.claude/skills/mutants-run/SKILL.md`, `.claude/skills/mutants-run/run.sh` (builds and runs the container of §1), `.claude/skills/mutants-run/watchdog.sh` (the watchdog of §1).
- **Test first:** `sh -n` on both scripts; `run.sh --dry-run` prints the `docker` commands without running them.
- **Verify:** tell the maintainer the run is starting; run `run.sh --module random` with the watchdog in the background; check its first results for `TIMEOUT`; on completion, `target/mutants/summary.md` exists. Record the summary table and the peak memory in `log.md`.
- **Commit:** `feat(claude): add the mutants-run skill with a capped container (M3.15)`

### Step 12: Workflow documentation (R1, R2)

- [ ] **Who:** main session.
- **Files:** `docs/workflow.md` (new: spec §4.1 to §4.4 and §4.10, for people and agents); `AGENTS.md` (rewritten, under 80 lines, spec §4.5); `CLAUDE.md` (new: `@AGENTS.md`); `CONTRIBUTING.md` ("How work is organized" links `docs/workflow.md`; "Reviews before a pull request" names the reviewer agents and the simplify pass); `docs/README.md` (`workflow.md`, `lessons/`, the writing rule); `docs/maintainers.md` (the two milestone items of spec §4.11; "Before merging a chunk" says the main session merges on the go-ahead); `.claude/README.md` (one line per entry; how to use `/chunk` and `/grill`); `CHANGELOG.md` (one entry under Changed).
- **Verify:** `wc -l AGENTS.md` under 80; link-check reports 0; `git grep -n 'stop for the maintainer to review both'` prints nothing (the old rule is gone everywhere).
- **Commit:** `docs: describe the agent workflow and rewrite AGENTS.md (M3.15)`

### Step 13: CLAUDE.md audit (R10)

- [ ] **Who:** main session.
- **Change:** run `claude-md-management:claude-md-improver` on `AGENTS.md` and `CLAUDE.md`; apply findings that keep `AGENTS.md` under 80 lines and within spec §4.5; list every finding and its answer in `log.md` for the pull request.
- **Commit:** `docs: apply the CLAUDE.md audit (M3.15)`, or none if nothing changed.

## 3. Final verification

- `cargo xtask ci` passes.
- Stage 5 of the loop: the simplify builder over `main...HEAD`, then the five reviewers (code, security, conditional with "portability" for the hook script and "n/a" for the rest, docs, spec), fixes and fresh re-reviews; results go in the PR's Reviews section, naming the agent files (R11).
- Stage 6: promote `log.md`'s `lesson?` entries through the capture gate.
- Stage 7: PR (title `docs: agent workflow, lessons and chunk folders (M3.15)`), close-out commit, merge after the go-ahead (R15, recorded in `log.md`), then delete or reduce the migrated memory files (spec §4.8).
- The spec's §9 list, ticked with evidence.

## 4. Lessons

None exist yet: `docs/lessons/` is created in step 7. The rules this chunk moves out of local memory (the `bounded` helpers, the capped container, the benchmark protocol) are followed as written in the spec.

## 5. Review focus

Inputs no step's tests cover that are most likely to bite:

1. **A push the guard parses wrongly in a real session**, such as a command with a heredoc or a subshell (`(cd r && git push origin main)`). Step 5 adds a test for the subshell form: blocked.
2. **The guard on Windows**, where the hook runs under Git Bash and `$CLAUDE_PROJECT_DIR` has a drive letter. The conditional reviewer checks the hook command's quoting; no Windows trial is possible here, and the PR says so.
3. **`/chunk resume` with a dirty tree or a half-written step.** Step 9 runs on a clean tree; step 8's skill trial adds an uncommitted scratch change and checks the skill reports it before acting.
4. **A builder that edits files outside its step.** The main session checks `git status` against the step's file list before each commit; the `4-work` stage file says so.
5. **Stale links after the move in files that are not Markdown** (Rust doc comments, TOML comments). Step 2's `git grep` covers every tracked file, not only Markdown.

## 6. Deviations

<!-- Filled in during implementation. -->
