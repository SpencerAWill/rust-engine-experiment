---
applies_to:
  - "docs/chunks/**"
---
# Lessons: writing specs and plans

- Status: Living (2026-11-15)

- **specs-1.** A §8 check written as "finds nothing" names the paths it covers. *Why:* `git grep 'design/m[0-9]'` also matched historical records that quote the old paths on purpose, so the check could never pass as written (M3.15). *From:* M3.15 · *Confirmed:* 2026-11-15
- **specs-2.** Evidence that §8 says will be "noted in `log.md`" is its own plan step, or it is forgotten. *Why:* M3.15's R7 follow-up note was missing until the spec review asked for it. *From:* M3.15 · *Confirmed:* 2026-11-15
