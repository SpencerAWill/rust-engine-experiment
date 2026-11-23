---
applies_to:
  - "libs/engine/src/**"
  - "bench/compare/**"
  - "libs/engine/tests/collections_allocations.rs"
  - "**/Cargo.toml"
  - "xtask/src/main.rs"
---
# Lessons: performance and measurement

- Status: Living (2026-11-16)

- **perf-1.** `#[inline]` is a hint: a hot method that also contains rare work (growth, a climb through parent layers, panic formatting) can grow past LLVM's inlining budget, and then every call pays a full prologue. Keep the hot path tiny and move rare work into `#[cold]` or `#[inline(never)]` functions; check the result with `--emit=asm`, not by reading the source. *Why:* `HierarchicalBitSet::insert` was not inlined (a call with a 6-register, 80-byte prologue per insert) and ran 1.2 to 2.4 times `hibitset`'s time; moving the parent climb out of line made it 2.4 to 2.8 times faster (M3.17). *From:* M3.17 · *Confirmed:* 2026-11-16
- **perf-2.** `dhat`'s peak (`max_bytes`) cannot be reset within a process, and a `realloc` is recorded as a change of size only. Build a measurement's inputs before starting the profiler (dhat ignores blocks it never tracked), and run each case in its own process. *Why:* the M3.17 space report started the profiler before generating its inputs, so the peak of three rows was the input generator's, not the structure's (code review). *From:* M3.17 · *Confirmed:* 2026-11-16
- **perf-3.** Before comparing against a baseline's presized constructor, check what it does in setup: one that writes its zeros there (as `hibitset::BitSet::with_capacity` does) gets warm, faulted-in pages, while one that only reserves pays the zero-fill and page faults inside the timed loop. Say so next to the numbers. *Why:* it flattered `hibitset` in M3.17's sparse insert cases. *From:* M3.17 · *Confirmed:* 2026-11-16
- **perf-4.** A dependency added to any workspace member can change the features, and so the code, of crates in every `cargo bench --workspace` build, not only in crates that use it (Cargo unifies features over the selected packages). Before comparing a run with a baseline saved earlier, diff `cargo tree -e features,normal,dev -f '{p} [{f}]'` over that command's package selection on base and head, for each CI target, name the crate that turns each feature on, and scope any "same features" claim to what was diffed; keep members a benchmark does not need out of its selection. *Why:* M3.10's `hecs` turned on `hashbrown`'s `inline-more` and `default-hasher` under `engine::collections` in `cargo xtask bench`'s `--workspace` build of the `libs/` benchmarks, which no library benchmark depends on; the first caveat named the wrong crate, and the fixed one first listed macOS's features only (performance reviews). *From:* M3.10 · *Confirmed:* 2026-11-23
