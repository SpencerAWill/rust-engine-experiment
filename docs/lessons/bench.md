---
applies_to:
  - "xtask/src/bench_regress.rs"
  - "libs/engine/src/**"
  - "bench/regress/**"
---
# Lessons: the instruction-count gate

- Status: Living (2026-11-15)

- **bench-1.** `bench_regress::is_relevant` treats every `.md` path as unable to change an instruction count. Before code under `libs/` or `bench/regress/` pulls in a Markdown file (`include_str!`, `include_bytes!`, `#[doc = include_str!(...)]`), make the gate keep that file. *Why:* otherwise a change to the included file changes the build but skips the comparison, and nothing reports it (M3.16 security review). *From:* M3.16 · *Confirmed:* 2026-11-15
