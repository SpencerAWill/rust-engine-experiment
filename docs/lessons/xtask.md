---
applies_to:
  - "xtask/src/**"
---
# Lessons: xtask commands

- Status: Living (2026-11-15)

- **xtask-1.** A command that reads the repository tree finds the root itself (`git rev-parse --show-toplevel`) and runs git and reads files from there. *Why:* `cargo xtask` keeps the caller's current directory, so `check-docs` run from `xtask/` reported 21 false problems, telling the reader to add READMEs that existed (M3.16). *Retire when:* every tree-reading command shares one root helper. *From:* M3.16 · *Confirmed:* 2026-11-15
- **xtask-2.** Test a check that parses text files against a CRLF copy of the tree (clone into the scratchpad, convert every tracked text file, run the check and its tests there). *Why:* Windows checkouts with `core.autocrlf` give CRLF files and `.gitattributes` pins only scripts; the M3.16 portability review found `check-docs` correct this way in minutes (M3.16). *From:* M3.16 · *Confirmed:* 2026-11-15
