---
name: lessons-refresh
description: Check every lesson in docs/lessons/ against the current code and keep, edit, merge or delete each one. Use at each milestone boundary (docs/maintainers.md) or when lessons look stale.
---

# Lessons refresh

Lessons go stale as the code changes. This pass keeps `docs/lessons/` short and true. Format and rules: [docs/lessons/README.md](../../../docs/lessons/README.md).

Work on a branch (`chore/lessons-refresh-<date>`); the result is one small pull request.

For each lesson file:

1. Check each `applies_to` pattern still matches tracked files (`git ls-files '<pattern>'`). Fix or remove patterns that match nothing.
2. For each lesson, look for evidence in the code and history that it still holds (the code it names exists, the failure it prevents is still possible). Then:
   - **Keep:** the code it names exists and nothing contradicts it; set *Confirmed* to today.
   - **Code disagrees:** the lesson is right but existing code breaks it. Narrow the lesson to what is true (for example, "new tests"), and open an issue to bring the code in line; set *Confirmed*.
   - **Edit:** the lesson is right but its details moved; fix them and set *Confirmed* (any outcome that checked the evidence sets it).
   - **Merge:** two lessons say the same thing; keep the older ID, fold the other in, delete it.
   - **Replace with a check:** a test, lint or `xtask` check now enforces it (or could, cheaply: add it through a builder); delete the lesson.
   - **Delete:** its *Retire when* happened, or it no longer applies. Git history is the archive.
3. Edit single bullets; never regenerate a file. Never reuse a deleted ID.

Update `docs/lessons/README.md`'s Contents if a file was added or removed. The pull request lists every lesson by ID with its outcome.
