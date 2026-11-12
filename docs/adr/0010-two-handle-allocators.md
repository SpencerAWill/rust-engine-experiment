# ADR 0010: Two handle allocators, lean and dense

Status: Accepted (2026-11-12)

## Context

`HandleAllocator` was a wrapper around `HandlePool<()>`. Benchmarks showed it 1.3-3.1x slower than `slotmap<()>`: besides the slot array it maintained a dense array of live handles (and a slot-to-dense link), which is what makes iteration fast but doubles the memory traffic of every allocate and free. The values array costs nothing, because `Vec<()>` never allocates.

Some callers want only generation-checked IDs (their data lives elsewhere), and some also want to iterate the live IDs. One type cannot be best at both.

## Decision

- `HandleAllocator<T>` is the lean allocator: one 8-byte slot per slot ever used and a free list. It has `allocate`, `free`, `is_alive`, `len` and `clear`, and no iteration.
- `DenseHandleAllocator<T>` is the previous implementation (a `HandlePool<()>`): it adds `iter` and `retain` at the cost of the dense array.
- Both, and `HandlePool`, share one private `SlotTable` (generations, LIFO free list, retirement on exhaustion, the slot limit), so the rules cannot drift apart. A model test requires the lean and dense allocators to issue identical handles for the same operations.

## Consequences

- Callers choose by need: IDs only, IDs plus iteration, or values (`HandlePool`).
- The rename of the old `HandleAllocator` to `DenseHandleAllocator` is a breaking change, accepted because every crate is `0.x` and unstable.
- A policy-level alternative was a single type with a mode parameter. It was rejected: it makes one name carry two APIs and noisier signatures.
- Further shrinking the slot (for example a separate free stack so a slot needs only a generation) was considered and not done: it would cost a 31-bit generation or a second array, and the measured gap to `slotmap` is already small.
