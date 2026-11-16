---
applies_to:
  - "AGENTS.md"
  - "CONTRIBUTING.md"
  - "docs/**"
  - ".claude/**"
---
# Lessons: documentation

- Status: Living (2026-11-15)

- **docs-1.** A rule that several entry files need (the waits, the escalations) has one copy; the other files link to it. *Why:* the escalation list was written three times with three wordings within one chunk (M3.15). *From:* M3.15 · *Confirmed:* 2026-11-15
- **docs-2.** A header field or section a stage tells agents to add belongs in the template they copy (`docs/chunks/_template/`), so its format has one copy; when a fix changes how many items a document lists, grep the plan for the old count. *Why:* chunk stage 1 told agents to add an `**Issue:**` line the template lacked, and plan step 6 still said "the two built-in workflows" after the setup grew to four (M3.18). *From:* M3.18 · *Confirmed:* 2026-11-16
