# .claude/

Status: Living (2026-11-15).

Claude Code configuration shared by everyone who works on this repository: the agent roles, skills and the push guard that run the chunk loop of [docs/workflow.md](../docs/workflow.md). Everything here is plain Markdown or shell, readable without Claude Code.

## When this applies

- You run a chunk with Claude Code: `/chunk next` starts the next one on the roadmap, `/chunk resume` continues one, `/grill <doc>` interrogates a design document.
- You change how agents work here: a role, a skill, a permission or a hook.

## Contents

- `agents/`: one file per role, each pinning its model; the table is in [docs/workflow.md](../docs/workflow.md#roles).
- `skills/chunk/`: the loop. `SKILL.md` dispatches `next`, `resume` and single stages; `stages/` holds one file per stage, so only the current one is read.
- `skills/grill/`: the grill stage, usable on any design document.
- `skills/lessons-refresh/`: the milestone pass over [docs/lessons/](../docs/lessons/).
- `skills/bench-run/`: how to take wall-clock benchmarks on a laptop that can be trusted.
- `skills/mutants-run/`: mutation testing in a memory-capped container, with its scripts.
- `rules/`: one pointer per area (`libs/engine/`, `bench/`, `xtask/`, `docs/`, ...). Claude Code loads a rule when it touches a file matching the rule's `paths:`; the rule names the README to read and says to follow the matching lessons. Rules hold no rule text of their own: the README or `CONTRIBUTING.md` does.
- `hooks/guard-push.sh`: blocks agent pushes to `main` and force pushes; the logic is `cargo xtask guard-push`.
- `settings.json`: the hook, and an allowlist of check commands whose flags `xtask` itself restricts (no bare `cargo test *`: cargo's `--config` and `--manifest-path` flags can run any program). No model is set, so each contributor keeps their own.
- `README.md`: this file.

Local state is ignored by git: `settings.local.json` (personal overrides), `worktrees/`, `agent-memory-local/`.

## How to

- **Change a role or skill:** edit the Markdown in a chunk's pull request; lessons that change how agents work may be edited the same way and are listed in its Lessons section.
- **New agent files** are loaded when a session starts. Until then, dispatch a general-purpose agent with the file's model and "follow `.claude/agents/<role>.md`" as its prompt.
