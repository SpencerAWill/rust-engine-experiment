---
applies_to:
  - ".claude/**"
  - "xtask/src/guard_push.rs"
  - ".gitattributes"
---
# Lessons: agent tooling (hooks, permissions, skills, container scripts)

- Status: Living (2026-11-15)

- **tooling-1.** A Claude Code hook blocks only by exiting 2, so its checker's own failure (a broken build, a missing tool) must map to 2 for risky input and to 0 for the rest: fail closed on the narrowest signal, not on anything the parser cannot read. Test it with real agent commands: multi-line scripts, heredocs and prose, `2>&1`, `| tail`, comments with apostrophes. *Why:* the push guard first blocked an ordinary commit (a heredoc mentioning `git push`), then let every push through while xtask failed to build (M3.15). *From:* M3.15 · *Confirmed:* 2026-11-15
- **tooling-2.** A hook that runs `cargo` builds in its own target directory nested under any existing one (`${CARGO_TARGET_DIR:-target}/<name>`) and sets an explicit `timeout`. *Why:* sharing the default target dir made the hook wait 37 s behind a concurrent build, and an absolute path inside the repository is shared by the host and the devcontainer (M3.15). *From:* M3.15 · *Confirmed:* 2026-11-15
- **tooling-3.** A `Bash(<tool> *)` allow rule accepts every flag of the tool; for cargo, `--config` (a `runner`) and `--manifest-path` make any allowed subcommand run any program. Allow only wrappers that reject unknown flags (`cargo xtask ...`), or exact command lines. *Why:* `cargo test *` was allowed as "read-only" until the security review ran a program through it (M3.15). *From:* M3.15 · *Confirmed:* 2026-11-15
- **tooling-4.** A guard that reads command lines catches habits, not intent: abbreviated flags, `-c` settings, aliases and stored config route around it. Put the real barrier on the server (the `main` ruleset) and stop hardening the hook when reviews only find deliberate evasions. *Why:* three review rounds kept finding new shell forms (M3.15). *From:* M3.15 · *Confirmed:* 2026-11-15
- **tooling-5.** A POSIX `sh` script's `trap` does not run while a foreground child runs; to clean up a long `docker run`, start it in the background with `--init`, then `wait`. *Why:* the mutants script's trap left the container running when the script was killed (M3.15). *From:* M3.15 · *Confirmed:* 2026-11-15
- **tooling-6.** Every script a tool runs keeps LF line endings (`.gitattributes`); a script type not matched by `*.sh` needs its own rule. *Why:* a CRLF checkout on Windows turns a hook into a syntax error, which exits 2 and blocks every Bash call (M3.15). *Deviate when:* the file is never run by `sh`. *From:* M3.15 · *Confirmed:* 2026-11-15
- **tooling-7.** Trial a new or changed skill before relying on it: a fresh agent follows it read-only against a real document or state and reports what it had to guess. *Why:* every skill trial in M3.15 found gaps that writing the skill had not (resume on a dirty tree, grill on an accepted spec, the power-log filter). *From:* M3.15 · *Confirmed:* 2026-11-15
