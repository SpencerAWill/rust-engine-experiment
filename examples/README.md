# examples/

Status: Living (2026-11-15). Programs that use `engine` the way a game would, and fix the workloads that later milestones are measured against.

## When this applies

- You change an example, or an engine API that one uses.
- You need a workload to measure or a checksum to compare against.

Examples are workspace packages: they follow the workspace lints and `#![forbid(unsafe_code)]`, and their dependencies (such as `winit`) are never dependencies of `libs/`.

## Contents

- `fixed_step_sim/`: a headless, deterministic particle simulation using `genid`, `random`, `time` and `math` together; its checksum is the same on every architecture (`cargo run -p fixed_step_sim`).
- `agents_2d/`: 10,000 2D agents in a `winit` and `softbuffer` window, the plain-`Vec` baseline for the M3 storage decision, with an order-independent checksum; its rules are in the [M3.0 spec](../docs/chunks/m3.0-windowed-baseline/spec.md) (`cargo run -p agents_2d --release`, or `-- --headless`).
