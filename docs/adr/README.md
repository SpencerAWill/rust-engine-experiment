# Architecture decision records

Status: Living (2026-11-15). One short file per decision that would be expensive to reverse: its context, the decision and its consequences.

## When this applies

- A change would touch one of the decisions below, or make a new decision that is hard to reverse: it needs an ADR (or one that supersedes an old one) in the same pull request.
- You want to know why the engine works the way it does.

Accepted ADRs are not rewritten to change the decision; a new ADR supersedes the old one ([conventions](../README.md#conventions)). ADRs up to 0011 use the milestone numbers from before 2026-11-12 ([roadmap](../roadmap.md#milestone-numbering-note)).

## Contents

| ADR | Decision | Status |
|---|---|---|
| [`0001-unsafe-policy.md`](0001-unsafe-policy.md) | No `unsafe` outside allowlisted crates | Accepted |
| [`0002-originality-and-dependency-licensing.md`](0002-originality-and-dependency-licensing.md) | No copied code; allowed dependency licences | Accepted |
| [`0003-same-build-determinism.md`](0003-same-build-determinism.md) | Same build, same platform, same results; transcendental backends | Accepted, rewritten by M3.13 |
| [`0004-crate-splitting-rule.md`](0004-crate-splitting-rule.md) | One library crate; when to split one off | Accepted, rewritten by M3.13 |
| [`0005-coordinate-system-and-units.md`](0005-coordinate-system-and-units.md) | Coordinate system and units | Accepted |
| [`0006-large-world-precision.md`](0006-large-world-precision.md) | Large-world precision | Accepted |
| [`0007-host-owns-the-loop.md`](0007-host-owns-the-loop.md) | The host owns the loop | Accepted |
| [`0008-no-global-state.md`](0008-no-global-state.md) | No global mutable state | Accepted |
| [`0009-tooling-uses-public-apis.md`](0009-tooling-uses-public-apis.md) | Tooling uses only public APIs | Accepted |
| [`0010-two-handle-allocators.md`](0010-two-handle-allocators.md) | Two handle allocators | Superseded by 0012 |
| [`0011-heap-allocation-control.md`](0011-heap-allocation-control.md) | Controlling heap allocation | Accepted |
| [`0012-generational-ids.md`](0012-generational-ids.md) | Generational IDs: one lean allocator, `GenId`, an untyped core | Accepted |
