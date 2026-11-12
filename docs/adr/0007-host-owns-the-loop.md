# ADR 0007: The host owns the loop

Status: Accepted (2026-11-12)

## Context

The engine will be embedded in Rust games, C# applications (through the C ABI), editors, servers, tests, and browsers. Each of these already has a main loop, a thread model, or an event pump. An engine that insists on owning the loop cannot be embedded in most of them.

## Decision

The engine is library-first. It never takes over a thread it was not given. Hosts call into the engine, for example `step(...)`, and supply time, randomness, and input as explicit parameters. A convenience "app runner" may be layered on top later, but nothing in the engine may require it.

M1 reflects this directly: `FixedTimestep` is a pure accumulator that the host feeds elapsed time, and `examples/fixed_step_sim` drives the simulation from its own `main`.

## Consequences

- The same core runs in a headless test, a server tick, a C# game loop, and a browser frame callback.
- Determinism is easier to guarantee, because the host controls what time and input the simulation sees (ADR 0003).
- The engine cannot assume frame pacing or thread ownership, and any scheduling convenience must be optional.
