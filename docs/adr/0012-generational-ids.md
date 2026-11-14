# ADR 0012: Generational IDs: one lean allocator, the name `GenId`, and an untyped core

Status: Draft (2026-11-14). Supersedes [ADR 0010](0010-two-handle-allocators.md) once accepted. Specified in [M3.11](../design/m3-ecs-storage/m3.11-genid/spec.md).

## Context

`engine_handle` provides a typed generational identifier, `Handle<T>`: a slot index and the slot's generation, checked on use. It also provides a dense pool of values (`HandlePool<T>`) and, after ADR 0010, two ID-only allocators: the lean `HandleAllocator<T>` and the iterable `DenseHandleAllocator<T>`.

Three problems have come up since then.

- **The iterable allocator has no caller.** Outside its own tests, benches and fuzz target nothing uses `DenseHandleAllocator`. ADR 0010 describes it as a `HandlePool<()>` plus `iter` and `retain`, and `HandlePool<()>` already provides both. `Vec<()>` never allocates, so the empty values array costs nothing. Meanwhile the allocator carries a fuzz target, a model test, benchmarks, mutation-triage rows and its own docs. "Dense" also means two different things: contiguous values in the pool, but "iterable" here (#34).
- **The name says the wrong thing.** "Handle" suggests something that points to or owns a resource, and this type does neither. Readers coming from Bevy expect `Handle<T>` to be a reference-counted asset handle. An asset system here will likely want that kind of owning handle later, and "handle" is the natural name for it (#39).
- **Type-erased code has no type.** Code without a compile-time `T` is coming: runtime-registered and mod-defined components (M3.6), the registry (M4), serialisation (M5) and the C ABI (M8). Today it would pass a bare `u64` from `to_bits`, which has no accessors, no readable `Debug` and no `Option` niche.

The crates are `0.x`, `publish = false`, and nothing outside the repository uses them, so a breaking change costs only the repository's own edits.

## Decision

1. **One ID-only allocator.** `DenseHandleAllocator` is removed. Callers who need to iterate live IDs use `GenIdMap<()>`. The allocator and `GenIdMap` keep sharing the private, non-generic slot table, and a model test still requires them to issue identical IDs for the same operations.
2. **Rename to `GenId`** ("generational ID"). The name follows the Rust convention that acronyms and contractions are one word in UpperCamelCase.

   | Before | After |
   |---|---|
   | crate `engine_handle` | `engine_genid` |
   | `engine::handle` | `engine::genid` |
   | `Handle<T>` | `GenId<T>` |
   | `HandlePool<T>` | `GenIdMap<T>` |
   | `HandleAllocator<T>` | `GenIdAllocator<T>` |
   | `InvalidHandleBits` | `InvalidGenIdBits` |
   | `DenseHandleAllocator<T>`, `DenseAllocIter` | removed; use `GenIdMap<()>` |
   | `Handle::to_bits` (frozen) | `UntypedGenId::to_bits` (frozen), with `GenId::to_bits` forwarding to it |

   `HandlePool` becomes `GenIdMap` because it maps each `GenId<T>` it issued to a `T`, as slotmap's `SlotMap` does. "Pool" suggests a preallocated set of reused objects. "Vec" invites reading it as a list of IDs, or asking why it is not a plain `Vec`. That its values are contiguous is a performance property, documented on the type. A future side table keyed by IDs issued elsewhere can take a longer name, like slotmap's `SecondaryMap`.

   No aliases are kept for the old names.
3. **Split out an untyped core.** `UntypedGenId { index: u32, generation: NonZeroU32 }` holds the index, the generation, the bit encoding and the comparison traits. `GenId<T>` is a `#[repr(transparent)]` wrapper over it with `PhantomData<fn() -> T>`. Conversions are explicit at both ends: `id.untyped()` and `GenId::<T>::from_untyped(u)`. There is still no `cast::<U>()`, and no `Deref` or `From` conversion in either direction. The containers accept only typed IDs.
4. **The frozen format is unchanged:** `(index << 32) | generation`, never 0. Only the method that defines it moves, to `UntypedGenId::to_bits`. The golden values stay as they are.

## Consequences

- One fewer public type, fuzz target and set of benchmarks to maintain. Iterating IDs without values uses `GenIdMap<()>`, which pays for a dense array of IDs as `DenseHandleAllocator` did.
- "Handle" is free for a future owning asset handle.
- Type-erased code gets `UntypedGenId`, at no runtime cost: both types are 8 bytes, `Option` of either is 8 bytes, and the methods are the same `const fn`s.
- The split does not reduce monomorphisation much, despite what #39 expected. The shared slot table was already non-generic, and only the thin typed wrappers are compiled per `T`. The split is justified by the type-erased API, not by compile time.
- Documents written before this ADR (the M0+M1 spec, the M2 and earlier M3 specs and plans, ADRs 0003, 0010 and 0011, `benchmarks.md` and `mutation-triage.md`) keep the old names. The table above maps them.
- Rejected alternatives: keeping `Handle`; `Id<T>` (too generic, collides with `egui::Id`); `Gid<T>` (read as a Unix group ID); `RawGenId` for the untyped type ("raw" does not say what is missing); `GenIdPool`, `GenIdVec`, `GenVec`, `GenIdStore` and `GenArena` for the value container; keeping `u64` bits for type-erased code; deprecated aliases; renaming the dense allocator instead of removing it. The reasons are in the [M3.11 spec](../design/m3-ecs-storage/m3.11-genid/spec.md) §7.
- Still open: whether M4's world needs `GenIdAllocator` at all; 32-bit or configurable layouts (#39, "Future"); whether the crate later merges into a coarser core crate (#40).
