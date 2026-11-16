---
applies_to:
  - "libs/engine/src/**"
  - "bench/compare/**"
  - "libs/engine/tests/collections_allocations.rs"
---
# Lessons: performance and measurement

- Status: Living (2026-11-16)

- **perf-1.** `#[inline]` is a hint: a hot method that also contains rare work (growth, a climb through parent layers, panic formatting) can grow past LLVM's inlining budget, and then every call pays a full prologue. Keep the hot path tiny and move rare work into `#[cold]` or `#[inline(never)]` functions; check the result with `--emit=asm`, not by reading the source. *Why:* `HierarchicalBitSet::insert` was not inlined (a call with a 6-register, 80-byte prologue per insert) and ran 1.2 to 2.4 times `hibitset`'s time; moving the parent climb out of line made it 2.4 to 2.8 times faster (M3.17). *From:* M3.17 · *Confirmed:* 2026-11-16
- **perf-2.** `dhat`'s peak (`max_bytes`) cannot be reset within a process, and a `realloc` is recorded as a change of size only. Build a measurement's inputs before starting the profiler (dhat ignores blocks it never tracked), and run each case in its own process. *Why:* the M3.17 space report started the profiler before generating its inputs, so the peak of three rows was the input generator's, not the structure's (code review). *From:* M3.17 · *Confirmed:* 2026-11-16
- **perf-3.** Before comparing against a baseline's presized constructor, check what it does in setup: one that writes its zeros there (as `hibitset::BitSet::with_capacity` does) gets warm, faulted-in pages, while one that only reserves pays the zero-fill and page faults inside the timed loop. Say so next to the numbers. *Why:* it flattered `hibitset` in M3.17's sparse insert cases. *From:* M3.17 · *Confirmed:* 2026-11-16
