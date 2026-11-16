---
applies_to:
  - "libs/engine/src/collections/**"
---
# Lessons: testing

- Status: Living (2026-11-15)

- **testing-1.** A new test that walks an iterator or formats the `Debug` output of code under test goes through `bounded` / `debug_bounded` (`libs/engine/src/collections/tests/mod.rs`), not a bare `collect` or `format!("{:?}")`. *Why:* under mutation testing a mutant made `next` return `Some(0)` forever, and tests that collected it grew memory until the host had to be force-quit (M3.14). *Deviate when:* the iterator is a std type, or in `libs/engine/tests/`, which cannot reach the crate-private helpers (use `.take(n)` there). Older tests in `index_map.rs`, `index_set.rs` and `tests/collections_contracts.rs` predate it and still collect or format unbounded. *From:* M3.14 · *Confirmed:* 2026-11-16
- **testing-2.** A panic-message test asserts the qualified operation name (`"HierarchicalBitSet::extend:"`), not the bare one: the bare name is usually a substring of the `try_` twin the same message names, so the check passes for a mislabelled message. *Why:* four M3.17 panic tests checked `"extend"` and `"ensure_capacity"` and would have passed with the wrong operation named (re-review). *From:* M3.17 · *Confirmed:* 2026-11-16
- **testing-3.** A container that grows several buffers (layers, pages) reports `capacity()` as the bound across all of them, and its test checks every buffer's `Vec::capacity()` before and after inserting at `capacity() - 1`. *Why:* `HierarchicalBitSet::capacity()` was computed from layer 0 alone, so an insert below it grew layer 1; `with_capacity` reserved every layer, so the `dhat` checks passed (M3.17 review). *From:* M3.17 · *Confirmed:* 2026-11-16
