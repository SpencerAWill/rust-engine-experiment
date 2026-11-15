---
applies_to:
  - "libs/engine/src/collections/**"
---
# Lessons: testing

- Status: Living (2026-11-15)

- **testing-1.** A new test that walks an iterator or formats the `Debug` output of code under test goes through `bounded` / `debug_bounded` (`libs/engine/src/collections/tests/mod.rs`), not a bare `collect` or `format!("{:?}")`. *Why:* under mutation testing a mutant made `next` return `Some(0)` forever, and tests that collected it grew memory until the host had to be force-quit (M3.14). *Deviate when:* the iterator is a std type, or in `libs/engine/tests/`, which cannot reach the crate-private helpers (use `.take(n)` there). Older tests in `index_map.rs`, `index_set.rs` and `tests/collections_contracts.rs` predate it and still collect or format unbounded. *From:* M3.14 · *Confirmed:* 2026-11-15
