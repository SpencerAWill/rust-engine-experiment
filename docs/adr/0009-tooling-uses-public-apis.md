# ADR 0009: Tooling uses only public APIs

Status: Accepted (2026-11-12)

## Context

A game editor is a planned product, leaning out-of-process (it talks to a running engine over a protocol). Inspectors, profilers, and remote protocols will also exist. If the engine's own tools get privileged access, third-party tool builders end up with a second-class, incomplete API.

## Decision

Nothing the engine's own tools need (inspectors, editor, remote protocol, profilers) may require private access. If a tool needs it, it becomes a designed, documented public API. In particular, `#[doc(hidden)] pub` back doors are not allowed.

## Consequences

- The public API stays complete and honest for third-party tool builders.
- Crate boundaries cannot hide shared internals behind private escape hatches, which reinforces the "keep in one crate" rules of ADR 0004.
- Debuggability features (`Debug` output, invariant checks, `#[track_caller]` panics) are designed as public surface from the start.
