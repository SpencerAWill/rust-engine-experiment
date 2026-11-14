# Data structures for a game engine: research catalog

Status: Draft (2026-11-14). External research, not a decision. Nothing here is scheduled, and listing an entry does not mean the engine will have it.

This catalog collects the data structures that game engines use, their common variants, and recent research that could be useful, so that the work can be tracked. The tracking issue, [#38](https://github.com/SpencerAWill/rust-engine-experiment/issues/38), has one checkbox per entry. Implementation work starts as a sub-issue of it and, once scheduled, becomes a roadmap chunk with its own spec ([roadmap](../roadmap.md), [CONTRIBUTING.md](../../CONTRIBUTING.md)).

Read an entry's sources before a spec relies on it. Most sources are papers, engine documentation and practitioner posts. Performance numbers are the authors' own, and nothing here was measured in this repository. Following [ADR 0002](../adr/0002-originality-and-dependency-licensing.md), sources are for learning designs; no code is copied from them.

## How to read an entry

Each entry has these fields:

- **Uses:** what it is for in an engine.
- **Notes:** properties, trade-offs, and research results.
- **Variants:** named versions of the structure that change how it behaves.
- **Repo fit:** one or more of the tags below.
- **Determinism:** what a spec has to define so the structure keeps the guarantee in [ADR 0003](../adr/0003-same-build-determinism.md): same build, same platform, same operations, same results. Engine code also may not use `HashMap`, `HashSet`, wall-clock time or global state.
- **Builds on:** other entries it is usually built from.
- **Sources:** links. "None checked yet" means the entry comes from general knowledge and needs a source before a spec cites it.

Repo fit tags:

| Tag | Meaning |
|---|---|
| `safe` | Can be written under the current rules: safe Rust, CPU only, no global state. It could go in an existing crate or a new one ([ADR 0004](../adr/0004-crate-splitting-rule.md)). |
| `unsafe` | Needs `unsafe` (an [ADR 0001](../adr/0001-unsafe-policy.md) allowlist entry and its own ADR) or an external crate that uses it. |
| `os` | Needs operating-system calls such as virtual-memory mapping. Implies `unsafe`. |
| `gpu` | Runs on the GPU, so it waits for a renderer. |
| `subsystem` | Specific to a later subsystem (rendering, physics, audio, AI, networking, editor), so it belongs in that subsystem rather than in a general collection. |
| `theory` | A research result with no practical implementation path yet. Tracked so it is not forgotten. |
| `in repo` | Already exists, or a roadmap chunk covers it. |

Section numbers are stable. New sections go at the end, and new entries go at the end of their section. Links point to entry headings, so do not rename a heading without updating the tracking issue.

## 1. Variants: common modifications

These patterns recur across many entries. They are not tracked items; each entry lists the ones that apply to it.

| Variant | What it adds | Examples |
|---|---|---|
| Generation or version tag | Detects stale references | [Slot map](#slot-map) |
| Paging or chunking | Elements never move; it grows without copying; empty pages cost nothing | [Sparse set](#sparse-set), [Hive](#hive), [SPGrid](#spgrid) |
| Dense plus sparse split | Packed iteration with O(1) lookup | [Sparse set](#sparse-set), [Handle pool](#handle-pool) |
| Hierarchical summaries | Skips empty ranges; answers aggregate queries quickly | [Hierarchical bitset](#hierarchical-bitset), [Hierarchical Z-buffer](#hierarchical-z-buffer) |
| Augmented (monoid) summaries | Seek by any measure (bytes, lines, weight, time) | [Sum tree](#sum-tree), [Fenwick tree](#fenwick-tree), [Segment tree](#segment-tree) |
| Implicit (pointer-free) layout | Positions computed by index arithmetic; cache-friendly | [Eytzinger layout](#eytzinger-layout), [Static S-tree](#static-s-tree), [Binary and d-ary heap](#binary-and-d-ary-heap) |
| Loose or enlarged bounds | Fewer reinsertions when objects move | [Loose octree](#loose-octree), [Dynamic AABB tree](#dynamic-aabb-tree) |
| Lazy deletion and tombstones | Cheap removal, cleanup later | [Hive](#hive), open addressing in [Robin Hood hashing](#robin-hood-hashing) |
| Struct of arrays and SIMD width | Vectorised processing | [Struct-of-arrays vector](#struct-of-arrays-vector), [Wide and compressed BVH](#wide-and-compressed-bvh), [Swiss table](#swiss-table) |
| Quantisation and compression | Less memory traffic | [Compressed animation clips](#compressed-animation-clips), [Dense Geometry Format](#dense-geometry-format) |
| Deduplication (tree to DAG) | Shares identical subtrees | [Sparse voxel DAG](#sparse-voxel-dag), [HashDAG](#hashdag) |
| Persistence by path copying | Cheap snapshots and undo | [Persistent collections](#persistent-collections) |
| Intrusive links | No allocation per element | [Intrusive linked list](#intrusive-linked-list) |
| Double or triple buffering | Readers and writers do not block each other | [Double-buffered frame allocator](#double-buffered-frame-allocator), [Triple buffer](#triple-buffer) |
| Sharding per thread | No contention | [Slab and size-class allocator](#slab-and-size-class-allocator) |
| Lock-free or wait-free version | Progress guarantees under concurrency | Section 16 |
| GPU-parallel version | Built with atomics and prefix sums | Section 17 |
| Deterministic version | Results depend only on the operations applied | [Insertion-ordered map](#insertion-ordered-map) |
| Learned version | A model predicts where a key is | [Learned index](#learned-index) |

## 2. Memory and allocation

#### Bump arena

- **Uses:** scratch memory for one frame or one job, cleared all at once.
- **Notes:** an allocation only moves a pointer forward. AAA engines commonly pair a per-frame bump or stack arena for short-lived data with a general heap for long-lived data.
- **Variants:** bumping downward rather than upward; growth backed by virtual memory (see [Virtual memory arena](#virtual-memory-arena)); one arena per thread.
- **Repo fit:** `safe` as a typed index arena ([Typed index arena](#typed-index-arena)); `unsafe` for raw memory.
- **Determinism:** handing out raw addresses is fine only if nothing ever reads them back as data. Indices are deterministic.
- **Builds on:** nothing.
- **Sources:** [Frechette, stack frame allocators](https://nfrechette.github.io/2016/10/17/vmem_stack_frame_allocator), [framealloc technical notes](https://docs.rs/crate/framealloc/0.11.0/source/TECHNICAL.md).

#### Stack allocator with markers

- **Uses:** allocations whose lifetime follows a code scope.
- **Notes:** you roll back to a saved marker instead of freeing each allocation. Mixing allocations and frees out of order fragments it, so it can no longer roll back.
- **Variants:** two stacks growing toward each other from both ends of one buffer; one stack per thread (Unity's thread-local stack allocator).
- **Repo fit:** `safe` (indices) or `unsafe` (raw memory).
- **Determinism:** deterministic as long as markers are restored in nested order.
- **Builds on:** [Bump arena](#bump-arena).
- **Sources:** [Unity native allocators](https://docs.unity.com/en-us/engine/6000.6/manual/analysis/performance-memory/native-memory/allocators), [Frechette, greedy stack frame allocator](https://nfrechette.github.io/2016/05/09/greedy_stack_frame_allocator).

#### Double-buffered frame allocator

- **Uses:** data written by the simulation in one frame and read by the renderer in the next.
- **Notes:** N arenas used in rotation; arena k is cleared when frame k+N starts. Two or three buffers are common for rendering.
- **Variants:** double or triple buffering; the arena is released only after a GPU fence says the GPU is finished with it.
- **Repo fit:** `safe` (indices).
- **Determinism:** clearing must follow frame ticks, not wall-clock time.
- **Builds on:** [Bump arena](#bump-arena).
- **Sources:** [Frechette, stack frame allocators](https://nfrechette.github.io/2016/10/17/vmem_stack_frame_allocator).

#### Pool allocator

- **Uses:** many objects of the same size: particles, nodes, components.
- **Notes:** the free list is stored inside the free slots themselves, so it costs no extra memory. Allocation and free are O(1).
- **Variants:** a fixed-capacity pool or a pool that grows in chunks; free lists kept per thread.
- **Repo fit:** `safe` (index pool); `in repo` in spirit: the slot arrays inside `engine_handle` work this way.
- **Determinism:** the order in which freed slots are reused must be defined (LIFO is the usual choice).
- **Builds on:** [Free-list array](#free-list-array).
- **Sources:** [Middleditch, slot map](https://seanmiddleditch.github.io/2013-01-05-data-structures-for-game-developers-the-slot-map).

#### Slab and size-class allocator

- **Uses:** a general-purpose heap.
- **Notes:** objects are sorted into size classes, and each class gets its own pages (slabs). mimalloc shards its free lists by page: each page has its own list for frees on the owning thread and another for frees from other threads, which keeps contention low and the fast path short.
- **Variants:** caches per thread; heaps with security hardening.
- **Repo fit:** `unsafe`. Engines would normally use an existing allocator rather than write one.
- **Determinism:** affects only addresses, which simulation code must never read.
- **Builds on:** [Pool allocator](#pool-allocator).
- **Sources:** [Leijen et al., mimalloc: free list sharding in action (APLAS 2019)](https://unpaywall.org/10.1007%2F978-3-030-34175-6_13).

#### TLSF allocator

- **Uses:** a general heap with a bounded worst case; GPU memory heaps.
- **Notes:** "two-level segregated fit". Allocation and free run in bounded constant time; realloc may fall back to a linear-time copy. Zest uses it for CPU and GPU memory.
- **Variants:** a version that tracks offsets only, without touching the memory (see [Offset allocator](#offset-allocator)).
- **Repo fit:** `safe` if it manages offsets; `unsafe` if it manages raw memory.
- **Determinism:** deterministic for a given sequence of operations.
- **Builds on:** [Bitset](#bitset) (finds a non-empty bin with bit scans).
- **Sources:** [tlsf crate](https://docs.rs/tlsf), [Zest memory](https://peterigz.github.io/zest/concepts/memory).

#### Buddy allocator

- **Uses:** power-of-two blocks; older GPU heaps.
- **Notes:** merging a block with its free neighbour ("buddy") is cheap. Rounding requests up to a power of two wastes up to half of each block.
- **Variants:** binary buddy; tree-based buddy.
- **Repo fit:** `safe` (offsets).
- **Determinism:** deterministic.
- **Builds on:** [Bitset](#bitset).
- **Sources:** none checked yet.

#### Offset allocator

- **Uses:** handing out ranges inside GPU buffers and heaps, or any range of units; allocates offsets, not memory.
- **Notes:** Sebastian Aaltonen's design is hard real-time O(1) with low fragmentation. It has 256 bins on a small floating-point scale (3-bit mantissa, 5-bit exponent), so rounding wastes at most 12.5% (6.25% on average) instead of up to 100% with power-of-two bins. A two-level bitfield finds a free bin with two leading-zero-count instructions. The units need not be bytes. Patrick Walton ported it to Rust for Bevy.
- **Variants:** TLSF-style bins (galloc).
- **Repo fit:** `safe`.
- **Determinism:** deterministic.
- **Builds on:** [TLSF allocator](#tlsf-allocator), [Bitset](#bitset).
- **Sources:** [offset-allocator crate (Rust port)](https://github.com/pcwalton/offset-allocator), [galloc](https://github.com/gogpu/galloc).

#### Upload ring allocator

- **Uses:** streaming per-frame data from CPU to GPU: constants, staging buffers.
- **Notes:** a ring of buffer space whose tail is reclaimed when GPU fences signal.
- **Variants:** one ring per frame in flight.
- **Repo fit:** `gpu`, `subsystem`.
- **Determinism:** does not affect simulation.
- **Builds on:** [Ring buffer](#ring-buffer).
- **Sources:** none checked yet.

#### Virtual memory arena

- **Uses:** arenas that can grow very large and whose elements never move.
- **Notes:** reserves a large address range and maps in physical memory on demand. At the end of a frame it can give memory back in one call while keeping some slack.
- **Variants:** reserve and commit; guard pages.
- **Repo fit:** `os`.
- **Determinism:** addresses must not reach simulation results.
- **Builds on:** [Bump arena](#bump-arena).
- **Sources:** [Frechette, virtual-memory-aware stack frame allocator](https://nfrechette.github.io/2016/10/17/vmem_stack_frame_allocator).

#### Compacting heap with relocatable handles

- **Uses:** sessions that run for a long time and fragment memory: streaming, editors.
- **Notes:** callers hold handles instead of pointers, so the heap can move blocks to defragment.
- **Variants:** compacting incrementally, a little each frame.
- **Repo fit:** `safe` if handle-based.
- **Determinism:** compaction must run at defined points.
- **Builds on:** [Slot map](#slot-map).
- **Sources:** none checked yet.

#### Typed index arena

- **Uses:** the safe-Rust replacement for most raw-memory allocators: a `Vec<T>` addressed by `u32` indices and cleared in bulk.
- **Notes:** fits `forbid(unsafe_code)`. It has no use-after-free check unless the index carries a generation (see [Slot map](#slot-map)).
- **Variants:** one per frame; one per thread.
- **Repo fit:** `safe`.
- **Determinism:** indices come out in order, so they are deterministic.
- **Builds on:** nothing.
- **Sources:** none checked yet.

## 3. Handles, identifiers and object storage

#### Free-list array

- **Uses:** object tables in which freed slots are reused.
- **Notes:** reusing an index makes old references silently point at the new object, which is the reason generations were added (see [Slot map](#slot-map)).
- **Variants:** the free list stored inside the freed slots.
- **Repo fit:** `safe`.
- **Determinism:** the reuse order must be defined.
- **Builds on:** nothing.
- **Sources:** [Middleditch, slot map](https://seanmiddleditch.github.io/2013-01-05-data-structures-for-game-developers-the-slot-map).

#### Slot map

- **Uses:** references to entities, assets and graph nodes that stay valid and are checked on use.
- **Notes:** each index carries a generation, so a stale reference is detected instead of reaching the wrong object. A plain slot map can never shrink, because it must remember the latest generation of every slot.
- **Variants:**
  - Hop slot map: iterates faster when many slots are empty, but inserting and removing are about 2x slower.
  - Dense slot map: iterates fastest, but random access is slower.
  - Secondary map: a `Vec` holding extra data for the keys of a primary map.
  - Retire on overflow (`slotmap-careful`): a slot whose generation would wrap is retired for good, so no handle is ever reused, at the cost of memory that grows without bound under endless churn.
- **Repo fit:** `in repo` (`engine_handle`: `HandleAllocator`, `HandlePool`).
- **Determinism:** handles are a pure function of the operation sequence (already guaranteed in this repo; `Handle::to_bits` is frozen).
- **Builds on:** [Free-list array](#free-list-array).
- **Sources:** [slotmap crate](https://docs.rs/slotmap/1.0.3), [slotmap-careful](https://docs.rs/slotmap-careful/latest/slotmap_careful/), [Middleditch](https://seanmiddleditch.github.io/2013-01-05-data-structures-for-game-developers-the-slot-map).

#### Handle pool

- **Uses:** values packed contiguously for iteration, addressed by handles that stay valid.
- **Notes:** a dense array holds the values; a sparse array maps each handle to its slot and doubles as the free list. Removal swaps the last element into the hole, so iteration order is not stable.
- **Variants:** a dense allocator holding IDs only.
- **Repo fit:** `in repo` (`HandlePool`, `DenseHandleAllocator`).
- **Determinism:** swap-remove order is a function of the operations.
- **Builds on:** [Slot map](#slot-map).
- **Sources:** [Kiah, game engine containers: handle_map](https://gamedev.net/articles/programming/general-and-gameplay-programming/game-engine-containers-handle-map-r4495).

#### Sparse set

- **Uses:** ECS component storage; O(1) membership, insertion and removal for integer keys.
- **Notes:** a sparse array maps keys to positions in a dense array, which is packed for iteration. EnTT splits the sparse array into pages, so key ranges nobody uses cost no memory.
- **Variants:** paged sparse array; sorted dense array; [Owning groups](#owning-groups).
- **Repo fit:** `in repo` (M3.3 prototype).
- **Determinism:** dense order is a function of the operations; sorting the dense array makes it canonical.
- **Builds on:** [Handle pool](#handle-pool).
- **Sources:** [skypjack, ECS back and forth](https://skypjack.github.io/2019-03-21-ecs-baf-part-2-insights).

#### Hive

- **Uses:** objects that are created and destroyed constantly but must keep stable addresses because other systems hold pointers to them (entities referenced by octrees and level structures).
- **Notes:** standardised as `std::hive` in C++26, from `plf::colony`, which was designed for game engines. Elements live in independently allocated blocks whose minimum and maximum sizes you can set. Iteration uses a skip field to jump over erased slots, and erased slots are reused. Insertion and erasure never invalidate references to other elements.
- **Variants:** block-size limits; jump-counting skip fields.
- **Repo fit:** `safe` with indices in place of pointers.
- **Determinism:** which erased slots get reused, and in what order, must be defined.
- **Builds on:** [Chunked stable vector](#chunked-stable-vector).
- **Sources:** [P0447 std::hive](https://open-std.org/jtc1/sc22/wg21/docs/papers/2024/p0447r28.html), [Introducing plf::colony](https://www.gamedeveloper.com/programming/introducing-plf-colony-a-c-data-container-designed-initially-for-games-), [C++26 std::hive](https://sandordargo.com/blog/2026/09/02/cpp26-hive).

#### Chunked stable vector

- **Uses:** a vector that grows without moving existing elements, so references stay valid.
- **Notes:** a list of fixed-size chunks; an element's index is split into a chunk number and an offset.
- **Variants:** chunks that double in size each time (segmented vector).
- **Repo fit:** `safe`.
- **Determinism:** deterministic.
- **Builds on:** nothing.
- **Sources:** none checked yet.

#### Packed array with remap table

- **Uses:** compacting storage after removals while old IDs still resolve.
- **Notes:** a side table maps old positions to new ones after compaction. Used when loading saves and when remapping persistent IDs (roadmap M5).
- **Variants:** remapping in bulk.
- **Repo fit:** `safe`.
- **Determinism:** the remap order must be defined.
- **Builds on:** nothing.
- **Sources:** none checked yet.

#### Packed entity ID

- **Uses:** IDs that cross FFI, the network or save files.
- **Notes:** packs an index, a generation and sometimes other fields into one integer. This repo's `Handle<T>` is a `u32` index plus a `NonZeroU32` generation, encoded as `(index << 32) | generation`. That uses all 64 bits, with a zero generation as the niche that keeps `Option<Handle<T>>` at 8 bytes. Every extra field takes bits from the index (fewer live objects) or the generation (it wraps sooner, so a stale handle can match a reused slot).
- **Variants:** fields other engines pack in:
  - **Kind or type tag:** what the ID names (entity, component, texture, mesh). `Handle<T>` covers this at compile time; a runtime tag matters once handles lose their type, across the C ABI (M8) or in a type-erased registry.
  - **World or pool ID:** which world or allocator issued the ID, so a handle used against the wrong or a recreated world is caught.
  - **Shard or thread ID:** which per-thread allocator created it, so threads can create IDs in parallel without locks. Some engines also split the index into page and slot for paged storage.
  - **Relationship and role flags (Flecs):** the high bits mark roles. A pair flag means the ID is a (relationship, target) pair with both entities' indices packed in, so one integer type names an entity, a component or a pair.
  - **Networking:** a local-or-replicated bit for entities the client predicts; an owner or peer ID, so peers create IDs without colliding.
  - **Persistence:** a persistent-or-transient bit (saved or runtime-only), or instead a separate stable ID remapped on load (M5).
  - **Reserved values:** a null sentinel; a range of placeholder IDs for spawns deferred in command buffers; generations retired when they would wrap.
  - **Common layouts (not checked against current source):** EnTT defaults to 32 bits, about 20 of index and 12 of version, configurable; Bevy's `Entity` is a 32-bit index and a 32-bit non-zero generation; Flecs uses 64 bits, 32 for the index, the generation above it and the top bits for role flags.
- **Repo fit:** `in repo` (`Handle::to_bits`, frozen format). Likely reasons to add a field: untyped handles over FFI (M8), entity relationships (pair encoding), networking (owner or local bit).
- **Determinism:** the bit layout is a frozen format, so changing it needs an ADR and new golden tests.
- **Builds on:** [Slot map](#slot-map).
- **Sources:** [slotmap crate](https://docs.rs/slotmap/1.0.3).

#### Intrusive linked list

- **Uses:** older engines and kernels; lists whose nodes are embedded in the objects themselves.
- **Notes:** no allocation per element, but iteration jumps around memory and is slow.
- **Variants:** singly or doubly linked; index-based links.
- **Repo fit:** `safe` with index links; `unsafe` with pointers.
- **Determinism:** deterministic.
- **Builds on:** nothing.
- **Sources:** [P0447 std::hive (motivation)](https://open-std.org/jtc1/sc22/wg21/docs/papers/2024/p0447r28.html).

## 4. Sequences and buffers

#### Small vector

- **Uses:** short lists that rarely need the heap.
- **Notes:** stores up to N elements inline and moves to the heap when it grows past that.
- **Variants:** Unreal's `TArray` with an inline allocator; `SmallVec`.
- **Repo fit:** `safe` (external crates exist).
- **Determinism:** deterministic.
- **Builds on:** nothing.
- **Sources:** none checked yet.

#### Fixed-capacity vector

- **Uses:** `no_std` code, memory budgets, hot paths that must never allocate.
- **Notes:** capacity is fixed at compile time or at construction; pushing past it fails or reports an error. Relevant to [ADR 0011](../adr/0011-heap-allocation-control.md).
- **Variants:** a vector whose capacity is a const generic; a fixed-capacity string.
- **Repo fit:** `safe`.
- **Determinism:** deterministic.
- **Builds on:** nothing.
- **Sources:** none checked yet.

#### Struct-of-arrays vector

- **Uses:** SIMD-friendly layouts for particles, poses and agents.
- **Notes:** each field gets its own array, so a loop reads only the fields it uses.
- **Variants:** array of structs of arrays (AoSoA, SIMD-width blocks).
- **Repo fit:** `safe`; `in repo` as a spike backend (M3.2's SoA agents).
- **Determinism:** deterministic.
- **Builds on:** nothing.
- **Sources:** none checked yet.

#### Ring buffer

- **Uses:** input history, rewind, logs, audio delay lines, and queues between producer and consumer.
- **Notes:** a fixed-size circular array. Overwriting the oldest entry avoids allocating and shifting elements.
- **Variants:** power-of-two capacity with index masking; overwrite-oldest vs reject-when-full; see also [Bip buffer](#bip-buffer), [Mirrored ring buffer](#mirrored-ring-buffer), [SPSC ring](#spsc-ring).
- **Repo fit:** `safe`.
- **Determinism:** deterministic.
- **Builds on:** nothing.
- **Sources:** [Godot forum, input buffer](https://forum.godotengine.org/t/where-to-start-when-making-an-input-buffer-system/141330), [Wikipedia, circular buffer](https://en.wikipedia.org/wiki/Circular_buffer).

#### Bip buffer

- **Uses:** a ring buffer for APIs that need one contiguous block of memory (I/O, encoders).
- **Notes:** Simon Cooke's "bipartite" buffer (2003) keeps data in two regions instead of wrapping, so every read and write is one contiguous block. It uses reserve, commit and read steps.
- **Variants:** none noted.
- **Repo fit:** `safe`.
- **Determinism:** deterministic.
- **Builds on:** [Ring buffer](#ring-buffer).
- **Sources:** [bipbuffer crate](https://docs.rs/bipbuffer), [Wikipedia, circular buffer](https://en.wikipedia.org/wiki/Circular_buffer).

#### Mirrored ring buffer

- **Uses:** ring reads and writes that never split at the wrap point.
- **Notes:** maps the same physical memory to two adjacent virtual address ranges. Its size must be a multiple of the page size.
- **Variants:** none noted.
- **Repo fit:** `os`.
- **Determinism:** deterministic.
- **Builds on:** [Ring buffer](#ring-buffer).
- **Sources:** [magic-ring-buffer crate](https://docs.rs/magic-ring-buffer).

#### Deque

- **Uses:** work lists and breadth-first search.
- **Notes:** push and pop at both ends.
- **Variants:** a deque on top of a ring buffer; a deque of blocks.
- **Repo fit:** `safe` (`VecDeque`).
- **Determinism:** deterministic.
- **Builds on:** [Ring buffer](#ring-buffer).
- **Sources:** none checked yet.

#### Double-buffered event queue

- **Uses:** events that systems send to each other between ticks.
- **Notes:** writers fill this tick's buffer while readers read last tick's, and the two swap at a defined point. Bevy's `Events` works this way. This bears on the roadmap's open M7 question of how systems send events to each other.
- **Variants:** one queue per event type; one queue per reader with cursors.
- **Repo fit:** `safe`.
- **Determinism:** the order of events from parallel writers must be defined, for example sorted by system order and then by sequence number.
- **Builds on:** [Ring buffer](#ring-buffer).
- **Sources:** none checked yet.

## 5. Priority queues, timers and sampling

#### Binary and d-ary heap

- **Uses:** A* and Dijkstra open lists, event queues, top-N selection.
- **Notes:** an implicit tree stored in an array. A 4-ary heap is often faster than a binary one in practice because it is shallower.
- **Variants:** min heap or max heap; d-ary heaps.
- **Repo fit:** `safe` (`BinaryHeap`).
- **Determinism:** equal priorities must be broken by an explicit tie-breaker such as insertion sequence or ID.
- **Builds on:** nothing.
- **Sources:** none checked yet.

#### Indexed heap with decrease-key

- **Uses:** Dijkstra and A* with priority updates; schedulers.
- **Notes:** a side table maps each item to its heap position so its priority can be changed in place.
- **Variants:** a binary or d-ary indexed heap.
- **Repo fit:** `safe`.
- **Determinism:** needs a tie-breaker.
- **Builds on:** [Binary and d-ary heap](#binary-and-d-ary-heap).
- **Sources:** none checked yet.

#### Pairing heap

- **Uses:** priority queues with cheap merge and decrease-key.
- **Notes:** a simple, self-adjusting heap kept as a tree.
- **Variants:** none noted.
- **Repo fit:** `safe`.
- **Determinism:** needs a tie-breaker.
- **Builds on:** nothing.
- **Sources:** none checked yet.

#### Radix heap and bucket queue

- **Uses:** shortest paths with small integer costs that only increase over time, such as grid pathfinding.
- **Notes:** a bucket queue (Dial's algorithm) has one bucket per cost value; a radix heap sizes its buckets by powers of two.
- **Variants:** none noted.
- **Repo fit:** `safe`.
- **Determinism:** the order within a bucket must be defined.
- **Builds on:** nothing.
- **Sources:** none checked yet.

#### Working-set heap

- **Uses:** research (FOCS 2024, best-paper distinction).
- **Notes:** Haeupler, Hladík, Rozhoň, Tarjan and Tětek built a heap with a "working-set" property. With it, Dijkstra's algorithm is universally optimal for ordering vertices by distance, while still matching Fibonacci-heap worst-case bounds.
- **Variants:** none noted.
- **Repo fit:** `theory`.
- **Determinism:** not applicable.
- **Builds on:** [Binary and d-ary heap](#binary-and-d-ary-heap).
- **Sources:** [Universal optimality of Dijkstra via beyond-worst-case heaps](https://arxiv.org/abs/2311.11793), [Quanta coverage](https://www.quantamagazine.org/computer-scientists-establish-the-best-way-to-traverse-a-graph-20241025).

#### Timing wheel

- **Uses:** thousands of timers, cooldowns and scheduled gameplay events.
- **Notes:** a ring of buckets indexed by time slot, so adding and expiring timers is O(1); a hierarchy of wheels covers long delays. Introduced by Varghese and Lauck.
- **Variants:** hierarchical wheels; hashed wheels.
- **Repo fit:** `safe`.
- **Determinism:** advance it by simulation ticks, not wall-clock time, and define the order within a slot.
- **Builds on:** [Ring buffer](#ring-buffer).
- **Sources:** none checked yet.

#### Alias table

- **Uses:** O(1) weighted random choice: loot tables, emitter selection, spawn tables.
- **Notes:** Vose's method builds the table in O(n); each sample draws one uniform number and makes one comparison.
- **Variants:** use a [Fenwick tree](#fenwick-tree) when the weights change often.
- **Repo fit:** `safe`. It could sit next to `engine_random`.
- **Determinism:** the construction order and the PRNG calls per sample must be fixed, and the sampling algorithm specified exactly, as `engine_random` already does.
- **Builds on:** nothing.
- **Sources:** none checked yet.

## 6. Hash tables and maps

#### Chained hash table

- **Uses:** general maps; this is the design of `std::unordered_map`.
- **Notes:** every entry is a separate allocation, so lookups jump around memory. Mostly replaced by open addressing.
- **Variants:** chaining inside a single array.
- **Repo fit:** `safe`.
- **Determinism:** iteration order depends on hash values, so it must never be observed.
- **Builds on:** nothing.
- **Sources:** [Ankerl, which hash map](https://martin.ankerl.com/which-hash-map/).

#### Robin Hood hashing

- **Uses:** fast open-addressing maps with predictable probe lengths.
- **Notes:** linear probing in which an inserted entry takes the slot of an entry that is closer to its ideal position than the new one, which keeps the maximum probe distance low. Rust's `HashMap` used it until it switched to hashbrown. It reads more cache lines for keys than a Swiss table does.
- **Variants:** backward-shift deletion (no tombstones); `tsl::robin_map`.
- **Repo fit:** `safe`.
- **Determinism:** hash-dependent order, so it must not be observed.
- **Builds on:** nothing.
- **Sources:** [Swiss tables overview](https://bluuewhale.github.io/posts/swiss-table), [Ankerl, which hash map](https://martin.ankerl.com/which-hash-map/).

#### Hopscotch hashing

- **Uses:** high-load open addressing; concurrent maps.
- **Notes:** Herlihy, Shavit and Tzafrir (DISC 2008). Each bucket has a neighbourhood of H nearby slots and keeps an H-bit map of which entries in it belong to it. It stays fast above 90% full, and its concurrent version scales well. Kelly, Pearlmutter and Maguire published a lock-free version in 2020.
- **Variants:** sequential, concurrent, lock-free.
- **Repo fit:** `safe` (sequential); `unsafe` (concurrent).
- **Determinism:** hash-dependent order.
- **Builds on:** [Bitset](#bitset).
- **Sources:** [Wikipedia, hopscotch hashing](https://en.wikipedia.org/wiki/Hopscotch_hashing), [Tel Aviv University record](https://cris.tau.ac.il/en/publications/hopscotch-hashing/).

#### Cuckoo hashing

- **Uses:** lookups that check at most two or three places; static and GPU tables.
- **Notes:** each key has two or more candidate positions, and inserting may evict ("kick out") an existing key to its other position. The bucketed version is the fastest static GPU hash table in BGHT; a later study found three hash functions with 16-slot buckets the best choice.
- **Variants:** bucketed cuckoo; d-ary cuckoo.
- **Repo fit:** `safe`.
- **Determinism:** the eviction sequence depends on the hash seed, so fix the seed.
- **Builds on:** nothing.
- **Sources:** [BGHT](https://awesome.ecosyste.ms/projects/github.com%2Fowensgroup%2FBGHT), [Better GPU hash tables](https://hgpu.org/?p=25447).

#### Swiss table

- **Uses:** the current default fast hash map.
- **Notes:** a control byte per slot stores part of the hash, and a group of 16 control bytes is checked at once with SIMD, so most key comparisons are skipped. Used by Abseil (`flat_hash_map`), Rust's hashbrown (the standard `HashMap` since Rust 1.36) and Go 1.24's maps.
- **Variants:** Boost `unordered_flat_map`, which keeps elements directly in the bucket array; Folly's F14. In Ankerl's 2026 comparison, `boost::unordered_flat_map` was fastest on 8 of 15 workloads, but the lead changes with the workload.
- **Repo fit:** `safe` (hashbrown), but std `HashMap`/`HashSet` are banned here.
- **Determinism:** order depends on the hash, so use it only where iteration is never observed, or use [Insertion-ordered map](#insertion-ordered-map).
- **Builds on:** nothing.
- **Sources:** [Swiss tables overview](https://bluuewhale.github.io/posts/swiss-table), [Ankerl, which hash map](https://martin.ankerl.com/which-hash-map/), [Boost.Unordered](https://www.boost.org/outreach/program_page/unordered).

#### Insertion-ordered map

- **Uses:** hashed lookup with deterministic iteration; the engine's default map.
- **Notes:** entries live in a dense vector, and a hash table stores their indices. Iteration follows insertion order, so it is a pure function of the operations.
- **Variants:** `ankerl::unordered_dense` (dense storage and a separate index table); `indexmap`.
- **Repo fit:** `in repo` (`engine_collections::IndexMap`, `IndexSet`).
- **Determinism:** already guaranteed (insertion order, defined `swap_remove` and `shift_remove`).
- **Builds on:** [Swiss table](#swiss-table).
- **Sources:** [Ankerl, which hash map](https://martin.ankerl.com/which-hash-map/).

#### Iceberg hashing

- **Uses:** high-load tables with stable positions; GPU tables that allow inserts and queries at the same time.
- **Notes:** offered in BGHT next to power-of-two probing. Unlike BGHT's cuckoo tables, it supports concurrent insertion and queries.
- **Variants:** none noted.
- **Repo fit:** `safe` (CPU); `gpu`.
- **Determinism:** hash-dependent.
- **Builds on:** nothing.
- **Sources:** [BGHT](https://awesome.ecosyste.ms/projects/github.com%2Fowensgroup%2FBGHT).

#### Funnel and elastic hashing

- **Uses:** research (2025).
- **Notes:** Farach-Colton, Krapivin and Kuszmaul. Elastic hashing (non-greedy) achieves O(1) amortized and O(log δ⁻¹) worst-case expected probes without moving existing entries, and they prove that worst-case bound is the best possible. Funnel hashing (greedy) achieves O(log² δ⁻¹), which disproves Yao's conjecture that uniform hashing is optimal. Not tuned for production use.
- **Variants:** none noted.
- **Repo fit:** `theory`.
- **Determinism:** not applicable.
- **Builds on:** nothing.
- **Sources:** [Optimal bounds for open addressing without reordering](https://arxiv.org/abs/2501.02305v2).

#### Minimal perfect hash

- **Uses:** static lookup tables built offline: asset IDs, string IDs, pack file directories.
- **Notes:** n keys map to n slots with no collisions; it can only be built when every key is known in advance.
- **Variants:** none noted.
- **Repo fit:** `safe`.
- **Determinism:** the build must be seeded deterministically.
- **Builds on:** nothing.
- **Sources:** none checked yet.

#### Concurrent hash map

- **Uses:** shared caches and registries read by many threads.
- **Notes:** papaya (Rust) is built for read-heavy use: lock-free reads, incremental resizing for steady latency, and the seize crate for memory reclamation. It suffers on update-heavy work because of allocator pressure and reclamation cost. A deterministic engine rarely needs one.
- **Variants:** sharded maps; [Left-right](#left-right); lock-free maps.
- **Repo fit:** `unsafe`.
- **Determinism:** results must not depend on how threads interleave.
- **Builds on:** [Epoch-based reclamation](#epoch-based-reclamation).
- **Sources:** [papaya](https://github.com/ibraheemdev/papaya).

#### Sorted vector map

- **Uses:** small maps; maps that are built once and read often; deterministic order without hashing.
- **Notes:** lookups binary-search one contiguous array. Insertion is O(n), which is fine for small n.
- **Variants:** a layout tuned for search (see [Eytzinger layout](#eytzinger-layout)).
- **Repo fit:** `safe`.
- **Determinism:** deterministic: ordered by key.
- **Builds on:** nothing.
- **Sources:** none checked yet.

## 7. Ordered and search structures

#### B-tree

- **Uses:** ordered maps, range scans, timelines.
- **Notes:** wide nodes make good use of cache lines.
- **Variants:** a B+ tree (values only in the leaves, leaves linked); see also [Sum tree](#sum-tree).
- **Repo fit:** `safe` (`BTreeMap`).
- **Determinism:** deterministic if the keys are themselves stable. Keys such as addresses or `TypeId` differ between runs, which is why this repo uses `IndexMap` for those.
- **Builds on:** nothing.
- **Sources:** [Algorithmica, B-tree](https://en.algorithmica.org/hpc/data-structures/b-tree/).

#### Static S-tree

- **Uses:** read-only sorted lookups: asset tables, curve keyframes, lookup tables.
- **Notes:** Algorithmica's S-tree and S+ tree are static B-trees without pointers: child positions are computed, and SIMD compares a whole node at once. Much faster than plain binary search.
- **Variants:** S-tree; S+ tree; 16-key blocks sized to one cache line.
- **Repo fit:** `safe`.
- **Determinism:** deterministic.
- **Builds on:** [Eytzinger layout](#eytzinger-layout).
- **Sources:** [Algorithmica, static B-trees](https://en.algorithmica.org/hpc/data-structures/s-tree/), [stree-go](https://pkg.go.dev/github.com/Akron/stree-go).

#### Eytzinger layout

- **Uses:** binary search over large static arrays.
- **Notes:** stores a sorted array in breadth-first (heap) order. Khuong and Morin found it among the best layouts in practice, used with branch-free search and explicit prefetching. In one benchmark on a 1 GB array, query time fell from about 1150 ns to 200 ns.
- **Variants:** branch-free search with prefetching.
- **Repo fit:** `safe`.
- **Determinism:** deterministic.
- **Builds on:** nothing.
- **Sources:** [Khuong and Morin, array layouts](https://cglab.ca/~morin/misc/arraylayout/), [Algorithmica, Eytzinger](https://algorithmica.org/en/eytzinger).

#### Cache-oblivious layout

- **Uses:** trees that perform well at every cache level without knowing the cache sizes.
- **Notes:** the van Emde Boas layout cuts the tree at half its height and lays out the pieces recursively, so a search takes O(log_{B+1} N) memory transfers. Cache-oblivious B-trees are from Bender, Demaine and Farach-Colton (FOCS 2000). Brodal, Fagerberg and Jacob simplified updates by rebuilding the static layout whenever the size doubles or halves.
- **Variants:** concurrent cache-oblivious B-trees.
- **Repo fit:** `safe`.
- **Determinism:** deterministic.
- **Builds on:** nothing.
- **Sources:** [Brodal et al., cache-oblivious search trees](https://www.cs.au.dk/~gerth/papers/soda02.pdf), [MIT CSAIL abstract](https://publications.csail.mit.edu/abstracts/abstracts06/demaine1/demaine1.html).

#### Learned index

- **Uses:** research, mostly from databases: a model predicts where a key sits in sorted data.
- **Notes:** ALEX reports up to 4.1x faster than a B+ tree and indexes up to 2000x smaller (the authors' own numbers). PGM handles inserts with an LSM-tree, which slows lookups. LIPP was proposed as a simpler alternative. Rarely relevant to engines.
- **Variants:** RMI, PGM, ALEX, LIPP.
- **Repo fit:** `theory`.
- **Determinism:** model training must be deterministic.
- **Builds on:** [Sorted vector map](#sorted-vector-map).
- **Sources:** [ALEX](https://ar5iv.arxiv.org/html/1905.08898), [LIPP](https://arxiv.org/pdf/2104.05520).

#### Skip list

- **Uses:** ordered sets that are easy to make concurrent.
- **Notes:** Pugh (1990): balanced by randomness instead of rebalancing rules; search, insert and delete take O(log n) expected time. Pugh's later "cookbook" adds merge, split and concatenate.
- **Variants:** concurrent skip lists.
- **Repo fit:** `safe`.
- **Determinism:** level choices must come from a seeded PRNG.
- **Builds on:** nothing.
- **Sources:** [Pugh, skip lists (CACM 1990)](https://www.cs.ucdavis.edu/~amenta/w04/skiplists.pdf), [A skip list cookbook](https://drum.lib.umd.edu/items/56c44671-3973-46b6-9e52-f71dc95af178/full).

#### Balanced binary search tree

- **Uses:** general ordered sets.
- **Notes:** red-black, AVL, splay trees and treaps. Mostly replaced by B-trees because B-trees use the cache better.
- **Variants:** treap (randomised); splay tree (moves recently used items toward the root).
- **Repo fit:** `safe`.
- **Determinism:** treap priorities must come from a seeded PRNG.
- **Builds on:** nothing.
- **Sources:** none checked yet.

#### Adaptive radix tree

- **Uses:** fast maps keyed by strings or integers; prefix queries.
- **Notes:** Leis et al. (ICDE 2013). Each node picks its size from how many children it has (4, 16, 48 or 256), which keeps a radix tree compact. A synchronised version came later (2016).
- **Variants:** Node4, Node16, Node48, Node256; versions that handle concurrent access.
- **Repo fit:** `safe`.
- **Determinism:** deterministic: ordered by key bytes.
- **Builds on:** [Trie](#trie).
- **Sources:** [ART paper notes](https://www.the-paper-trail.org/post/art-paper-notes), [Leis et al., ART synchronization](https://db.in.tum.de/%7Eleis/papers/artsync.pdf).

#### Trie

- **Uses:** console autocomplete, asset path prefixes, matching input sequences.
- **Notes:** one tree level per character or byte.
- **Variants:** radix (compressed) trie; [Adaptive radix tree](#adaptive-radix-tree).
- **Repo fit:** `safe`.
- **Determinism:** deterministic.
- **Builds on:** nothing.
- **Sources:** none checked yet.

#### Interval tree

- **Uses:** sequencer tracks, animation events, audio regions: "what overlaps time t?"
- **Notes:** each node also stores the largest end point in its subtree, so stabbing and overlap queries skip whole subtrees.
- **Variants:** a centred interval tree; an augmented B-tree.
- **Repo fit:** `safe`.
- **Determinism:** results must be returned in a defined order.
- **Builds on:** [Balanced binary search tree](#balanced-binary-search-tree).
- **Sources:** none checked yet.

#### Segment tree

- **Uses:** range queries such as minimum, maximum or sum over a range of values that changes.
- **Notes:** an implicit binary tree of per-range summaries.
- **Variants:** lazy propagation; a persistent segment tree.
- **Repo fit:** `safe`.
- **Determinism:** with floating-point values, the order of combining must be fixed.
- **Builds on:** nothing.
- **Sources:** none checked yet.

#### Fenwick tree

- **Uses:** prefix sums that change over time; weighted random choice when the weights change.
- **Notes:** a compact implicit tree; both update and prefix query are O(log n).
- **Variants:** 2D Fenwick trees.
- **Repo fit:** `safe`.
- **Determinism:** use integer weights, or fix the combining order.
- **Builds on:** nothing.
- **Sources:** none checked yet.

## 8. Bitsets and compressed sequences

#### Bitset

- **Uses:** component masks, dirty flags, occupancy, free bins in allocators.
- **Notes:** one bit per index, scanned a word at a time.
- **Variants:** a fixed size or a growable size.
- **Repo fit:** `safe`.
- **Determinism:** deterministic.
- **Builds on:** nothing.
- **Sources:** none checked yet.

#### Hierarchical bitset

- **Uses:** ECS joins; iterating sparse sets of indices quickly.
- **Notes:** each upper layer records which ranges below it are non-empty, so union, intersection and iteration skip empty ranges. `hibitset` (from specs) also iterates in parallel with rayon. Its layer sizes depend on pointer width.
- **Variants:** a fixed number of layers; parallel iteration.
- **Repo fit:** `safe`.
- **Determinism:** layer sizes must not depend on pointer width (this repo's rule against `usize`-dependent behaviour).
- **Builds on:** [Bitset](#bitset).
- **Sources:** [hibitset](https://docs.rs/hibitset).

#### Sparse hierarchical bitset

- **Uses:** the same jobs as a hierarchical bitset, with less memory.
- **Notes:** `hi_sparse_bitset` stores only the blocks that have bits set, so memory grows with occupied blocks, not the largest index. Its iteration cursors can stop and resume. It claims to be faster than hibitset (its own benchmarks).
- **Variants:** none noted.
- **Repo fit:** `safe`.
- **Determinism:** deterministic.
- **Builds on:** [Hierarchical bitset](#hierarchical-bitset).
- **Sources:** [hi_sparse_bitset](https://docs.rs/crate/hi_sparse_bitset/0.9.0/source/README.md).

#### Roaring bitmap

- **Uses:** large sparse sets of IDs, query caches, "has component" indexes.
- **Notes:** splits the integer range into chunks and stores each chunk as a sorted array, a bitmap or a list of runs, whichever is smallest. It is often about 2x smaller and faster than older compressed bitmaps such as WAH and Concise.
- **Variants:** array, bitmap and run containers.
- **Repo fit:** `safe`.
- **Determinism:** deterministic.
- **Builds on:** [Bitset](#bitset).
- **Sources:** [Engineering compact rank/select structures](https://arxiv.org/pdf/2206.01149).

#### Rank and select bit vector

- **Uses:** a building block for compressed structures: "how many 1s before position i" and "where is the k-th 1" in O(1).
- **Notes:** needs only a small amount of extra space on top of the bits.
- **Variants:** RRR compression.
- **Repo fit:** `safe`.
- **Determinism:** deterministic.
- **Builds on:** [Bitset](#bitset).
- **Sources:** [RRR](https://alexbowe.com/rrr), [Engineering compact rank/select structures](https://arxiv.org/pdf/2206.01149).

#### Elias-Fano encoding

- **Uses:** compressed sorted integer sequences such as ID lists and offsets.
- **Notes:** stores the low bits plainly and the high bits in unary, with rank and select support.
- **Variants:** partitioned Elias-Fano.
- **Repo fit:** `safe`.
- **Determinism:** deterministic.
- **Builds on:** [Rank and select bit vector](#rank-and-select-bit-vector).
- **Sources:** [SDSL introduction](https://didawiki.di.unipi.it/lib/exe/fetch.php/magistraleinformaticanetworking/ae/ae2021/sdsl_a_gentle_introduction.pdf).

#### Wavelet tree

- **Uses:** rank and select over sequences of symbols, not just bits.
- **Notes:** a tree of bit vectors; each query walks from the root to a leaf.
- **Variants:** wavelet matrix.
- **Repo fit:** `safe`.
- **Determinism:** deterministic.
- **Builds on:** [Rank and select bit vector](#rank-and-select-bit-vector).
- **Sources:** [On compressing permutations and adaptive sorting](https://arxiv.org/pdf/1108.4408).

## 9. Probabilistic filters and sketches

#### Bloom filter

- **Uses:** "is this asset in the pack?", whether a shader cache has an entry, checking a negative cache.
- **Notes:** you can add keys over time. Uses about 44% more space than the theoretical minimum.
- **Variants:** blocked Bloom (one cache line per key); counting Bloom (supports deletion).
- **Repo fit:** `safe`.
- **Determinism:** the hash seed must be fixed.
- **Builds on:** [Bitset](#bitset).
- **Sources:** [Binary fuse filters](https://arxiv.org/abs/2201.01174).

#### Cuckoo filter

- **Uses:** membership tests with deletion.
- **Notes:** often better than Bloom filters, and it supports removing keys.
- **Variants:** none noted.
- **Repo fit:** `safe`.
- **Determinism:** the hash seed must be fixed.
- **Builds on:** [Cuckoo hashing](#cuckoo-hashing).
- **Sources:** [Binary fuse filters](https://arxiv.org/abs/2201.01174).

#### Quotient filter

- **Uses:** membership tests with counting, deletion, resizing and merging.
- **Notes:** Pandey et al.'s counting quotient filter (SIGMOD 2017) reorganises its metadata so most operations become rank and select. It stays fast up to 95% full, is several times faster than Bloom filters and about as fast as cuckoo filters while also counting, and it works on SSD.
- **Variants:** quotient filter; counting quotient filter.
- **Repo fit:** `safe`.
- **Determinism:** the hash seed must be fixed.
- **Builds on:** [Rank and select bit vector](#rank-and-select-bit-vector).
- **Sources:** [Pandey et al., a general-purpose counting filter](https://blog.acolyer.org/2017/08/08/a-general-purpose-counting-filter-making-every-bit-count/), [cqf](https://github.com/splatlab/cqf).

#### Xor and binary fuse filters

- **Uses:** static sets built offline: pack contents, shader cache indexes.
- **Notes:** every key must be known when the filter is built. Xor filters are within about 23% of the minimum space, binary fuse filters about 13% (8% for the 4-wise version), against 44% for Bloom. Queries can be about twice as fast as Bloom, and binary fuse filters build more than twice as fast as xor filters (Graf and Lemire, 2022).
- **Variants:** 3-wise and 4-wise binary fuse.
- **Repo fit:** `safe`.
- **Determinism:** the construction seed must be fixed.
- **Builds on:** nothing.
- **Sources:** [Binary fuse filters](https://arxiv.org/abs/2201.01174), [xorf crate](https://docs.rs/xorf/latest/xorf/).

#### Ribbon filter

- **Uses:** static sets where space and false-positive rate need fine tuning.
- **Notes:** Dillinger and Walzer (2021). Like an xor filter but built by solving a banded system of linear equations over bits, which keeps memory accesses local.
- **Variants:** none noted.
- **Repo fit:** `safe`.
- **Determinism:** the seed must be fixed.
- **Builds on:** nothing.
- **Sources:** [Aleph filter (related work)](https://arxiv.org/html/2404.04703v5).

#### Prefix filter

- **Uses:** filters that keys are added to over time.
- **Notes:** VLDB 2022: space similar to other dynamic filters, query speed comparable to cuckoo filters.
- **Variants:** none noted.
- **Repo fit:** `safe`.
- **Determinism:** the seed must be fixed.
- **Builds on:** nothing.
- **Sources:** [Aleph filter (related work)](https://arxiv.org/html/2404.04703v5).

#### Expandable filter

- **Uses:** filters for sets that keep growing, with no rebuild.
- **Notes:** InfiniFilter (2023) doubles its capacity by giving up one fingerprint bit per entry, so its false-positive rate rises as it grows. Aleph (Dayan, Bercea, Pagh, VLDB 2024) supports insert, query and delete in constant time however large the data gets. Given an estimate of eventual growth, its memory versus false-positive trade-off matches static filters.
- **Variants:** InfiniFilter, Aleph.
- **Repo fit:** `safe`.
- **Determinism:** the seed must be fixed.
- **Builds on:** [Quotient filter](#quotient-filter).
- **Sources:** [Aleph filter](https://arxiv.org/html/2404.04703v5).

#### HyperLogLog

- **Uses:** counting distinct items in telemetry: unique assets touched, unique players.
- **Notes:** Flajolet et al.: a standard error of about 1.04/√m. HyperLogLogLog (2022) is about 40% smaller.
- **Variants:** HyperLogLog++; HyperLogLogLog; newer variants such as UltraLogLog and ExaLogLog (not checked).
- **Repo fit:** `safe`.
- **Determinism:** the hash seed must be fixed.
- **Builds on:** nothing.
- **Sources:** [Flajolet et al., HyperLogLog](https://dmtcs.episciences.org/3545/pdf), [HyperLogLogLog](https://arxiv.org/pdf/2205.11327).

#### Count-min sketch

- **Uses:** finding the most frequent items: the most-loaded assets, the most frequent events.
- **Notes:** Cormode and Muthukrishnan (2005). Space grows with 1/ε. It can overestimate a count but never underestimates it.
- **Variants:** count sketch.
- **Repo fit:** `safe`.
- **Determinism:** the hash seed must be fixed.
- **Builds on:** nothing.
- **Sources:** [Cormode and Muthukrishnan, count-min](https://dsf.berkeley.edu/cs286/papers/countmin-latin2004.pdf).

#### Quantile sketch

- **Uses:** frame-time and hitch percentiles (p99) in profiling and benchmarks (roadmap M9, `bench-report`).
- **Notes:** HDR Histogram records values at a configurable precision across a wide range and is wait-free. DDSketch inserts fast and guarantees a relative error. t-digest is most accurate in the tails, and two digests merge without the raw data.
- **Variants:** HDR Histogram, DDSketch, t-digest, KLL.
- **Repo fit:** `safe`.
- **Determinism:** deterministic for a given input order. Merging sketches in a different order may change results slightly.
- **Builds on:** nothing.
- **Sources:** [DDSketch](https://alphaxiv.org/abs/1908.10693v1), [Timescale percentile approximation](https://www.tigerdata.com/docs/build/data-management/hyperfunctions/percentile-approx/advanced-agg).

## 10. Strings, names and text

#### Interned name table

- **Uses:** names and tags compared as integers (Unreal's `FName`).
- **Notes:** a global table of unique strings, hashed for fast lookup from a string. An `FName` is an index into the table plus an instance number (the `_1` in `MyObject_1`). The index can change between runs, so saved data must store the string. UE3's version never freed unused names. A global table conflicts with [ADR 0008](../adr/0008-no-global-state.md), so here it would be a table owned by the world or the registry.
- **Variants:** lookup that only finds an existing name without adding it; instance numbers.
- **Repo fit:** `safe` (without global state).
- **Determinism:** IDs depend on the order names are added, so persist the strings, not the IDs.
- **Builds on:** [Insertion-ordered map](#insertion-ordered-map), [String arena](#string-arena).
- **Sources:** [Unreal FName](https://dev.epicgames.com/documentation/en-us/unreal-engine/fname-in-unreal-engine), [GameplayTags and FNames in depth](https://itsbaffled.github.io/posts/UE/GameplayTags-And-FNames-In-Depth), [UE3 names](https://robojumper.github.io/too-real/names-strings).

#### Hashed string ID

- **Uses:** event, asset and localisation keys computed when building.
- **Notes:** costs nothing at runtime, but the build must detect collisions. Unlike an interned table, the ID is stable across runs.
- **Variants:** FNV or xxHash, 32-bit or 64-bit.
- **Repo fit:** `safe`.
- **Determinism:** a stable hash is a frozen format (roadmap M5's stable content hasher needs an ADR).
- **Builds on:** nothing.
- **Sources:** none checked yet.

#### Inline string

- **Uses:** fewer allocations; fast comparisons of string keys.
- **Notes:** "German strings" (from the Umbra database, now CedarDB) are a fixed 16-byte struct. Short strings (about 12 bytes, depending on the implementation) are stored inside the struct. Long strings keep their length, a 4-byte prefix and a pointer, so many comparisons finish on the prefix alone.
- **Variants:** small-string optimisation; German strings; `strumbra`, `german-str`.
- **Repo fit:** `safe` (handles in place of raw pointers) or `unsafe`.
- **Determinism:** deterministic.
- **Builds on:** [String arena](#string-arena).
- **Sources:** [CedarDB, German strings](https://cedardb.com/blog/german_strings), [spellbook notes](https://the-mikedavis.github.io/posts/german-string-optimizations-in-spellbook).

#### String arena

- **Uses:** loading big string tables in one allocation, as localisation and debug names need.
- **Notes:** all the text in one buffer, referenced by (offset, length) spans.
- **Variants:** one per frame; one per asset.
- **Repo fit:** `safe`.
- **Determinism:** deterministic.
- **Builds on:** [Bump arena](#bump-arena).
- **Sources:** none checked yet.

#### Rope

- **Uses:** large text buffers edited in place: script editors, consoles.
- **Notes:** a balanced tree of text chunks, so inserting in the middle is O(log n).
- **Variants:** [Sum tree](#sum-tree) ropes.
- **Repo fit:** `safe`, `subsystem` (editor).
- **Determinism:** deterministic.
- **Builds on:** [B-tree](#b-tree).
- **Sources:** [Zed decoded: rope and SumTree](https://www.simonwillison.net/2024/Apr/28/zed-decoded-rope-sumtree).

#### Piece table and gap buffer

- **Uses:** text editors.
- **Notes:** a piece table records edits as pieces of the original and an append-only buffer; a gap buffer keeps an empty gap at the cursor, so typing there is cheap.
- **Variants:** a piece tree (pieces held in a balanced tree).
- **Repo fit:** `safe`, `subsystem` (editor).
- **Determinism:** deterministic.
- **Builds on:** nothing.
- **Sources:** none checked yet.

#### Sum tree

- **Uses:** any sequence you seek through by more than one measure: lines and bytes in text, time in timelines, weight in weighted lists. Zed uses it for highlights, folds, git blame and file trees.
- **Notes:** a B+ tree whose nodes store a summary of their subtree, defined by the user. Each part of the summary (lines, characters, bytes) is a "dimension" a cursor can seek by, and cursors can seek by several at once. Zed's base node size was 6 (up to 12 items per node).
- **Variants:** a rope over a sum tree; finger trees.
- **Repo fit:** `safe`.
- **Determinism:** deterministic if summaries are combined in a fixed order.
- **Builds on:** [B-tree](#b-tree).
- **Sources:** [Zed decoded: rope and SumTree](https://www.simonwillison.net/2024/Apr/28/zed-decoded-rope-sumtree), [Zed rope notes](https://greathongtu.github.io/posts/zed-rope).

## 11. ECS storage

#### Bitmask signature

- **Uses:** early ECS designs such as Artemis: each entity has a bitmask of its components, and a query compares masks.
- **Notes:** simple, but a query has to test every entity.
- **Variants:** masks combined with [Hierarchical bitset](#hierarchical-bitset) joins (specs).
- **Repo fit:** `safe`.
- **Determinism:** deterministic.
- **Builds on:** [Bitset](#bitset).
- **Sources:** none checked yet.

#### Archetype table

- **Uses:** fast queries over many entities: each archetype (a unique set of component types) gets a table with one column per type.
- **Notes:** a graph of edges records which table you reach by adding or removing a component, so the move is an O(1) lookup. Flecs calls tables archetypes.
- **Variants:** [Chunked archetype storage](#chunked-archetype-storage); tables split per relationship target (see [Entity relationships](#entity-relationships)).
- **Repo fit:** `in repo` (M3.4 prototype).
- **Determinism:** the order tables are created in and the order of rows within them must follow the operation sequence. Moving between tables uses swap-remove.
- **Builds on:** [Type-erased column](#type-erased-column), [Insertion-ordered map](#insertion-ordered-map).
- **Sources:** [Flecs internals gist](https://gist.github.com/SanderMertens/18be89a5cfdf7faa6aef8adfa19a1869), [Geary archetype graph](https://docs.mineinabyss.com/geary/backend/core/archetype-graph).

#### Chunked archetype storage

- **Uses:** archetype rows in fixed-size chunks (Unity DOTS uses 16 KB).
- **Notes:** Unreal Mass has per-entity fragments, shared fragments (one copy per archetype), chunk fragments (one per chunk), and tags (zero-size markers). Changing an entity's composition moves it to another archetype.
- **Variants:** shared and per-chunk data.
- **Repo fit:** `safe`.
- **Determinism:** the order chunks are filled must be defined.
- **Builds on:** [Archetype table](#archetype-table).
- **Sources:** [Mass Entity data types](https://x157.github.io/UE5/Mass/DataTypes), [Epic, Mass overview](https://dev.epicgames.com/documentation/unreal-engine/overview-of-mass-entity-in-unreal-engine).

#### Sparse-set component storage

- **Uses:** components that are added and removed often.
- **Notes:** one [Sparse set](#sparse-set) per component type. A query intersects sets, starting with the smallest.
- **Variants:** [Owning groups](#owning-groups).
- **Repo fit:** `in repo` (M3.3 prototype).
- **Determinism:** dense order is a function of the operations.
- **Builds on:** [Sparse set](#sparse-set).
- **Sources:** [skypjack, ECS back and forth](https://skypjack.github.io/2019-03-21-ecs-baf-part-2-insights).

#### Per-component storage choice

- **Uses:** a mix: table storage by default, sparse-set storage for components that come and go often.
- **Notes:** Bevy's `StorageType` is `Table` (the default: fast, cache-friendly iteration but slower to add and remove) or `SparseSet` (fast to add and remove), chosen per component type. One entity can have components of both kinds.
- **Variants:** none noted.
- **Repo fit:** `in repo` (M3.5 prototype).
- **Determinism:** as for its two parts.
- **Builds on:** [Archetype table](#archetype-table), [Sparse-set component storage](#sparse-set-component-storage).
- **Sources:** [Bevy StorageType](https://docs.rs/bevy/0.16.0/bevy/ecs/component/enum.StorageType.html), [Bevy Cheatbook (dated)](https://bevy-cheatbook.github.io/patterns/component-storage.html).

#### Owning groups

- **Uses:** perfect struct-of-arrays layouts for particular combinations of components.
- **Notes:** EnTT reorders the pools in a group so the entities that have every component in the group sit at the front, in the same order. A pool can belong to only one owning group, and two groups that own the same types must be nested.
- **Variants:** full-owning, partial-owning and non-owning groups.
- **Repo fit:** `safe`.
- **Determinism:** the reorder is a function of the operations.
- **Builds on:** [Sparse-set component storage](#sparse-set-component-storage).
- **Sources:** [EnTT groups notes](https://gist.github.com/dakom/82551fff5d2b843cbe1601bbaff2acbf), [skypjack, nested groups](https://skypjack.github.io/2019-11-19-ecs-baf-part-6).

#### Entity relationships

- **Uses:** hierarchies, ownership, "child of", "targets".
- **Notes:** Flecs stores (relationship, target) pairs as components, which gives one archetype per target. That makes "children of X" fast, but many targets fragment the storage, and creating archetypes for relationships costs extra. Bevy added relationships in 0.16. The roadmap keeps relationships out of M4.
- **Variants:** exclusive relationships; [Non-fragmenting components](#non-fragmenting-components).
- **Repo fit:** `safe`.
- **Determinism:** the order of targets must be defined.
- **Builds on:** [Archetype table](#archetype-table).
- **Sources:** [CUBOS relations](https://gamedevtecnico.github.io/cubos/), [jecs relationships](https://ukendio.github.io/jecs/learn/concepts/relationships.html), [Arche relations](https://mlange-42.github.io/arche/guide/relations).

#### Non-fragmenting components

- **Uses:** relationships and components that should not split archetypes.
- **Notes:** Flecs 4.1 added non-fragmenting components (the `DontFragment` trait) and replaced its old relationship mechanism with `DontFragment` and `Exclusive`. A later release reports queries on sparse components up to 20x faster. Mertens has said this storage is specialised for asset hierarchies.
- **Variants:** sparse component storage.
- **Repo fit:** `safe`.
- **Determinism:** as for [Sparse set](#sparse-set).
- **Builds on:** [Sparse set](#sparse-set).
- **Sources:** [Flecs releases](https://github.com/SanderMertens/flecs/releases?page=1), [Bevy discussion quoting Mertens](https://github.xiaohuzi123231346.workers.dev/bevyengine/bevy/issues/17301).

#### Change ticks

- **Uses:** change detection, such as "added or changed since my last run" (roadmap M6).
- **Notes:** each component column stores the tick of the last write, and each system remembers the tick it last ran at.
- **Variants:** per-component ticks; per-chunk ticks; bitsets of added and changed entities.
- **Repo fit:** `safe`.
- **Determinism:** ticks come from `engine_time::Tick`, never wall-clock time.
- **Builds on:** [Bitset](#bitset).
- **Sources:** none checked yet.

#### Command buffer

- **Uses:** structural changes (spawn, despawn, insert, remove) recorded during iteration and applied later.
- **Notes:** each thread records its own commands, and they are applied at a sync point.
- **Variants:** commands sorted by a key before they are applied.
- **Repo fit:** `safe`.
- **Determinism:** buffers must be applied in a defined order, such as system order then recording order, never thread completion order.
- **Builds on:** [Bump arena](#bump-arena).
- **Sources:** none checked yet.

#### Type-erased column

- **Uses:** component columns for types registered at runtime, such as mod-defined components (M3.6) and C# components (M8).
- **Notes:** raw bytes plus a layout descriptor (size, alignment, drop and clone functions). The safe version keeps bytes in `Vec<u8>` and copies values in and out; the fast version needs `unsafe` (M3.7).
- **Variants:** safe byte columns; `unsafe` pointer columns.
- **Repo fit:** `in repo` (M3.6, M3.7).
- **Determinism:** deterministic. Padding bytes must not reach hashes.
- **Builds on:** nothing.
- **Sources:** none checked yet.

#### Query cache

- **Uses:** remembering which archetypes match a query, so it is not re-matched every frame.
- **Notes:** updated when a new archetype is created.
- **Variants:** none noted.
- **Repo fit:** `safe`.
- **Determinism:** cached archetypes must be iterated in creation order.
- **Builds on:** [Archetype table](#archetype-table).
- **Sources:** none checked yet.

## 12. Spatial partitioning and queries

#### Uniform grid

- **Uses:** particles, crowds, and many objects of similar size.
- **Notes:** Ihmsen et al. compared five uniform-grid variants for SPH fluids. Z-index sort and compact hashing were fastest, with the cell size equal to the influence radius. Build by counting sort plus a prefix sum.
- **Variants:** counting-sort build; Z-index sort; [Spatial hash grid](#spatial-hash-grid).
- **Repo fit:** `safe`.
- **Determinism:** objects within a cell must be ordered (by handle or by insertion).
- **Builds on:** [Space-filling curve](#space-filling-curve).
- **Sources:** [Ihmsen et al., data structures for SPH](https://cg.informatik.uni-freiburg.de/publications/2011_CGF_dataStructuresSPH.pdf).

#### Spatial hash grid

- **Uses:** large or sparse worlds.
- **Notes:** memory grows with the number of objects, not the world size. Cell size is the key setting; large objects have to go into many cells.
- **Variants:** compact hashing (Ihmsen); a grid for small objects combined with a tree for large ones.
- **Repo fit:** `safe`.
- **Determinism:** the hash must be fixed and queries must return results in a defined order. `IndexMap`, or a sort of the results, fits.
- **Builds on:** [Uniform grid](#uniform-grid), [Insertion-ordered map](#insertion-ordered-map).
- **Sources:** [Spatial hashing for collisions](https://saeed1262.github.io/blog/2025/spatial-hashing-collision), [Ihmsen et al.](https://cg.informatik.uni-freiburg.de/publications/2011_CGF_dataStructuresSPH.pdf).

#### Hierarchical grid

- **Uses:** objects of very different sizes.
- **Notes:** several grids with different cell sizes; each object goes in the level that matches its size.
- **Variants:** none noted.
- **Repo fit:** `safe`.
- **Determinism:** as for [Uniform grid](#uniform-grid).
- **Builds on:** [Uniform grid](#uniform-grid).
- **Sources:** none checked yet.

#### Quadtree and octree

- **Uses:** region queries, picking, level data.
- **Notes:** good for spatial queries, weaker for finding pairs of colliding objects.
- **Variants:** pointer trees; linear trees keyed by Morton code (no pointers); hashed trees; [Loose octree](#loose-octree).
- **Repo fit:** `safe`.
- **Determinism:** results returned in a defined order.
- **Builds on:** [Space-filling curve](#space-filling-curve).
- **Sources:** [Gamedev.net, spatial partitioning](https://gamedev.net/forums/topic/598183-spatial-partitioning/4788952/), [DragonSpace demo](https://github.com/Appleguysnake/DragonSpace-Demo).

#### Loose octree

- **Uses:** dynamic objects in a tree.
- **Notes:** nodes are allowed to overlap, so each object goes into exactly one node.
- **Variants:** a looseness factor (usually 2).
- **Repo fit:** `safe`.
- **Determinism:** as for [Quadtree and octree](#quadtree-and-octree).
- **Builds on:** [Quadtree and octree](#quadtree-and-octree).
- **Sources:** [Real-Time Rendering, collision detection chapter](https://mail.realtimerendering.com/Real-Time_Rendering_4th-Collision_Detection.pdf).

#### k-d tree

- **Uses:** nearest neighbours over static points, finding the agents near each agent for crowd avoidance (RVO2), and motion-matching search.
- **Notes:** splits space by alternating axes. GPU builds exist.
- **Variants:** a balanced build using the median; a build using the surface area heuristic.
- **Repo fit:** `safe`.
- **Determinism:** equal distances must be broken by ID.
- **Builds on:** nothing.
- **Sources:** [RVO2, what is new](https://gamma.cs.unc.edu/RVO2/documentation/2.0/whatsnew.html), [k-d trees for particles](https://people.montefiore.uliege.be/poirrier/download/particle/poirrier-kdtree-pp1.pdf).

#### Sweep and prune

- **Uses:** finding overlapping pairs when objects move only a little each frame.
- **Notes:** keeps interval end points sorted along one or more axes and updates them each frame. PhysX's SAP is a three-axis version that works well when many objects are asleep, but it needs world bounds.
- **Variants:** one-axis or three-axis; incremental; the GPU version in PhysX.
- **Repo fit:** `safe`.
- **Determinism:** equal end points must be ordered by ID, and the output pairs sorted.
- **Builds on:** nothing.
- **Sources:** [PhysX broad-phase types](https://nvidia-omniverse.github.io/PhysX/physx/5.5.1/_api_build/structPxBroadPhaseType.html), [Unity broad phase](https://docs.unity.com/en-us/engine/6000.3/manual/physics-section/physics-overview/physics-optimization/cpu/broad-phase.md).

#### Box pruning

- **Uses:** a physics broadphase that handles many moving objects.
- **Notes:** PhysX's MBP (multi box pruning) needs regions defined in advance. ABP manages regions automatically, often has the best average speed and memory use, and needs neither world bounds nor regions. PABP is a parallel ABP and often the fastest on the CPU, at a higher memory cost.
- **Variants:** MBP, ABP, PABP.
- **Repo fit:** `safe`.
- **Determinism:** output pairs must be sorted.
- **Builds on:** [Sweep and prune](#sweep-and-prune).
- **Sources:** [PhysX broad-phase types](https://nvidia-omniverse.github.io/PhysX/physx/5.5.1/_api_build/structPxBroadPhaseType.html).

#### Dynamic AABB tree

- **Uses:** the standard broadphase for dynamic physics; it also answers ray casts and region queries.
- **Notes:** leaves store enlarged ("fat") boxes and are reinserted only when the object leaves its box. Insertion picks the branch that grows least, using the surface area heuristic, and tree rotations keep it balanced. Box2D uses it. Godot moved both its 2D and 3D physics to a dynamic BVH (2025) and removed the slower alternatives.
- **Variants:** fat margins; insertion guided by the surface area heuristic; rotations; rebuilding after bulk insertion.
- **Repo fit:** `safe`.
- **Determinism:** insertion order decides the tree shape, so queries must return results in a defined order, not tree order.
- **Builds on:** [SAH BVH](#sah-bvh).
- **Sources:** [Catto, dynamic BVH (GDC 2019)](https://box2d.org/files/ErinCatto_DynamicBVH_GDC2019.pdf), [Chou, dynamic AABB tree](https://allenchou.net/2014/02/game-physics-broadphase-dynamic-aabb-tree/), [Godot commit](https://remotebranch.eu/Stowage/godot/commit/3877ed73d01bbbf764d0a2756fe6b310ba9dc6f4).

#### Layered quadtree broadphase

- **Uses:** a physics broadphase you can query from other threads (Jolt).
- **Notes:** one quadtree per broadphase layer. It is lock-free, so bodies can be loaded and unloaded in the background and collision queries run in parallel while the simulation updates; `FrameSync` is the one point where it waits. The number of layers is fixed when it starts. Adding bodies one at a time makes a poor tree, so add them in batches, and the tree is rebalanced during the update.
- **Variants:** none noted.
- **Repo fit:** `unsafe` (lock-free).
- **Determinism:** queries made during the update may see different states. Deterministic simulation queries need a fixed snapshot.
- **Builds on:** [Quadtree and octree](#quadtree-and-octree).
- **Sources:** [Architecting Jolt for Horizon Forbidden West](https://www.guerrilla-games.com/read/architecting-jolt-physics-for-horizon-forbidden-west), [Jolt broadphase discussion](https://github.com/jrouwe/JoltPhysics/discussions/139).

#### R-tree

- **Uses:** many static rectangles; picking in the editor; map data.
- **Notes:** groups nearby rectangles into bounding boxes, a page at a time.
- **Variants:** R*-tree; bulk-loaded (STR).
- **Repo fit:** `safe`.
- **Determinism:** results returned in a defined order.
- **Builds on:** nothing.
- **Sources:** none checked yet.

#### BSP tree

- **Uses:** static indoor levels (Doom, Quake): visibility order, collision.
- **Notes:** built offline; poor for dynamic scenes.
- **Variants:** solid-leaf BSP.
- **Repo fit:** `safe`.
- **Determinism:** deterministic.
- **Builds on:** nothing.
- **Sources:** none checked yet.

#### Portals and potentially visible sets

- **Uses:** indoor visibility.
- **Notes:** cells joined by portals, or a visibility table precomputed per cell. Mostly replaced by GPU occlusion culling (see [Hierarchical Z-buffer](#hierarchical-z-buffer)).
- **Variants:** none noted.
- **Repo fit:** `safe`, `subsystem` (rendering).
- **Determinism:** deterministic.
- **Builds on:** [BSP tree](#bsp-tree).
- **Sources:** none checked yet.

#### Space-filling curve

- **Uses:** sort keys, cache-friendly ordering, building LBVHs, quadtrees without pointers.
- **Notes:** a Morton code interleaves the bits of quantised coordinates. Hilbert curves keep nearby points closer together but cost more to compute.
- **Variants:** Morton (Z-order), Hilbert, extended Morton codes that also encode primitive size.
- **Repo fit:** `safe`.
- **Determinism:** quantisation must be exactly specified.
- **Builds on:** nothing.
- **Sources:** [HLBVH](https://research.nvidia.com/sites/default/files/pubs/2010-06_HLBVH-Hierarchical-LBVH/HLBVH-final.pdf), [Extended Morton codes](https://dcgi.fel.cvut.cz/home/bittner/emc/emc2017.pdf).

#### RT-core spatial queries

- **Uses:** research: neighbour and range queries on ray-tracing hardware.
- **Notes:** RTNN (2022) turns fixed-radius neighbour search into ray queries. TrueKNN grows the radius until it finds k neighbours; Juno handles high-dimensional vectors; Arkade handles distances other than Euclidean; ORCS (2026) targets particle simulation. A 2026 review finds neighbour search benefits most, while RT cores often sit unused in general computing.
- **Variants:** fixed-radius, k-NN, approximate.
- **Repo fit:** `gpu`, `theory`.
- **Determinism:** hardware traversal order is unspecified, so results must be sorted.
- **Builds on:** [Two-level BVH](#two-level-bvh).
- **Sources:** [RT cores for general computing, review](https://arxiv.org/html/2603.28771v1), [ORCS](https://arxiv.org/html/2601.15633v1), [TrueKNN](https://arxiv.org/pdf/2305.18356.pdf).

## 13. Bounding volume hierarchies

#### SAH BVH

- **Uses:** ray casts and mesh collision; quality builds offline or at load time.
- **Notes:** the surface area heuristic estimates traversal cost from node surface areas, because the chance a ray hits a convex object is proportional to its surface area.
- **Variants:** binned builds; SBVH, which splits primitives spatially.
- **Repo fit:** `safe`.
- **Determinism:** a single-threaded build is deterministic; a parallel build needs a fixed partition and a fixed order of reductions.
- **Builds on:** nothing.
- **Sources:** [Catto, dynamic BVH (GDC 2019)](https://box2d.org/files/ErinCatto_DynamicBVH_GDC2019.pdf).

#### BVH refitting and restructuring

- **Uses:** animated scenes.
- **Notes:** refit updates the boxes and keeps the tree shape; restructuring rebuilds only the parts whose culling has got poor (selective restructuring, 2007). Treelet optimisation improves the quality of a fast build.
- **Variants:** refit; rotations; reinsertion; treelet restructuring.
- **Repo fit:** `safe`.
- **Determinism:** as for [SAH BVH](#sah-bvh).
- **Builds on:** [SAH BVH](#sah-bvh).
- **Sources:** [Selective restructuring](https://dspace.kaist.ac.kr/bitstream/10203/24301/1/Selective.pdf), [PLOC (related work)](https://dcgi.fel.cvut.cz/wp-content/wpallimport-dist/publications/pdf/publications-2018-meister-tvcg-ploc-paper.pdf).

#### Linear BVH

- **Uses:** very fast parallel builds.
- **Notes:** sorts primitives by Morton code and builds the tree from the sorted order. Karras (2012) builds the whole tree in one kernel launch. Trees are lower quality than SAH builds; extended Morton codes improve quality by up to 54%.
- **Variants:** HLBVH; Karras-Apetrei.
- **Repo fit:** `safe`, `gpu`.
- **Determinism:** codes must be unique (append the index to break ties).
- **Builds on:** [Space-filling curve](#space-filling-curve), [Onesweep radix sort](#onesweep-radix-sort).
- **Sources:** [HLBVH](https://research.nvidia.com/sites/default/files/pubs/2010-06_HLBVH-Hierarchical-LBVH/HLBVH-final.pdf), [Linear BVH notes](https://forwardercn.github.io/blogs/blog2024/002-Linear_BVH.html).

#### PLOC and H-PLOC

- **Uses:** fast GPU builds with near-SAH quality.
- **Notes:** PLOC (2018) builds the tree bottom-up by clustering nearby primitives in parallel. H-PLOC (ACM TOG 2024) gives similar quality in one kernel launch with 1.1–3.6x lower build times than PLOC++ and ATRBVH. It can also convert a binary BVH to an n-wide one in a single kernel, and a follow-up merges that collapse into the H-PLOC build itself.
- **Variants:** PLOC++, H-PLOC.
- **Repo fit:** `gpu`.
- **Determinism:** parallel clustering must be deterministic for a given build.
- **Builds on:** [Linear BVH](#linear-bvh).
- **Sources:** [H-PLOC](https://gpuopen.com/download/publications/HPLOC.pdf), [PLOC](https://dcgi.felk.cvut.cz/projects/ploc/), [Wide BVH construction](https://arxiv.org/html/2506.22849v1).

#### Wide and compressed BVH

- **Uses:** SIMD traversal on the CPU; compressed GPU nodes.
- **Notes:** BVH4 or BVH8 nodes test 4 or 8 boxes at once. CWBVH (Ylitie, Karras, Laine 2017) quantises child boxes. obvhs ships a CWBVH8 builder.
- **Variants:** BVH4, BVH8, CWBVH, quantised nodes.
- **Repo fit:** `safe`, `gpu`.
- **Determinism:** quantisation must be exact.
- **Builds on:** [SAH BVH](#sah-bvh).
- **Sources:** [obvhs](https://lib.rs/crates/obvhs), [Wide BVH thesis](https://dcgi.fel.cvut.cz/wp-content/wpallimport-dist/theses/pdf/theses-2025-cezneluk-thesis.pdf).

#### Two-level BVH

- **Uses:** instanced scenes; the model used by DXR and Vulkan ray tracing.
- **Notes:** a top-level tree over instances, with one bottom-level tree per mesh. It mirrors instancing in the scene.
- **Variants:** none noted.
- **Repo fit:** `safe`, `gpu`.
- **Determinism:** instance order must be defined.
- **Builds on:** [SAH BVH](#sah-bvh).
- **Sources:** [Frostbite, shipping dynamic GI (TLAS)](https://advances.realtimerendering.com/s2024/content/EA-GIBS2/Apers_Advances-s2024_Shipping-Dynamic-GI.pdf).

#### Oriented bounding box BVH

- **Uses:** tighter bounds around long, thin or rotated geometry.
- **Notes:** Vitsas et al. (2023) convert an AABB BVH to oriented boxes on the GPU. DOBB-BVH (2025) shares one rotation per node, chosen from a fixed set, for an average 18.5% primary-ray speedup at 12.6% more build time. SOBB (2025) uses skewed boxes and converts about 2.6x faster than Vitsas.
- **Variants:** OBVH, DOBB-BVH, SOBB.
- **Repo fit:** `gpu`, `safe`.
- **Determinism:** rotation fitting must be deterministic.
- **Builds on:** [SAH BVH](#sah-bvh).
- **Sources:** [Vitsas et al.](https://diglib.eg.org/handle/10.1111/cgf14758), [DOBB-BVH](https://diglib.eg.org/handle/10.1111/cgf70220), [SOBB](https://diglib.eg.org/handle/10.1111/cgf70062).

## 14. Volumes, terrain and sparse grids

#### Quadtree terrain with CDLOD

- **Uses:** large terrains rendered from heightmaps.
- **Notes:** a quadtree over the heightmap chooses patch detail from distance. CDLOD (Strugar) adapts to terrain complexity, where clipmaps use fixed rings.
- **Variants:** geomipmapping; chunked LOD.
- **Repo fit:** `subsystem` (rendering).
- **Determinism:** affects rendering only.
- **Builds on:** [Quadtree and octree](#quadtree-and-octree).
- **Sources:** [Open world in the browser, clipmaps](https://app.cinevva.com/blog/2026-02-25-open-world-browser-part-06-clipmaps).

#### Geometry clipmaps

- **Uses:** terrain as nested regular grids centred on the viewer.
- **Notes:** Losasso and Hoppe (SIGGRAPH 2004): the grids are cached in video memory and refilled as the viewer moves, with no dependency tracking between patches. The GPU version (GPU Gems 2) moves almost all the work to the GPU.
- **Variants:** CPU and GPU versions.
- **Repo fit:** `subsystem`, `gpu`.
- **Determinism:** affects rendering only.
- **Builds on:** [Ring buffer](#ring-buffer) (toroidal updates).
- **Sources:** [Geometry clipmaps](https://hhoppe.com/proj/geomclipmap/), [GPU geometry clipmaps](https://hhoppe.com/proj/gpugcm/).

#### Concurrent binary tree

- **Uses:** adaptive tessellation on the GPU for terrain and planets; a GPU memory-pool manager.
- **Notes:** Dupuy (2020); Benyoub and Dupuy (HPG 2024) extend it from squares to any polygon mesh, mapping one triangle ("bisector") to each half-edge. The tree also manages a memory pool, so bisectors can be allocated and freed concurrently. Under 0.2 ms on console hardware, rendering planets from very coarse meshes.
- **Variants:** an implicit encoding; a memory-pool manager.
- **Repo fit:** `gpu`, `subsystem`.
- **Determinism:** affects rendering only.
- **Builds on:** [Half-edge mesh](#half-edge-mesh).
- **Sources:** [CBT for large-scale game components](https://arxiv.org/pdf/2407.02215), [Intel article](https://www.intel.com/content/www/us/en/developer/articles/technical/cbt-for-large-scale-game-components.html), [SIGGRAPH 2024 slides](https://advances.realtimerendering.com/s2024/content/Intel/large_scale_cbt_slides_siggraph_advances_2024.pdf).

#### Palette-compressed voxel chunks

- **Uses:** Minecraft-style block worlds.
- **Notes:** each chunk keeps a palette of the block types it contains and stores bit-packed indices into the palette.
- **Variants:** the index width grows as the palette grows.
- **Repo fit:** `safe`.
- **Determinism:** palette order must be defined.
- **Builds on:** [Bitset](#bitset).
- **Sources:** none checked yet.

#### Sparse voxel octree

- **Uses:** voxel rendering and ray tracing; 3D flight navigation (see [3D flight navigation](#3d-flight-navigation)).
- **Notes:** subdivides only where there is geometry.
- **Variants:** efficient SVOs (Laine and Karras 2010).
- **Repo fit:** `safe`.
- **Determinism:** deterministic.
- **Builds on:** [Quadtree and octree](#quadtree-and-octree).
- **Sources:** [Game AI Pro 3, 3D flight navigation](https://www.gameaipro.com/GameAIPro3/GameAIPro3_Chapter21_3D_Flight_Navigation_Using_Sparse_Voxel_Octrees.pdf).

#### Sparse voxel DAG

- **Uses:** huge static voxel scenes.
- **Notes:** merges identical subtrees of a sparse voxel octree into a DAG (Kämpe et al. 2013). Aokana (2025) adds LOD and streaming for open worlds: up to 9x less memory and up to 4.8x faster rendering than earlier methods, with 8–40 bytes per node.
- **Variants:** symmetry-aware DAGs; DAGs with attributes; [HashDAG](#hashdag).
- **Repo fit:** `safe` (build), `gpu` (render).
- **Determinism:** subtree merging must be deterministic.
- **Builds on:** [Sparse voxel octree](#sparse-voxel-octree).
- **Sources:** [Aokana](https://arxiv.org/abs/2505.02017v1), [Aokana review](https://www.themoonlight.io/review/aokana-a-gpu-driven-voxel-rendering-framework-for-open-world-games).

#### HashDAG

- **Uses:** voxel scenes that are compressed and still editable.
- **Notes:** Careil, Billeter and Eisemann (2020): carving, filling, copying and painting work directly on the compressed DAG, which also stores compressed attributes such as colour.
- **Variants:** none noted.
- **Repo fit:** `safe`.
- **Determinism:** deterministic.
- **Builds on:** [Sparse voxel DAG](#sparse-voxel-dag).
- **Sources:** [Interactively modifying compressed sparse voxel representations](https://diglib.eg.org/handle/10.1111/cgf13916).

#### VDB and NanoVDB

- **Uses:** smoke, fluids, clouds and other sparse volumes.
- **Notes:** VDB (Museth 2013) is a shallow, wide tree of grid blocks. NanoVDB is a GPU-friendly version that does not depend on OpenVDB at runtime. It does not use RTX BVH traversal, but it is fast on sparse grids.
- **Variants:** OpenVDB, NanoVDB, GVDB.
- **Repo fit:** `safe`, `gpu`.
- **Determinism:** deterministic.
- **Builds on:** [B-tree](#b-tree).
- **Sources:** [NanoVDB](https://research.nvidia.com/labs/prl/publication/nanovdb), [Museth, VDB](https://research.dreamworks.com/wp-content/uploads/2018/08/Museth_TOG13-Edited.pdf).

#### Brickmap

- **Uses:** sparse volumes on the GPU; global illumination data.
- **Notes:** the grid is split into fixed-size blocks ("bricks"), and only blocks that contain data are stored. RenderMan's brickmap stores global data this way.
- **Variants:** GVDB.
- **Repo fit:** `gpu`.
- **Determinism:** affects rendering only.
- **Builds on:** [Uniform grid](#uniform-grid).
- **Sources:** [NanoVDB paper](https://research.nvidia.com/labs/prl/nanovdb/nanovdb2021.pdf).

#### SPGrid

- **Uses:** sparse simulation grids: smoke, material point method (MPM).
- **Notes:** Setaluri et al. (2014) use the CPU's virtual-memory hardware to get close to dense-grid bandwidth from a pyramid of sparse uniform grids. Limited by main memory. A 2025 paper argues SPGrid-like and VDB-like designs do not suit GPUs and proposes a hierarchical B+ tree instead.
- **Variants:** none noted.
- **Repo fit:** `os`.
- **Determinism:** deterministic.
- **Builds on:** [Virtual memory arena](#virtual-memory-arena).
- **Sources:** [SPGrid](https://pages.cs.wisc.edu/~sifakis/project_pages/SPGrid.html), [Contiguous grid storage](https://arxiv.org/html/2512.11473v1).

#### Hierarchical sparse grid

- **Uses:** sparse simulation data described as a composition of grid levels (Taichi's SNodes).
- **Notes:** dense, bitmasked (one bit per child marks it active), pointer, and dynamic (list) nodes compose into trees, for example 16 pointers to 8 bitmasked blocks of 4 dense cells. Taichi's documentation is inconsistent about which node kinds remain supported.
- **Variants:** dense, bitmasked, pointer, dynamic.
- **Repo fit:** `safe`.
- **Determinism:** deterministic.
- **Builds on:** [Bitset](#bitset).
- **Sources:** [Taichi spatially sparse data structures](https://docs.taichi-lang.org/docs/sparse), [AsyncTaichi](https://arxiv.org/pdf/2012.08141).

#### Virtual texture page table

- **Uses:** streaming texture data in tiles.
- **Notes:** a page table (an indirection texture) maps virtual tiles to their place in a physical cache. A low-resolution feedback pass reports which tiles the view needs. The page table grows with the number of virtual tiles.
- **Variants:** feedback written to a texture or to a buffer with atomics.
- **Repo fit:** `gpu`, `subsystem`.
- **Determinism:** affects rendering only.
- **Builds on:** [LRU and CLOCK](#lru-and-clock).
- **Sources:** [EPFL virtual texturing report](https://infoscience.epfl.ch/record/208803/files/Matthieu_Rudelle_semester_project_final_report.pdf?version=1).

#### Virtual shadow maps

- **Uses:** high-resolution shadows with memory allocated on demand.
- **Notes:** Unity's VSM uses a virtual page table plus an inverse physical page table, with 128x128-texel pages. Blender EEVEE shares one physical atlas across every shadow map, allocates pages entirely on the GPU, and collapses three levels of indirection into one.
- **Variants:** sparse VSMs (StratusGFX).
- **Repo fit:** `gpu`, `subsystem`.
- **Determinism:** affects rendering only.
- **Builds on:** [Virtual texture page table](#virtual-texture-page-table).
- **Sources:** [Blender EEVEE shadows](https://developer.blender.org/docs/features/eevee/modules/shadow/), [UnityVSM](https://github.com/octoon/UnityVSM), [StratusGFX SVSM](https://ktstephano.github.io/rendering/stratusgfx/svsm).

#### Hashed world-space cache

- **Uses:** lighting caches stored sparsely in world space; neural encodings.
- **Notes:** Split Radiance Cascades (2026) stores probes sparsely in a hash map, only near visible surfaces. Multi-resolution hash grids (Instant NGP) feed neural networks (not checked).
- **Variants:** spatially hashed radiance caches; multi-resolution hash encoding.
- **Repo fit:** `gpu`.
- **Determinism:** affects rendering only.
- **Builds on:** [GPU hash table](#gpu-hash-table).
- **Sources:** [Split Radiance Cascades](https://arxiv.org/pdf/2607.20384).

## 15. Graphs and graph algorithms

#### Compressed sparse row graph

- **Uses:** static graphs: navigation graphs, asset dependencies, mesh adjacency.
- **Notes:** all the edge lists in one array, with an offset array giving where each node's edges start.
- **Variants:** edge lists sorted by target.
- **Repo fit:** `safe`.
- **Determinism:** deterministic.
- **Builds on:** nothing.
- **Sources:** none checked yet.

#### DAG with topological sort

- **Uses:** system schedules (M7), render graphs, asset builds, audio mixer graphs.
- **Notes:** Kahn's algorithm or depth-first search; detects cycles.
- **Variants:** incremental topological order.
- **Repo fit:** `safe`.
- **Determinism:** ties must be broken by a stable key (such as registration order), never by hash order.
- **Builds on:** [Compressed sparse row graph](#compressed-sparse-row-graph).
- **Sources:** [FrameGraph slides](https://www.slideshare.net/DICEStudio/framegraph-extensible-rendering-architecture-in-frostbite).

#### Union-find

- **Uses:** physics islands, connected regions, merging clusters.
- **Notes:** with path compression it runs in nearly linear time. Jorrit Rouwe presented a parallel union-find at GDC 2022, and Jolt implements it. Box2D notes that island links are deterministic as long as contacts are created in a deterministic order.
- **Variants:** path halving; union by rank or size; parallel union-find.
- **Repo fit:** `safe`.
- **Determinism:** the order of unions must be defined, and the representative chosen by a rule such as the smaller ID.
- **Builds on:** nothing.
- **Sources:** [Catto, simulation islands](https://box2d.org/posts/2023/10/simulation-islands/), [Avian islands](https://idanarye.github.io/bevy-tnua/src/avian3d/dynamics/solver/islands/mod.rs.html).

#### Dynamic connectivity

- **Uses:** splitting islands or regions when edges are removed, without a full rebuild.
- **Notes:** HDT (Holm, de Lichtenberg, Thorup) updates in O(log² n) amortised time, but in practice it can need more than 70x the memory of the graph. A 2024 batch-dynamic algorithm is up to 6.2x faster than HDT's. Euler-tour trees and link-cut trees are the building blocks, and no single one wins in practice. Box2D v3 sidesteps the problem by splitting at most one island per step.
- **Variants:** HDT; Euler-tour trees; link-cut trees; batch-dynamic.
- **Repo fit:** `safe`.
- **Determinism:** deterministic for a given edge order.
- **Builds on:** [Union-find](#union-find).
- **Sources:** [Batch-dynamic connectivity (2024)](https://arxiv.org/pdf/2411.11781), [Experimental comparison of dynamic connectivity trees](https://www.themoonlight.io/review/an-experimental-comparison-of-tree-data-structures-for-connectivity-queries-on-fully-dynamic-undirected-graphs-extended-version).

#### Graph colouring

- **Uses:** solving physics constraints in parallel (no two constraints of one colour share a body), and parallel scheduling.
- **Notes:** Box2D v3 colours its constraint graph and uses islands only for sleeping. Colouring is what lets it use many threads on big piles of bodies, and it speeds up the AVX2 contact solver. See [Solver sets](#solver-sets).
- **Variants:** greedy colouring; Jones-Plassmann parallel colouring; a fixed number of colours plus an overflow set.
- **Repo fit:** `safe`.
- **Determinism:** the visit order must be defined.
- **Builds on:** [Compressed sparse row graph](#compressed-sparse-row-graph).
- **Sources:** [Releasing Box2D 3.0](https://box2d.org/posts/2024/08/releasing-box2d-3.0), [Box2D constraint graph source](https://docs.rs/crate/box2d_sys/0.2.1/source/box2d/src/constraint_graph.c).

#### Shortest-path research

- **Uses:** research (STOC 2025 best paper).
- **Notes:** Duan, Mao, Mao, Shu and Yin give a deterministic O(m log^(2/3) n) algorithm for single-source shortest paths on directed graphs with non-negative real weights. It is the first to beat Dijkstra's O(m + n log n) on sparse graphs. Not practical yet; tracked for awareness, together with [Working-set heap](#working-set-heap).
- **Variants:** none noted.
- **Repo fit:** `theory`.
- **Determinism:** not applicable.
- **Builds on:** nothing.
- **Sources:** [Breaking the sorting barrier](https://arxiv.org/abs/2504.17033), [MPI news](https://www.mpi-inf.mpg.de/news/detail/stoc-best-paper-award-how-to-find-the-shortest-path-faster).

#### Contraction hierarchies

- **Uses:** very fast queries on large static graphs (road-like world maps, travel between regions).
- **Notes:** Geisberger et al. (2008) remove nodes in order of importance and add shortcut edges that preserve shortest distances. A query is a modified bidirectional Dijkstra that touches a few hundred nodes, and the result needs less space than the input graph. Parallel builds exist (2024).
- **Variants:** customisable CH; parallel CH.
- **Repo fit:** `safe`.
- **Determinism:** the contraction order must be defined.
- **Builds on:** [Compressed sparse row graph](#compressed-sparse-row-graph), [Indexed heap with decrease-key](#indexed-heap-with-decrease-key).
- **Sources:** [Geisberger et al.](https://researchr.org/publication/GeisbergerSSD08), [Parallel contraction hierarchies](https://arxiv.org/pdf/2412.18008).

## 16. Concurrency and synchronization

Every entry in this section needs `unsafe` or an external crate, and threading belongs to roadmap M7. The engine runs on threads the host provides ([ADR 0007](../adr/0007-host-owns-the-loop.md)), and the repo has a `concurrency-allowlist.toml`.

#### SPSC ring

- **Uses:** single-producer, single-consumer queues: feeding the audio thread, render submission, game thread to worker thread.
- **Notes:** the standard pattern between the audio thread and the rest of an application (JACK, PortAudio and CoreAudio all use it). A general multi-producer queue is unnecessary with exactly one writer and one reader. When the queue is full, drop the newest update rather than block the audio thread.
- **Variants:** cached head and tail indices; batched push and pop.
- **Repo fit:** `unsafe`.
- **Determinism:** with one producer and one consumer, order is deterministic. When a message is consumed must not affect simulation results.
- **Builds on:** [Ring buffer](#ring-buffer).
- **Sources:** [Lock-free audio queues](https://omr.it.com/blog/lock-free-audio-queues-real-time-dsp-threading/), [SPSC ring notes](https://0xkiire.pages.dev/lock-free-spsc-ring-buffer).

#### Bounded MPMC queue

- **Uses:** job queues with many producers and many consumers.
- **Notes:** Vyukov's design gives each slot a sequence counter, so producers and consumers can tell when a slot is free or ready without a global lock.
- **Variants:** rigtorp's MPMCQueue; FAA-based queues.
- **Repo fit:** `unsafe`.
- **Determinism:** the order items are consumed is not deterministic, so results must be collected and then sorted.
- **Builds on:** [Ring buffer](#ring-buffer).
- **Sources:** [Lock-free audio queues](https://omr.it.com/blog/lock-free-audio-queues-real-time-dsp-threading/).

#### Disruptor

- **Uses:** pipelines with several stages between threads.
- **Notes:** LMAX's ring buffer, coordinated by sequence counters ("sequencers") and barriers. It is not a drop-in replacement for queues.
- **Variants:** none noted.
- **Repo fit:** `unsafe`.
- **Determinism:** order within each stage is deterministic.
- **Builds on:** [Ring buffer](#ring-buffer).
- **Sources:** [Scott Logic, the Disruptor](https://blog.scottlogic.com/2021/12/01/disruptor.html), [Trisha Gee, QCon 2012](https://qconlondon.com/london-2012/qconlondon.com/dl/qcon-london-2012/slides/TrishaGee_ConcurrentProgrammingUsingTheDisruptor.pdf).

#### Chase-Lev deque

- **Uses:** work stealing in job systems (Unity jobs, Unreal's task graph, Bevy).
- **Notes:** each worker pushes and pops its own tasks at one end, and other threads steal from the other end. Only the owner may push and pop: one write-up lost tasks intermittently by handing out fibers round-robin across workers. Formally verified in 2023.
- **Variants:** ABP deque; a deque that grows.
- **Repo fit:** `unsafe`.
- **Determinism:** which thread runs a task varies, so task results must be merged in a defined order.
- **Builds on:** [Deque](#deque).
- **Sources:** [Formal verification of the Chase-Lev deque](https://arxiv.org/pdf/2309.03642), [Fiber job system write-up](https://wc-0904.github.io/).

#### Fiber job system with counters

- **Uses:** job dependencies without blocking OS threads (Naughty Dog's PS4 engine design).
- **Notes:** M lightweight fibers run on N OS threads, and waiting for a counter to reach zero expresses a dependency. One write-up says Naughty Dog used a single shared priority queue rather than work stealing.
- **Variants:** with or without work stealing.
- **Repo fit:** `unsafe`.
- **Determinism:** as for [Chase-Lev deque](#chase-lev-deque).
- **Builds on:** [Chase-Lev deque](#chase-lev-deque).
- **Sources:** [Fiber job system write-up](https://wc-0904.github.io/).

#### Triple buffer

- **Uses:** handing the latest value from one thread to another, where only the newest matters: simulation state to the renderer, audio parameters.
- **Notes:** three slots, so neither side ever waits. The reader always gets the latest complete value.
- **Variants:** none noted.
- **Repo fit:** `unsafe`.
- **Determinism:** which frame the reader sees varies, so only output (never simulation) may read it.
- **Builds on:** nothing.
- **Sources:** none checked yet.

#### Seqlock

- **Uses:** tiny shared state that is read often and written rarely.
- **Notes:** a writer increments a sequence number before and after writing, and a reader retries if the number changed while it read.
- **Variants:** none noted.
- **Repo fit:** `unsafe`.
- **Determinism:** as for [Triple buffer](#triple-buffer).
- **Builds on:** nothing.
- **Sources:** none checked yet.

#### Left-right

- **Uses:** data read far more often than it is written.
- **Notes:** Ramalhete and Correia (DISC 2015): two copies, so reads are wait-free while the single writer updates the other copy and then swaps. It costs twice the memory, writes block (but never interfere with reads), and more than one writer needs an external lock. Rust's `left-right` crate (evmap) implements it.
- **Variants:** none noted.
- **Repo fit:** `unsafe`.
- **Determinism:** readers may see the old copy.
- **Builds on:** nothing.
- **Sources:** [Left-right paper](https://lag.net/papers/content/leftright-extended.pdf), [left-right crate](https://docs.rs/crate/left-right/0.9.1).

#### Epoch-based reclamation

- **Uses:** freeing memory safely inside lock-free structures.
- **Notes:** memory is freed only once every thread has passed a later epoch. RCU (read-copy-update) applies the same idea to data that is read far more than written; its C++26 proposal is P2545.
- **Variants:** crossbeam-epoch; seize; quiescent-state-based reclamation (QSBR).
- **Repo fit:** `unsafe`.
- **Determinism:** affects only when memory is freed.
- **Builds on:** nothing.
- **Sources:** [P2545 RCU](https://www.open-std.org/JTC1/SC22/WG21/docs/papers/2023/p2545r2.pdf), [papaya](https://github.com/ibraheemdev/papaya).

#### Hazard pointers

- **Uses:** freeing memory safely with bounded memory overhead.
- **Notes:** each reader announces the pointer it is using, and nothing it announced is freed. Hazard pointers were added to the C++26 working draft (P2530R3). Proposals for synchronous reclamation (P3427) and batches (P3428) followed.
- **Variants:** batched hazard pointers.
- **Repo fit:** `unsafe`.
- **Determinism:** affects only when memory is freed.
- **Builds on:** nothing.
- **Sources:** [P2530 hazard pointers](https://open-std.org/jtc1/sc22/wg21/docs/papers/2023/p2530r3.pdf), [C++26 hazard pointers](https://enna1.github.io/blog/hazard-pointers).

#### Flat combining

- **Uses:** structures with heavy contention (stacks, queues, priority queues).
- **Notes:** Hendler, Incze, Shavit and Tzafrir (SPAA 2010): whichever thread holds the lock applies every other thread's pending requests. Their FIFO queue beat the fastest known concurrent queues at almost every level of concurrency, by more than 4x under high contention. A parallel version adds combiners as contention rises.
- **Variants:** parallel flat combining.
- **Repo fit:** `unsafe`.
- **Determinism:** the order requests are combined in is not deterministic.
- **Builds on:** nothing.
- **Sources:** [Flat combining (SPAA 2010)](https://people.csail.mit.edu/shanir/publications/Flat%20Combining%20SPAA%2010.pdf).

#### Atomic append buffer

- **Uses:** collecting output from parallel work, such as visible objects or contact pairs.
- **Notes:** threads reserve slots by atomically incrementing a counter.
- **Variants:** one buffer per thread, joined afterwards.
- **Repo fit:** `unsafe`.
- **Determinism:** the order of output is not deterministic, so sort it (or use per-thread buffers joined in thread-index order).
- **Builds on:** nothing.
- **Sources:** none checked yet.

## 17. GPU primitives and structures

#### Decoupled look-back prefix sum

- **Uses:** the building block for GPU compaction, allocation and sorting.
- **Notes:** Merrill and Garland (2016): a scan in a single pass. Each block publishes its local sum and its prefix with a status flag, and a block only waits for the predecessors it needs. It reduces memory traffic from 3N to 2N per pass.
- **Variants:** reduce-then-scan; chained scan.
- **Repo fit:** `gpu`.
- **Determinism:** integer scans are deterministic; floating-point scans need a fixed order.
- **Builds on:** nothing.
- **Sources:** [Onesweep](https://arxiv.org/abs/2206.01784), [NVIDIA GTC 2020 radix sort](https://developer.download.nvidia.com/video/gputechconf/gtc/2020/presentations/s21572-a-faster-radix-sort-implementation.pdf).

#### Onesweep radix sort

- **Uses:** sorting draw keys, Gaussian splats and particles; building LBVHs.
- **Notes:** Adinets and Merrill (2022): an LSD radix sort built on decoupled look-back, about 1.5x faster than CUB (29.4 billion keys per second on an A100 for 256M 32-bit keys). GPUOpen added a circular-buffer version that uses less memory. It runs on most desktop GPUs but has portability caveats on mobile.
- **Variants:** DeviceRadixSort (reduce-then-scan); ShaderOneSweep (HLSL).
- **Repo fit:** `gpu`.
- **Determinism:** a stable sort is deterministic.
- **Builds on:** [Decoupled look-back prefix sum](#decoupled-look-back-prefix-sum).
- **Sources:** [Onesweep](https://arxiv.org/abs/2206.01784), [GPUOpen Onesweep extension](https://gpuopen.com/learn/boosting_gpu_radix_sort/), [GPUSorting](https://www.github.com/b0nes164/GPUSorting).

#### GPU hash table

- **Uses:** deduplication and spatial hashing on the GPU.
- **Notes:** WarpCore reports up to 1.6 billion inserts and 4.3 billion lookups per second on a V100, and includes a compact bucket-list table that can grow. BGHT's tables are static, and bucketed cuckoo is the fastest of them. Buckets sized to cache lines drive performance.
- **Variants:** WarpCore, BGHT, SlabHash, compact cuckoo.
- **Repo fit:** `gpu`.
- **Determinism:** atomics make positions nondeterministic, so do not iterate it.
- **Builds on:** [Cuckoo hashing](#cuckoo-hashing).
- **Sources:** [WarpCore](https://arxiv.org/abs/2009.07914v1), [BGHT](https://awesome.ecosyste.ms/projects/github.com%2Fowensgroup%2FBGHT), [Compact parallel hash tables](https://research.tue.nl/en/publications/compact-parallel-hash-tables-onthegpu/).

#### Work graphs

- **Uses:** producer-consumer pipelines that run entirely on the GPU: deferred shading, streaming, culling.
- **Notes:** D3D12 Work Graphs (2024). Nodes send records to other nodes, which launches work; dependencies are per record, not resource barriers, so no full flush is needed. There is scheduling overhead, deadlock is possible, and a node may not send work back to a node it received records from. Needs Work Graphs 1.0 support.
- **Variants:** persistent-thread work queues (the older software approach).
- **Repo fit:** `gpu`.
- **Determinism:** affects rendering only.
- **Builds on:** [DAG with topological sort](#dag-with-topological-sort).
- **Sources:** [D3D12 Work Graphs spec](https://microsoft.github.io/DirectX-Specs/d3d/WorkGraphs.html), [DirectX blog](https://devblogs.microsoft.com/directx/d3d12-work-graphs/), [NVIDIA, deferred shading case study](https://developer.nvidia.com/blog/work-graphs-in-direct3d-12-a-case-study-of-deferred-shading), [GPUOpen Work Graph Playground](https://54ab3db1-c909-41ca-90c6-1ee199732964.proxy.brevy.com/GPUOpen-LibrariesAndSDKs/WorkGraphPlayground).

#### Indirect argument buffer

- **Uses:** GPU-driven rendering, in which the GPU writes its own draw and dispatch counts.
- **Notes:** a compaction pass writes the surviving items and then writes the draw arguments, so the CPU never reads the counts back.
- **Variants:** multi-draw indirect.
- **Repo fit:** `gpu`.
- **Determinism:** affects rendering only.
- **Builds on:** [Decoupled look-back prefix sum](#decoupled-look-back-prefix-sum).
- **Sources:** [Wicked Engine, GPU particles](https://wickedengine.net/2017/11/gpu-based-particle-simulation).

## 18. Rendering and lighting

#### Render graph

- **Uses:** ordering render passes, inserting barriers automatically, reusing transient memory.
- **Notes:** Frostbite's FrameGraph (O'Donnell, GDC 2017). Each pass declares every resource it reads, writes or creates. The frame is set up, compiled (unused passes are dropped and the rest sorted topologically) and then executed. Short-lived (transient) resources share memory, much like register allocation. Granite's render graph follows the same design.
- **Variants:** creating a resource only when it is first used.
- **Repo fit:** `subsystem` (rendering), `safe` (the graph itself).
- **Determinism:** pass order must be a function of the declarations.
- **Builds on:** [DAG with topological sort](#dag-with-topological-sort).
- **Sources:** [FrameGraph (GDC Vault)](https://gdcvault.com/play/1024612/FrameGraph-Extensible-Rendering-Architecture-in), [FrameGraph slides](https://www.slideshare.net/DICEStudio/framegraph-extensible-rendering-architecture-in-frostbite), [Themaister, render graphs](https://themaister.net/blog/2017/08/).

#### Draw sort key

- **Uses:** ordering draw calls to reduce state changes, and ordering transparent objects by depth.
- **Notes:** each draw's state is packed into a 64-bit key with the most important fields in the high bits. The (key, index) pairs are radix-sorted, and the draw data itself never moves. Field layout varies by engine.
- **Variants:** buckets per pass; depth-first keys for transparency.
- **Repo fit:** `safe`, `subsystem`.
- **Determinism:** equal keys must be broken by index.
- **Builds on:** [Onesweep radix sort](#onesweep-radix-sort).
- **Sources:** [Ericson, draw call keys](https://realtimecollisiondetection.net/blog/?p=86), [Stingray render contexts](https://bitsquid.blogspot.com/2017/02/stingray-renderer-walkthrough-3-render.html?m=0), [Flax RenderList](https://git.flaxengine.com/Flax/FlaxEngine/raw/branch/master/Source/Engine/Renderer/RenderList.h).

#### Command packet buffer

- **Uses:** rendering without hidden state: commands are recorded with their keys and translated to API calls later.
- **Notes:** one buffer per thread; sorted, then submitted.
- **Variants:** none noted.
- **Repo fit:** `subsystem`.
- **Determinism:** as for [Draw sort key](#draw-sort-key).
- **Builds on:** [Draw sort key](#draw-sort-key).
- **Sources:** [Stingray render contexts](https://bitsquid.blogspot.com/2017/02/stingray-renderer-walkthrough-3-render.html?m=0).

#### Clustered light grid

- **Uses:** rendering many lights.
- **Notes:** a 3D grid over the view frustum ("froxels"). Each cell stores an offset and a light count into one compact index list, which is built with a prefix sum. Persson's Avalanche layout keeps the per-cell entries in a 3D texture and the indices in a packed 16-bit buffer.
- **Variants:** fixed capacity per cell; a bitmask per cell; light trees, which can beat clustering in some scenes.
- **Repo fit:** `gpu`, `subsystem`.
- **Determinism:** affects rendering only.
- **Builds on:** [Decoupled look-back prefix sum](#decoupled-look-back-prefix-sum).
- **Sources:** [Persson, practical clustered shading](https://www.humus.name/Articles/PracticalClusteredShading.pdf), [Tiled light trees](https://www.kayru.org/publications/TiledLightTrees-preprint.pdf), [clustered shading tutorial](https://github.com/DaveH355/clustered-shading).

#### Light BVH

- **Uses:** choosing which of many lights to sample, in proportion to their likely contribution.
- **Notes:** Moreau et al. (2019) built a dynamic light BVH. It has to be rebuilt as lights change, which ReSTIR avoids. A hybrid uses the BVH to pick candidates for ReSTIR (a research direction).
- **Variants:** none noted.
- **Repo fit:** `gpu`.
- **Determinism:** affects rendering only.
- **Builds on:** [SAH BVH](#sah-bvh).
- **Sources:** [TU Delft thesis on ReSTIR and light BVHs](https://repository.tudelft.nl/file/File_1a4a2809-8a2d-469b-8c92-317a0b44aa6f?preview=1).

#### ReSTIR reservoirs

- **Uses:** sampling direct light from many lights by reusing samples across neighbouring pixels and frames.
- **Notes:** built on weighted reservoir sampling, with a small fixed-size reservoir per pixel and nothing more complicated than fixed-size arrays. The first prototype took 10 to 50 ms per frame; a production version cut lighting cost by up to 7x. ReGIR stores reservoirs in a world-space grid instead.
- **Variants:** ReSTIR DI, GI and PT; ReGIR.
- **Repo fit:** `gpu`.
- **Determinism:** affects rendering only.
- **Builds on:** [Uniform grid](#uniform-grid) (ReGIR).
- **Sources:** [ReSTIR](https://cdn.pharr.org/ReSTIR.pdf), [ReGIR](https://boksajak.github.io/files/RTG2_ReGIRGridBasedReservoirs.pdf).

#### Irradiance probe grid

- **Uses:** dynamic diffuse global illumination (DDGI).
- **Notes:** a uniform grid of probes updated by ray tracing (Majercik et al. 2019). Probes store irradiance and depth in octahedral maps rather than spherical harmonics, which are hard to update. Frostbite's 2024 probe system borrows this layout.
- **Variants:** probes that move or switch off by themselves.
- **Repo fit:** `gpu`.
- **Determinism:** affects rendering only.
- **Builds on:** [Uniform grid](#uniform-grid).
- **Sources:** [Frostbite, shipping dynamic GI](https://advances.realtimerendering.com/s2024/content/EA-GIBS2/Apers_Advances-s2024_Shipping-Dynamic-GI.pdf), [DDGI evaluation](https://cescg.org/wp-content/uploads/2022/04/Hvězda-Dynamic-Diffuse-Global-Illumination-Implementation-and-Evaluation-1.pdf).

#### Surfel cache

- **Uses:** global illumination that caches lighting on small surface patches (surfels), created as geometry comes into view.
- **Notes:** EA SEED's GIBS (2018, now part of Frostbite). Its acceleration structure gives more resolution near the camera, like a projection, and works for scenes of any size with predictable cost and memory.
- **Variants:** none noted.
- **Repo fit:** `gpu`.
- **Determinism:** affects rendering only.
- **Builds on:** [Spatial hash grid](#spatial-hash-grid).
- **Sources:** [SIGGRAPH 2021, surfel GI](https://www.advances.realtimerendering.com/s2021/SIGGRAPH%20Advances%202021%20-%20Surfel%20GI.pdf), [EA SEED](https://www.ea.com/seed/news/siggraph21-global-illumination-surfels).

#### Radiance cascades

- **Uses:** real-time global illumination, especially in 2D.
- **Notes:** Sannikov's method uses a stack of probe grids. Each level has half the spatial resolution and four times the angular resolution of the one below, and covers a further range of ray distances. Holographic RC (2025) runs at constant cost (1.85 ms at 512x512), but its memory use limits a move to 3D. Split RC (2026) brings it to 3D with probes stored sparsely in a hash map near visible surfaces, for diffuse light only.
- **Variants:** standard, holographic, split.
- **Repo fit:** `gpu`.
- **Determinism:** affects rendering only.
- **Builds on:** [Hashed world-space cache](#hashed-world-space-cache).
- **Sources:** [Holographic radiance cascades](https://arxiv.org/abs/2505.02041), [Split radiance cascades](https://arxiv.org/pdf/2607.20384).

#### Hierarchical Z-buffer

- **Uses:** GPU occlusion culling.
- **Notes:** a depth mip chain that keeps the farthest depth at each level. A test picks the mip level where the object's screen rectangle covers about 2x2 texels, which needs four reads. The two-pass method renders last frame's visible set first to build depth. Building the chain is the main cost.
- **Variants:** single-pass downsampling (Bevy); two-pass culling.
- **Repo fit:** `gpu`.
- **Determinism:** affects rendering only.
- **Builds on:** nothing.
- **Sources:** [Kitware, WebGPU occlusion culling](https://kitware.com/webgpu-occlusion-culling-in-vtk), [Bevy mip generation](https://doc.qu1x.dev/bevy_trackball/src/bevy_core_pipeline/experimental/mip_generation/mod.rs.html).

#### Order-independent transparency buffer

- **Uses:** transparency without sorting.
- **Notes:** an A-buffer (Carpenter 1984) or per-pixel linked lists (built on the GPU with atomics) are exact but heavy on memory. Weighted blended OIT (McGuire and Bavoil 2013) is one pass, approximate, and runs on WebGL-class hardware. Dual depth peeling sits in between.
- **Variants:** A-buffer, per-pixel linked lists, k-buffer, weighted blended, depth peeling.
- **Repo fit:** `gpu`.
- **Determinism:** affects rendering only.
- **Builds on:** [Atomic append buffer](#atomic-append-buffer).
- **Sources:** [McGuire and Bavoil](https://www.jcgt.org/published/0002/02/09/paper.pdf), [NVIDIA OIT](https://on-demand.gputechconf.com/gtc/2014/presentations/S4385-order-independent-transparency-opengl.pdf), [MJP, weighted blended OIT](https://therealmjp.github.io/posts/weighted-blended-oit).

#### Bindless descriptor table

- **Uses:** binding resources by index on GPUs that support it ("bindless").
- **Notes:** one large table of descriptors whose slots come from a free list.
- **Variants:** none noted.
- **Repo fit:** `gpu`.
- **Determinism:** affects rendering only.
- **Builds on:** [Free-list array](#free-list-array).
- **Sources:** none checked yet.

#### Pipeline cache

- **Uses:** reusing compiled GPU pipeline objects.
- **Notes:** keyed by a hash of the render state.
- **Variants:** a cache saved to disk.
- **Repo fit:** `subsystem`.
- **Determinism:** affects rendering only.
- **Builds on:** [Insertion-ordered map](#insertion-ordered-map).
- **Sources:** none checked yet.

#### Texture atlas packer

- **Uses:** UI images, glyphs, lightmaps, sprite sheets.
- **Notes:** shelf packing (etagere) works best when many items have similar sizes, such as glyphs, and frees space reasonably fast; it is not the densest packing. guillotière uses guillotine packing. etagere's bucketed allocator reclaims a bucket only once all of its items are freed.
- **Variants:** shelf, guillotine, skyline, MaxRects; a glyph cache with eviction.
- **Repo fit:** `safe`.
- **Determinism:** placement depends on the order items are added, so that order must be fixed.
- **Builds on:** nothing.
- **Sources:** [etagere](https://github.com/hecrj/etagere), [etagere docs](https://docs.rs/crate/etagere/latest).

#### Visibility buffer

- **Uses:** deferred rendering that stores which triangle covers each pixel instead of storing materials (Nanite).
- **Notes:** 64 bits per pixel: depth plus a cluster and triangle ID.
- **Variants:** none noted.
- **Repo fit:** `gpu`.
- **Determinism:** affects rendering only.
- **Builds on:** [Cluster DAG](#cluster-dag).
- **Sources:** [Nanite-style DAG paper](https://diglib7.eg.org/bitstream/handle/10.1111/cgf14868/v42i8_04_14868.pdf).

#### Neural texture compression

- **Uses:** much smaller material textures that can still be read at any texel.
- **Notes:** NVIDIA (SIGGRAPH 2023) compresses all of a material's textures and mip levels together, with a small network trained per material to decode them. It gives 16x the texels of block compression at similar memory. Each material needs its own trained network, and every channel is always decoded.
- **Variants:** RTX NTC SDK.
- **Repo fit:** `gpu`.
- **Determinism:** affects rendering only.
- **Builds on:** nothing.
- **Sources:** [Random-access neural compression of material textures](https://arxiv.org/pdf/2305.17105), [NVIDIA project page](https://research.nvidia.com/labs/rtr/neural_texture_compression/), [Tom's Hardware](https://www.tomshardware.com/news/nvidia-uses-neural-network-for-innovative-texture-compression-method).

#### Gaussian splat storage

- **Uses:** rendering captured scenes stored as 3D Gaussian splats.
- **Notes:** hierarchical or octree LOD (Octree-GS 2024; NanoGaussianSplats for UE5), plus a radix sort on the GPU, or stochastic rendering that needs no sort. Split large captures into chunks so not every splat is sorted every frame.
- **Variants:** octree LOD; LOD pyramid; rendering without sorting.
- **Repo fit:** `gpu`.
- **Determinism:** affects rendering only.
- **Builds on:** [Onesweep radix sort](#onesweep-radix-sort), [Quadtree and octree](#quadtree-and-octree).
- **Sources:** [Octree-GS](https://arxiv.org/pdf/2403.17898), [NanoGaussianSplats](https://digitalproduction.com/2026/03/24/nanogaussiansplats-for-ue5/), [Cesium splat LOD](https://radiancefields.com/cesium-adds-hierarchical-lod-for-gaussian-splats-to-3d-tiles-cesiumjs-and-cesium-for-unreal).

#### Instance cluster tree

- **Uses:** foliage and large numbers of mesh instances (Unreal's hierarchical instanced static mesh).
- **Notes:** a tree over instance clusters culls instances and picks LODs per instance, where plain instancing shares one LOD across all of them. Older versions had one occlusion query for a million instances, and the tree has to be rebuilt when it is out of date.
- **Variants:** none noted.
- **Repo fit:** `subsystem`.
- **Determinism:** affects rendering only.
- **Builds on:** [SAH BVH](#sah-bvh).
- **Sources:** [Unreal forums, HISM components](https://forums.unrealengine.com/t/need-info-on-hierarchical-instanced-static-mesh-components/23811).

## 19. Geometry and meshes

#### Indexed triangle mesh

- **Uses:** rendering and collision meshes.
- **Notes:** a vertex array plus an index array. Triangle strips are an older, more compact form.
- **Variants:** triangle strips; quantised vertices.
- **Repo fit:** `safe`.
- **Determinism:** deterministic.
- **Builds on:** nothing.
- **Sources:** none checked yet.

#### Half-edge mesh

- **Uses:** editing meshes, building navmeshes, convex hulls, adaptive subdivision.
- **Notes:** each edge is stored as two directed halves with next, twin and face links, so walking around a vertex or a face is cheap.
- **Variants:** DCEL; winged edge; quad-edge; corner table.
- **Repo fit:** `safe`.
- **Determinism:** deterministic.
- **Builds on:** nothing.
- **Sources:** [CBT, half-edge bisectors](https://arxiv.org/pdf/2407.02215).

#### Meshlets

- **Uses:** mesh shaders, culling at cluster granularity.
- **Notes:** small clusters of triangles, often around 128 each.
- **Variants:** none noted.
- **Repo fit:** `safe` (build), `gpu` (use).
- **Determinism:** clustering must be deterministic.
- **Builds on:** [Indexed triangle mesh](#indexed-triangle-mesh).
- **Sources:** [Nanite-style DAG paper](https://diglib7.eg.org/bitstream/handle/10.1111/cgf14868/v42i8_04_14868.pdf).

#### Cluster DAG

- **Uses:** continuous level of detail for virtual geometry (Nanite, Bevy 0.16).
- **Notes:** clusters are grouped, simplified and split again, level by level, with error bounds stored for choosing LOD at runtime. At runtime the renderer selects a "cut" through the DAG; Nanite walks it with a BVH over cluster groups. The quality of the DAG is the hardest part: partitioning so groups share few vertices (for example with METIS) helps. meshoptimizer (2025) builds DAGs for billions of triangles in minutes.
- **Variants:** a BVH over cluster groups.
- **Repo fit:** `safe` (build), `gpu`.
- **Determinism:** partitioning and simplification must be deterministic.
- **Builds on:** [Meshlets](#meshlets).
- **Sources:** [Bevy 0.16 virtual geometry](https://jms55.github.io/posts/2025-03-27-virtual-geometry-bevy-0-16), [zeux, billions of triangles](https://zeux.io/2025/09/30/billions-of-triangles-in-minutes/), [Recreating Nanite](https://jglrxavpok.github.io/2023/11/12/recreating-nanite-the-plan.html).

#### Dense Geometry Format

- **Uses:** compressed geometry for ray tracing and rasterisation.
- **Notes:** AMD (2024): 128-byte blocks holding up to 64 vertices and 64 triangles. The mesh's connectivity is kept, and any triangle can be fetched in one memory read. A lossless second stage (DGF SuperCompression) shrinks it further. Hardware support is expected in future GPUs; AMD and Samsung are working on a Vulkan extension from several vendors.
- **Variants:** DGF, DGFS.
- **Repo fit:** `gpu`.
- **Determinism:** the encoder must be deterministic.
- **Builds on:** [Meshlets](#meshlets).
- **Sources:** [AMD DGF](https://gpuopen.com/learn/amd-dgf-an-open-geometry-compression-standard/), [Solving the dense geometry problem](https://gpuopen.com/learn/problem_increasing_triangle_density/).

#### Micromaps

- **Uses:** hardware ray tracing of alpha-tested surfaces (foliage, fences) and displaced detail.
- **Notes:** opacity micromaps record the opacity state of tiny sub-triangles, so the RT core can resolve many hits without running a shader (NVIDIA measured 1.5x in one test). Displaced micro-meshes encode about 4 bits per micro-triangle; NVIDIA claims up to 10x faster BVH builds and 20x less BVH memory. Hardware support started with Ada.
- **Variants:** OMM, DMM.
- **Repo fit:** `gpu`.
- **Determinism:** affects rendering only.
- **Builds on:** [Two-level BVH](#two-level-bvh).
- **Sources:** [NVIDIA micro-mesh](https://developer.nvidia.com/rtx/ray-tracing/micro-mesh), [OMM SDK](https://developer.nvidia.com/rtx/ray-tracing/opacity-micro-map/get-started), [Micro-mesh basics](https://developer.download.nvidia.com/ProGraphics/nvpro-samples/slides/Micro-Mesh_Basics.pdf).

#### Vertex cache ordering

- **Uses:** reordering indices and vertices so the GPU fetches and caches them efficiently.
- **Notes:** offline optimisation passes such as meshoptimizer's.
- **Variants:** vertex cache, overdraw and fetch optimisation.
- **Repo fit:** `safe`.
- **Determinism:** deterministic.
- **Builds on:** [Indexed triangle mesh](#indexed-triangle-mesh).
- **Sources:** none checked yet.

## 20. Animation

#### Skeleton as a parent-index array

- **Uses:** joint hierarchies.
- **Notes:** joints are stored parents first, so one forward pass computes the world pose. ACL stores each joint relative to its parent, and error builds up down the hierarchy.
- **Variants:** sorted by depth.
- **Repo fit:** `safe`.
- **Determinism:** deterministic.
- **Builds on:** nothing.
- **Sources:** [ACL, advanced quantization](https://nfrechette.github.io/2017/03/12/anim_compression_advanced_quantization).

#### SoA pose buffer

- **Uses:** blending poses with SIMD.
- **Notes:** translations, rotations and scales stored as separate arrays, several joints at a time.
- **Variants:** blocks four joints wide.
- **Repo fit:** `safe`.
- **Determinism:** floating-point operations must happen in a fixed order.
- **Builds on:** [Struct-of-arrays vector](#struct-of-arrays-vector).
- **Sources:** none checked yet.

#### Compressed animation clips

- **Uses:** storing animation tracks and curves.
- **Notes:** ACL measures error in object space and picks a bit rate per joint: joints near the root, such as the pelvis, need more bits. It is header-only C++, and Unreal ships a plugin for it with separate compression for bones and curves.
- **Variants:** keyframe curves (Hermite, Bezier); variable bit rate; handling for looping clips.
- **Repo fit:** `safe`, `subsystem`.
- **Determinism:** decompression must be bit-exact for a given build.
- **Builds on:** [Skeleton as a parent-index array](#skeleton-as-a-parent-index-array).
- **Sources:** [ACL, advanced quantization](https://nfrechette.github.io/2017/03/12/anim_compression_advanced_quantization), [ACL in Unreal](https://dev.epicgames.com/documentation/unreal-engine/animation-compression-library-in-unreal-engine), [Compressing looping clips](https://nfrechette.github.io/2022/04/03/anim_compression_looping).

#### Blend tree and animation state machine

- **Uses:** choosing and blending animations from game state.
- **Notes:** a graph of blend nodes driven by parameters, with a state machine for transitions.
- **Variants:** animation graphs; montages.
- **Repo fit:** `subsystem`.
- **Determinism:** evaluation order must be fixed.
- **Builds on:** [Hierarchical state machine](#hierarchical-state-machine).
- **Sources:** none checked yet.

#### Motion matching database

- **Uses:** choosing animation poses by searching motion-capture data, instead of hand-authored transitions.
- **Notes:** Clavet (Ubisoft, GDC 2016, For Honor) searches all the mocap data every frame for the best-matching pose. It needs large datasets and a lot of CPU. Learned versions came later (not checked).
- **Variants:** a feature database plus a search structure; learned motion matching.
- **Repo fit:** `subsystem`.
- **Determinism:** equal costs must be broken by index.
- **Builds on:** [k-d tree](#k-d-tree).
- **Sources:** [Motion matching and the road to next-gen animation](https://www.gdcvault.com/play/1022985/Motion-Matching-and-The-Road), [CGWorld coverage](https://cgworld.jp/feature/1604-gdc2016-03-2.html).

## 21. AI, navigation and gameplay

#### Grid graph with jump point search

- **Uses:** pathfinding on uniform-cost grids.
- **Notes:** Harabor and Grastien (AAAI 2011) skip ahead along straight lines and expand only "jump points", which speeds A* up by an order of magnitude or more. The original assumes diagonal moves may cut corners; rules for grids that forbid it followed in 2012.
- **Variants:** JPS, JPS+.
- **Repo fit:** `safe`.
- **Determinism:** ties must be broken by a fixed rule.
- **Builds on:** [Binary and d-ary heap](#binary-and-d-ary-heap).
- **Sources:** [Harabor and Grastien, online graph pruning](https://ojs.aaai.org/index.php/AAAI/article/view/7994).

#### Waypoint graph

- **Uses:** simple hand-placed navigation.
- **Notes:** nodes and edges placed by designers.
- **Variants:** none noted.
- **Repo fit:** `safe`.
- **Determinism:** ties must be broken by a fixed rule.
- **Builds on:** [Compressed sparse row graph](#compressed-sparse-row-graph).
- **Sources:** none checked yet.

#### Navigation mesh

- **Uses:** walking characters.
- **Notes:** Recast voxelises the level and partitions it (watershed) into walkable polygons, taking each agent's size and limits into account; Detour searches polygon corridors and smooths the path ("string pulling").
- **Variants:** tiled navmeshes; overlapping layers for floors and bridges.
- **Repo fit:** `subsystem`.
- **Determinism:** building must be deterministic, and ties broken in a fixed way.
- **Builds on:** [Half-edge mesh](#half-edge-mesh).
- **Sources:** [Multi-threaded Recast-based A*](https://arxiv.org/html/2602.04130v1), [AstraWeave navigation](https://lazyxeon.github.io/AstraWeave-AI-Native-Gaming-Engine/core-systems/navigation.html).

#### Hierarchical pathfinding

- **Uses:** large maps.
- **Notes:** HPA* (Botea, Müller, Schaeffer 2004) splits the grid into clusters joined at entrances, giving paths within about 1% of optimal up to 10x faster than A*. HNA* (Pelechano and Fuentes) does the same on navmeshes and reports up to 7.7x faster. Both handle obstacles that change poorly.
- **Variants:** HPA*, HNA*.
- **Repo fit:** `subsystem`.
- **Determinism:** ties must be broken by a fixed rule.
- **Builds on:** [Grid graph with jump point search](#grid-graph-with-jump-point-search), [Navigation mesh](#navigation-mesh).
- **Sources:** [HPA*](https://webdocs.cs.ualberta.ca/%7emmueller/ps/hpastar.pdf), [Recast-based A* (HNA* discussion)](https://arxiv.org/html/2602.04130v1).

#### Any-angle pathfinding

- **Uses:** paths not restricted to grid edges or polygon edges.
- **Notes:** Theta* (Nash et al. 2007) propagates through grid edges without restricting paths to them. Anya (Harabor and Grastien 2013) finds optimal any-angle paths on grids. Polyanya (Cui, Harabor, Grastien 2017) is fast, online and optimal on navmeshes, beating Theta*, Lazy Theta* and Field A*; a Rust crate exists.
- **Variants:** Theta*, Lazy Theta*, Anya, Polyanya.
- **Repo fit:** `subsystem`.
- **Determinism:** ties must be broken by a fixed rule.
- **Builds on:** [Navigation mesh](#navigation-mesh).
- **Sources:** [Theta*](https://arxiv.org/pdf/1401.3843), [Polyanya crate](https://docs.rs/polyanya), [Harabor and Grastien, ICAPS 2013](https://users.cecs.anu.edu.au/%7Edharabor/data/papers/harabor-grastien-icaps13.pdf).

#### Incremental replanning

- **Uses:** paths that must be repaired when the map changes.
- **Notes:** D* Lite (Koenig and Likhachev 2002) builds on LPA* and is shorter than D* and at least as efficient.
- **Variants:** LPA*, D* Lite, Field D*.
- **Repo fit:** `subsystem`.
- **Determinism:** ties must be broken by a fixed rule.
- **Builds on:** [Indexed heap with decrease-key](#indexed-heap-with-decrease-key).
- **Sources:** [D* Lite](https://www.ri.cmu.edu/publications/d-lite), [AAAI 2002 paper](https://cse-robotics.engr.tamu.edu/dshell/cs625/aaai02b.pdf).

#### Compressed path database

- **Uses:** optimal paths on static maps with almost no search at runtime.
- **Notes:** Botea (AIIDE 2011) stores the first move of an optimal path from every start to every target, compressed. A query looks up the first move, takes it and repeats, so the delay before the first move is tiny. Building it takes a lot of time and memory. Run-length-encoded databases won the 2014 grid pathfinding competition by a wide margin.
- **Variants:** run-length-encoded; wildcards; bounded-suboptimal.
- **Repo fit:** `subsystem`.
- **Determinism:** deterministic.
- **Builds on:** nothing.
- **Sources:** [Botea, AIIDE 2011](https://ai.dmi.unibas.ch/research/reading_group/botea-aiide2011.pdf), [Fast, optimal pathfinding with CPDs](https://ojs.aaai.org/index.php/SOCS/article/view/18251).

#### 3D flight navigation

- **Uses:** agents that fly anywhere in 3D (Warframe's Archwing).
- **Notes:** Brewer (GDC; Game AI Pro 3) runs A* through a sparse voxel octree, because a navmesh only works for ground movement. The Mercuna middleware builds on this.
- **Variants:** none noted.
- **Repo fit:** `subsystem`.
- **Determinism:** ties must be broken by a fixed rule.
- **Builds on:** [Sparse voxel octree](#sparse-voxel-octree).
- **Sources:** [Game AI Pro 3, chapter 21](https://www.gameaipro.com/GameAIPro3/GameAIPro3_Chapter21_3D_Flight_Navigation_Using_Sparse_Voxel_Octrees.pdf), [Getting off the NavMesh](https://www.gdcvault.com/play/1022016/Getting-off-the-NavMesh-Navigating), [3D voxel pathfinding benchmarks](https://webdocs.cs.ualberta.ca/~nathanst/papers/brewer2018voxelslides.pdf).

#### Flow field

- **Uses:** many agents heading to one goal (crowds, RTS units).
- **Notes:** a single pass over the grid stores, in every cell, the direction toward the goal.
- **Variants:** flow fields computed per sector.
- **Repo fit:** `subsystem`.
- **Determinism:** ties must be broken by a fixed rule.
- **Builds on:** [Uniform grid](#uniform-grid), [Radix heap and bucket queue](#radix-heap-and-bucket-queue).
- **Sources:** none checked yet.

#### Influence map

- **Uses:** tactical reasoning: threat, control of territory.
- **Notes:** a grid of values spread out from units and decaying with distance.
- **Variants:** none noted.
- **Repo fit:** `subsystem`.
- **Determinism:** floating-point accumulation order must be fixed.
- **Builds on:** [Uniform grid](#uniform-grid).
- **Sources:** none checked yet.

#### Crowd avoidance neighbour structure

- **Uses:** local collision avoidance between agents (ORCA).
- **Notes:** in ORCA, each agent turns every neighbour into a half-plane constraint on its velocity and solves a small linear program; the result is collision-free, and velocities change continuously. RVO2 keeps a k-d tree to find nearby agents and obstacles. Global planning is left to the application.
- **Variants:** RVO2 in 2D and 3D.
- **Repo fit:** `subsystem`.
- **Determinism:** neighbours must be processed in a fixed order.
- **Builds on:** [k-d tree](#k-d-tree).
- **Sources:** [RVO2, what is new](https://gamma.cs.unc.edu/RVO2/documentation/2.0/whatsnew.html), [ORCA](https://gamma.cs.unc.edu/ORCA/).

#### Flat behaviour tree

- **Uses:** AI decision logic.
- **Notes:** a data-oriented runtime flattens the tree's shape into a one-dimensional array that many agents share. Each agent keeps its own execution state, and changes go through a buffer.
- **Variants:** event-driven behaviour trees.
- **Repo fit:** `subsystem`.
- **Determinism:** ticking order must be fixed.
- **Builds on:** nothing.
- **Sources:** [Behavior tree entrails](https://www.jikguard.com/news/in-depth-behavior-tree-entrails).

#### GOAP planner state

- **Uses:** goal-oriented action planning.
- **Notes:** world state stored as flat arrays reused between searches, with NaN marking unset keys. Actions are converted to integer-indexed structs before searching.
- **Variants:** world state held in a bitset.
- **Repo fit:** `subsystem`.
- **Determinism:** search ties must be broken by a fixed rule.
- **Builds on:** [Binary and d-ary heap](#binary-and-d-ary-heap).
- **Sources:** [GOAP engine architecture](https://beelabs-dev.gitbook.io/beelabs-docs/goap-engine/architecture-reference), [Aversa, BT vs GOAP](https://davideaversa.it/blog/choosing-behavior-tree-goap-planning/).

#### Utility AI curves

- **Uses:** scoring actions and choosing the best.
- **Notes:** response curves stored as data assets.
- **Variants:** none noted.
- **Repo fit:** `subsystem`.
- **Determinism:** equal scores must be broken by a fixed rule.
- **Builds on:** nothing.
- **Sources:** [Game AI overview](https://aischool.lillytechsystems.com/game-ai/behavior-trees.html).

#### Hierarchical state machine

- **Uses:** AI, gameplay and UI states.
- **Notes:** statecharts extend finite state machines with nested states, guard conditions, history and parallel states, which makes them modular and reusable (Dragert et al.). SCXML has been proposed as a standard for games.
- **Variants:** HFSM, statecharts, SCXML.
- **Repo fit:** `safe`.
- **Determinism:** the order in which transitions are evaluated must be fixed.
- **Builds on:** nothing.
- **Sources:** [Statechart-based AI in practice](https://ojs.aaai.org/index.php/AIIDE/article/view/12515), [Dragert et al.](https://www.sable.mcgill.ca/~clump/papers/dragert-12-statechart.pdf), [SCXML for games](https://eurosis.org/cms/files/conf/gameon-asia/gameon-asia2007/R-SESSION/G4.pdf).

#### Dialogue and quest graph

- **Uses:** branching dialogue, quest progress.
- **Notes:** mostly hand-authored finite state machines or branching trees. Fixed links make dialogue that reacts to game state hard to write, so one alternative picks lines dynamically from variants. HTN planning has also been used.
- **Variants:** Yarn Spinner and Ink (not checked).
- **Repo fit:** `subsystem`.
- **Determinism:** choices must come from a seeded PRNG.
- **Builds on:** [DAG with topological sort](#dag-with-topological-sort).
- **Sources:** [DiGRA paper on dialogue](https://dl.digra.org/index.php/dl/article/download/798/798), [Dialogue generation with HTN](https://cdn.aaai.org/ojs/18682/18682-52-22281-1-10-20210927.pdf).

#### Blackboard

- **Uses:** shared key-value memory for AI agents.
- **Notes:** typed keys, often per agent plus shared.
- **Variants:** none noted.
- **Repo fit:** `safe`.
- **Determinism:** use an [Insertion-ordered map](#insertion-ordered-map).
- **Builds on:** [Insertion-ordered map](#insertion-ordered-map).
- **Sources:** none checked yet.

#### Gameplay tag hierarchy

- **Uses:** hierarchical tags such as `Status.Debuff.Slow`, and queries over them.
- **Notes:** in Unreal's GameplayTags, a container implicitly includes the parents of each tag it holds. Queries are nested any, all and none expressions, stored as a byte stream.
- **Variants:** bitset containers.
- **Repo fit:** `safe`.
- **Determinism:** deterministic if tag IDs are assigned in a fixed order.
- **Builds on:** [Interned name table](#interned-name-table), [Bitset](#bitset).
- **Sources:** [Unreal FGameplayTagQuery](https://dev.epicgames.com/documentation/unreal-engine/API/Runtime/GameplayTags/FGameplayTagQuery), [FGameplayTagContainer](https://dev.epicgames.com/documentation/unreal-engine/API/Runtime/GameplayTags/FGameplayTagContainer).

#### Attribute modifier aggregator

- **Uses:** stat values built from many effects: buffs, debuffs, equipment.
- **Notes:** Unreal's Gameplay Ability System aggregator groups modifiers into ordered evaluation channels, each feeding the next. It can also evaluate in reverse (from the final value back to the base) for client prediction. The exact order of add and multiply steps should be checked in the source.
- **Variants:** evaluation channels.
- **Repo fit:** `safe`.
- **Determinism:** modifier order must be fixed.
- **Builds on:** nothing.
- **Sources:** [FAggregatorModChannelContainer](https://dev.epicgames.com/documentation/unreal-engine/API/Plugins/GameplayAbilities/FAggregatorModChannelContainer), [GAS modifiers reference](https://gas-reference.pages.dev/gameplay-effects/modifiers/).

## 22. Physics

#### Contact manifold cache

- **Uses:** keeping contacts between frames so the solver can start from last frame's impulses (warm starting).
- **Notes:** keyed by the pair of shapes; contacts are matched to last frame's by feature IDs.
- **Variants:** none noted.
- **Repo fit:** `subsystem`.
- **Determinism:** pairs must be iterated in sorted order.
- **Builds on:** [Insertion-ordered map](#insertion-ordered-map).
- **Sources:** none checked yet.

#### Persistent simulation islands

- **Uses:** putting groups of resting bodies to sleep.
- **Notes:** Box2D v3 keeps islands between steps. A new constraint between dynamic bodies merges islands with union-find; splitting is deferred and done by depth-first search, at most one island per step (the "sleepiest" island that lost a constraint). Islands are used only for sleeping.
- **Variants:** islands rebuilt every step (Box2D 2.4).
- **Repo fit:** `subsystem`.
- **Determinism:** links follow contact creation order.
- **Builds on:** [Union-find](#union-find), [Dynamic connectivity](#dynamic-connectivity).
- **Sources:** [Catto, simulation islands](https://box2d.org/posts/2023/10/simulation-islands/), [Avian islands](https://idanarye.github.io/bevy-tnua/src/avian3d/dynamics/solver/islands/mod.rs.html).

#### Solver sets

- **Uses:** keeping solver data for awake and sleeping bodies in separate arrays, coloured for parallel solving.
- **Notes:** Box2D v3's solver works on coloured constraint sets and is multithreaded with SIMD.
- **Variants:** none noted.
- **Repo fit:** `subsystem`.
- **Determinism:** colour assignment must be deterministic.
- **Builds on:** [Graph colouring](#graph-colouring).
- **Sources:** [Releasing Box2D 3.0](https://box2d.org/posts/2024/08/releasing-box2d-3.0), [Box2D solver source](https://docs.rs/crate/box2d_sys/0.2.1/source/box2d/src/solver.c).

#### Particle neighbour grid

- **Uses:** fluids (SPH) and granular materials.
- **Notes:** see [Uniform grid](#uniform-grid): Z-index sort and compact hashing were fastest on CPUs. On GPUs, counting sort plus a prefix scan, and Morton-ordered binning, improve locality.
- **Variants:** counting sort, Z-index sort, compact hashing.
- **Repo fit:** `subsystem`.
- **Determinism:** particles within a cell must be ordered.
- **Builds on:** [Uniform grid](#uniform-grid).
- **Sources:** [Ihmsen et al.](https://cg.informatik.uni-freiburg.de/publications/2011_CGF_dataStructuresSPH.pdf), [Neural particle automata](https://arxiv.org/pdf/2601.16096).

#### Height field and SDF colliders

- **Uses:** terrain collision; colliding with arbitrary shapes through signed distance fields.
- **Notes:** a grid of heights or distances that can be sampled directly.
- **Variants:** SDFs stored in bricks.
- **Repo fit:** `subsystem`.
- **Determinism:** sampling must be exact.
- **Builds on:** [Uniform grid](#uniform-grid).
- **Sources:** none checked yet.

## 23. Particles and procedural generation

#### GPU particle lists

- **Uses:** GPU particle systems.
- **Notes:** a dead list holds the indices of free particles. The simulation reads alive list 1 and writes the survivors to alive list 2; the lists swap each frame, and the draw uses the count the GPU wrote, through indirect arguments. Counters are updated with atomics, because append and consume buffers are not available in every API. Transparent particles are sorted (bitonic sort, or per tile).
- **Variants:** append and consume buffers; a CPU struct-of-arrays pool with swap-remove.
- **Repo fit:** `gpu`, `subsystem`.
- **Determinism:** affects rendering only. CPU simulation must order its pool deterministically.
- **Builds on:** [Indirect argument buffer](#indirect-argument-buffer).
- **Sources:** [Wicked Engine, GPU particles](https://wickedengine.net/2017/11/gpu-based-particle-simulation), [Bevy Hanabi research](https://medium.com/@Sou1gh0st/gpu-particle-research-bevy-hanabi-part-1-9797d3f08535), [Thomas, DirectCompute particles](https://www.slideshare.net/slideshow/holy-smoke-faster-particle-rendering-using-direct-compute-by-gareth-thomas/35656271).

#### Poisson disk background grid

- **Uses:** scattering objects evenly but randomly: foliage, props, sampling patterns.
- **Notes:** Bridson (2007) uses a background grid with cells of size r/√2, so each cell holds at most one sample and a check touches a fixed number of cells; an active list of samples drives it, and it runs in linear time.
- **Variants:** maximal Poisson disk sampling.
- **Repo fit:** `safe`.
- **Determinism:** uses a seeded PRNG with a fixed order of draws.
- **Builds on:** [Uniform grid](#uniform-grid).
- **Sources:** [Bridson, fast Poisson disk sampling](https://www.cs.ubc.ca/~rbridson/docs/bridson-siggraph07-poissondisk.pdf).

#### Wave function collapse grid

- **Uses:** generating tile maps and levels.
- **Notes:** an adjacency table lists the patterns allowed next to each pattern in each direction. Each cell keeps a set of patterns it may still become; a heap ordered by entropy picks the next cell to fix, and propagation narrows the neighbours' sets.
- **Variants:** overlapping and simple-tiled models; hierarchical WFC.
- **Repo fit:** `safe`.
- **Determinism:** entropy ties must be broken by a seeded PRNG.
- **Builds on:** [Bitset](#bitset), [Binary and d-ary heap](#binary-and-d-ary-heap).
- **Sources:** [Elm WFC implementation](https://git.bytes.zone/BrianHicks/elm-wave-function-collapse/blob/cfe52e68ce879cccebb8088093d5bc5b56fc1cef/src/Wave.elm), [Hierarchical WFC](https://pcgworkshop.com/archive/alaka2023hierarchical.pdf).

## 24. Audio

#### Mixer graph

- **Uses:** routing voices through submix buses to the master output.
- **Notes:** a DAG in which each voice or bus knows its parent; voices mix into buses, and buses are processed in topological order. Effects go on buses, so they run once per bus rather than once per sound. XAudio2 allows any depth of submixing.
- **Variants:** buffers preallocated for every bus.
- **Repo fit:** `subsystem`.
- **Determinism:** affects audio output only.
- **Builds on:** [DAG with topological sort](#dag-with-topological-sort).
- **Sources:** [XAudio2 introduction](https://learn.microsoft.com/en-us/windows/win32/xaudio2/xaudio2-introduction), [DALIA architecture](https://amhumming.github.io/dalia/architecture/).

#### Voice pool with virtual voices

- **Uses:** many sounds playing at once with a limited number actually rendered.
- **Notes:** physical voices are fully processed (streaming, decoding, filters); virtual voices only track their state and volume. Limits apply at several levels (Wwise: actor-mixer, bus, global), and stealing policies choose which voice to drop: newest, lowest priority, least audible, or a weighted mix.
- **Variants:** resume from the stored position when a voice becomes physical again.
- **Repo fit:** `subsystem`.
- **Determinism:** affects audio output only.
- **Builds on:** [Pool allocator](#pool-allocator), [Binary and d-ary heap](#binary-and-d-ary-heap).
- **Sources:** [Audiokinetic, voice optimisation](https://www.audiokinetic.com/en/how-to-get-a-hold-on-your-voices-optimizing-for-cpu-part-1), [web engine audio concepts](https://web-engine-dev-docs.pages.dev/concepts/audio).

## 25. UI and input

#### Immediate-mode ID stack

- **Uses:** stable widget identity in immediate-mode UI.
- **Notes:** Dear ImGui hashes the contents of an ID stack (PushID and PopID) into a 32-bit ID; `##` hides part of a label from display but keeps it in the hash. Identical labels in the same scope collide. egui uses a similar hashed ID.
- **Variants:** IDs hashed from the source-code location.
- **Repo fit:** `subsystem` (editor, tools).
- **Determinism:** the hash must be fixed.
- **Builds on:** nothing.
- **Sources:** [Source-code location hashing](https://jakubtomsu.github.io/posts/source_code_location_hashing), [egui Id](https://docs.rs/crate/egui/0.18.1/source/src/id.rs), [imgui-rs Id](https://docs.rs/imgui/latest/imgui/struct.Id.html).

#### Retained layout tree

- **Uses:** UI layout.
- **Notes:** Taffy implements CSS Block, Flexbox and Grid layout. It stores nodes in a slotmap, caches layout per node, and invalidates the cache on style changes. Servo, Bevy, Zed (GPUI) and Slint use it.
- **Variants:** recomputing only dirty subtrees.
- **Repo fit:** `subsystem`.
- **Determinism:** layout must be bit-exact for a given build.
- **Builds on:** [Slot map](#slot-map).
- **Sources:** [Taffy](https://github.com/dioxuslabs/taffy).

#### Immediate layout pass

- **Uses:** UI layout recomputed every frame.
- **Notes:** Clay (Nic Barker) is a single C header of about 2000 lines that does flexbox-style layout from scratch every frame, with static, arena-based memory. It keeps only what it needs between frames (hit testing, scrolling, text measurement) and outputs render commands for any renderer.
- **Variants:** none noted.
- **Repo fit:** `subsystem`.
- **Determinism:** deterministic.
- **Builds on:** [Bump arena](#bump-arena).
- **Sources:** [Clay](https://github.com/nicbarker/clay), [Simon Willison on Clay](https://simonwillison.net/b/8389).

#### Input buffer

- **Uses:** generous input timing; recognising motion inputs (fighting games, action games).
- **Notes:** a fixed ring of recent inputs per frame. A motion such as 236A is checked by walking the stored frames with an allowed gap between steps (8 frames in one example) and little or no leeway on the final button. Buffer windows vary by game (3–10 frames).
- **Variants:** buffering per action; leniency that changes with player skill.
- **Repo fit:** `safe`.
- **Determinism:** works on simulation ticks only, and has to be part of the replay inputs.
- **Builds on:** [Ring buffer](#ring-buffer).
- **Sources:** [Fighting game input buffer](https://seung-cha.github.io/coding/2024/01/26/fighting-game-input-buffer.html), [CritPoints, motion inputs](https://critpoints.net/2025/02/05/how-to-code-fighting-game-motion-inputs/), [WPI input buffer](https://dgmiwiki.wpi.edu/mechanic/input_buffer).

#### Input action map

- **Uses:** mapping physical inputs to game actions, with contexts and rebinding.
- **Notes:** a table from bindings to actions, with a stack of active contexts.
- **Variants:** chords; modifiers.
- **Repo fit:** `safe`.
- **Determinism:** inputs reach the simulation only as explicit parameters.
- **Builds on:** [Insertion-ordered map](#insertion-ordered-map).
- **Sources:** none checked yet.

## 26. Scripting runtimes

#### Tagged value representation

- **Uses:** dynamically typed values in a scripting VM or mod runtime.
- **Notes:** a 2026 study on AArch64 and x86-64 found low-bit pointer tagging usually fastest for mostly symbolic workloads. NaN-boxing is close and avoids allocating floats on the heap. Self-tagging (2024) speeds up float-heavy benchmarks by 2.4x in Scheme and 3.6x in JavaScript. Ladybird's NaN-boxed values keep 48 bits of payload.
- **Variants:** low-bit tagging, NaN-boxing, self-tagging, badged headers.
- **Repo fit:** `unsafe`, `subsystem`.
- **Determinism:** NaN payloads must be preserved exactly.
- **Builds on:** nothing.
- **Sources:** [Value representation re-benchmarked (2026)](https://arxiv.org/html/2606.05466), [Self-tagging](https://www.arxiv.org/pdf/2411.16544), [What is the best pointer tagging method?](https://coredumped.dev/2024/09/09/what-is-the-best-pointer-tagging-method/).

#### Hidden classes and inline caches

- **Uses:** fast property access on dynamic objects, and possibly on runtime-defined component types.
- **Notes:** objects with the same layout share a hidden class (V8's "map", inherited from Self). An inline cache checks the class and then reads at a known offset, skipping the lookup.
- **Variants:** monomorphic and polymorphic inline caches.
- **Repo fit:** `subsystem`.
- **Determinism:** deterministic.
- **Builds on:** [Insertion-ordered map](#insertion-ordered-map).
- **Sources:** [Egorov, V8 lecture notes](https://mrale.ph/talks/cwp2013).

## 27. Networking, replication and replay

#### Sequence buffer

- **Uses:** reliability over UDP: per-packet state indexed by sequence number.
- **Notes:** Gaffer's rolling buffer indexed by `sequence % N` avoids a hash table. Each packet header carries the latest ack and a 32-bit bitfield acking the 32 packets before it, so acks are repeated in every packet. Clear skipped entries, or old entries cause false acks when the 16-bit sequence wraps.
- **Variants:** ack bitfield; acks for chunked data blocks.
- **Repo fit:** `safe`.
- **Determinism:** deterministic.
- **Builds on:** [Ring buffer](#ring-buffer).
- **Sources:** [Gaffer, reliable ordered messages](https://gafferongames.com/post/reliable_ordered_messages), [Gaffer, reliability over UDP](https://gafferongames.com/post/reliability_ordering_and_congestion_avoidance_over_udp/), [Gaffer, sending large blocks](https://www.gafferongames.com/post/sending_large_blocks_of_data/).

#### Rollback state ring

- **Uses:** rollback netcode: saved states and inputs, so the game can rewind and resimulate.
- **Notes:** the full state after each frame goes into a ring; GGPO sends only inputs, never state. Without incremental snapshots, the whole physics state has to be saved every frame, which limits world size.
- **Variants:** input rings; snapshots saved as deltas.
- **Repo fit:** `safe`.
- **Determinism:** needs full simulation determinism (ADR 0003).
- **Builds on:** [Ring buffer](#ring-buffer).
- **Sources:** [Delta rollback](https://medium.com/@david.dehaene/delta-rollback-new-optimizations-for-rollback-netcode-7d283d56e54b), [HN discussion](https://hn.svelte.dev/item/47981979).

#### Delta encoding

- **Uses:** saving bandwidth on inputs and snapshots.
- **Notes:** GGRS and backroll XOR inputs against a reference and then run-length encode the result. Snapshots are delta-encoded against the last snapshot the client acknowledged.
- **Variants:** XOR plus RLE; deltas against an acknowledged baseline.
- **Repo fit:** `safe`.
- **Determinism:** the encoding is a format, so version it.
- **Builds on:** [Sequence buffer](#sequence-buffer).
- **Sources:** [GGRS compression](https://docs.rs/crate/ggrs/latest/source/src/network/compression.rs), [backroll](https://docs.rs/crate/backroll/0.6.0/source/src/protocol/compression.rs).

#### Bit-packed stream

- **Uses:** network packets and compact saves.
- **Notes:** values written with exactly the bits they need, including quantised floats.
- **Variants:** none noted.
- **Repo fit:** `safe`.
- **Determinism:** the layout is a format.
- **Builds on:** nothing.
- **Sources:** none checked yet.

#### Replication dirty mask

- **Uses:** sending only the properties that changed.
- **Notes:** Unreal's Iris marks dirty properties in a change mask and is designed around push-based updates; objects without push updates are polled at their update frequency. The dirty bit covers a whole property, so arrays need a fast-array serialiser for per-element deltas. Iris is still beta in UE 5.7.
- **Variants:** push or polling.
- **Repo fit:** `safe`.
- **Determinism:** the order properties are serialised in must be fixed.
- **Builds on:** [Bitset](#bitset).
- **Sources:** [Epic, migrate to Iris](https://dev.epicgames.com/documentation/en-us/unreal-engine/migrate-to-iris-in-unreal-engine), [FPropertyReplicationState](https://dev.epicgames.com/documentation/unreal-engine/API/Runtime/IrisCore/FPropertyReplicationState).

#### Relevancy filter

- **Uses:** deciding which objects each client receives (interest management).
- **Notes:** Iris replaces Replication Graph's nodes with object filters and prioritisers, and the two systems cannot be combined. A spatial grid is the common building block.
- **Variants:** grid filters; prioritisers.
- **Repo fit:** `subsystem`.
- **Determinism:** the order objects are sent in must be fixed.
- **Builds on:** [Uniform grid](#uniform-grid).
- **Sources:** [Epic, migrate to Iris](https://dev.epicgames.com/documentation/en-us/unreal-engine/migrate-to-iris-in-unreal-engine).

#### Priority accumulator

- **Uses:** sharing out limited bandwidth among objects.
- **Notes:** each object's priority builds up every tick until it is sent, then resets.
- **Variants:** none noted.
- **Repo fit:** `safe`.
- **Determinism:** ties must be broken by ID.
- **Builds on:** [Binary and d-ary heap](#binary-and-d-ary-heap).
- **Sources:** none checked yet.

#### Jitter buffer

- **Uses:** smoothing uneven packet arrival for snapshot interpolation and voice.
- **Notes:** holds packets for a short delay, in sequence order.
- **Variants:** adaptive delay.
- **Repo fit:** `safe`.
- **Determinism:** only output reads it.
- **Builds on:** [Sequence buffer](#sequence-buffer).
- **Sources:** none checked yet.

#### State hash tree

- **Uses:** detecting desyncs, and finding which part of the state differs (roadmap M9 record and replay).
- **Notes:** a Merkle tree of hashes over parts of the state; comparing roots and then walking down finds where two states diverge.
- **Variants:** a single checksum; per-system hashes.
- **Repo fit:** `safe`.
- **Determinism:** the hash is a frozen format (it needs an ADR, like M5's stable content hasher).
- **Builds on:** [Hashed string ID](#hashed-string-id).
- **Sources:** none checked yet.

## 28. Persistence, assets and the editor

#### Undo command stack

- **Uses:** editor undo and redo (roadmap M6).
- **Notes:** each edit is an invertible command; an undo stack and a redo stack.
- **Variants:** merging commands; event sourcing.
- **Repo fit:** `safe`.
- **Determinism:** applying and then undoing must restore the exact state (M6's exit criterion).
- **Builds on:** nothing.
- **Sources:** none checked yet.

#### Persistent collections

- **Uses:** cheap snapshots for undo history, branching edits, and sharing state between threads.
- **Notes:** a HAMT gives a persistent hash map (Clojure, Scala). CHAMP (Steindorfer and Vinju, OOPSLA 2015) keeps a HAMT's lookup and update speed but uses less memory and iterates and compares much faster. RRB vectors make concatenation and splitting cheap. The immer library (CppCon 2025) builds undo history as a vector of document snapshots. The cost is more memory and more allocations.
- **Variants:** HAMT, CHAMP, RRB vector, persistent B-tree.
- **Repo fit:** `safe`.
- **Determinism:** hash-ordered iteration must not be observed, or define the order.
- **Builds on:** [Trie](#trie).
- **Sources:** [CHAMP](https://blog.acolyer.org/2015/11/27/hamt/), [champ-trie](https://docs.rs/crate/champ-trie/0.2.0/source/README.md), [immer, CppCon 2025](https://sinusoid.es/talks/cppcon25), [RRB trees](https://medium.com/@abhi18av/immutable-data-structures-rrb-trees-part-1-177a986950ec).

#### Text CRDT

- **Uses:** collaborative editing of scripts and text in the editor.
- **Notes:** Fugue (Weidner and Kleppmann 2023) minimises interleaving when two users type at the same spot, and performs about as well as Yjs. Eg-walker (Gentle and Kleppmann, EuroSys 2025) holds only the text in memory while editing. It uses an order of magnitude less memory than CRDTs, loads orders of magnitude faster, and needs no server.
- **Variants:** Yjs, Fugue, FugueMax, Eg-walker, Loro.
- **Repo fit:** `subsystem` (editor).
- **Determinism:** merges are deterministic by design.
- **Builds on:** [Sum tree](#sum-tree).
- **Sources:** [Fugue](https://arxiv.org/pdf/2305.00583), [Eg-walker](https://arxiv.org/abs/2409.14252), [Kleppmann on Eg-walker](https://martin.kleppmann.com/2025/03/30/eg-walker-collaborative-text.html).

#### Scene merge

- **Uses:** several people editing one level.
- **Notes:** LevelMerge (2018) merges levels as labelled graphs. Blazium's multi-user editor merges scripts with text CRDTs and edits scenes live. One argument is that level edits rarely touch the same data at the same time, so merging per property or locking per region may suit levels better than a CRDT.
- **Variants:** merging per property; locking per region.
- **Repo fit:** `subsystem`.
- **Determinism:** merge results must be deterministic.
- **Builds on:** [Text CRDT](#text-crdt), [Undo command stack](#undo-command-stack).
- **Sources:** [LevelMerge](https://www.academia.edu/102248301/LevelMerge_Collaborative_Game_Level_Editing_by_Merging_Labeled_Graphs), [Blazium multi-user editor](https://www.moddb.com/news/multiuser-editor-module), [Caustic, collaboration architecture](https://caustic-marketing-staging.pages.dev/blog/real-time-collaboration-architecture).

#### Zero-copy archive

- **Uses:** fast loading of assets and saves without a parse step.
- **Notes:** rkyv uses relative pointers (offsets), so a file can be memory-mapped and used where it lies, provided each pointer and its target move together. It has no schema of its own beyond the Rust types, so saves need their own versioning. FlatBuffers uses an external schema with per-table field offsets ("vtables", not checked). Relevant to the M5 serialisation decision.
- **Variants:** rkyv, FlatBuffers, Cap'n Proto.
- **Repo fit:** `safe` (validated access) or `unsafe`.
- **Determinism:** the layout is a format.
- **Builds on:** nothing.
- **Sources:** [rkyv relative pointers](https://rkyv.org/architecture/relative-pointers.html), [Zero-copy deserialisation](https://mohakchugh.is-a.dev/blog/zero-copy-deserialization-rkyv-flatbuffers-memory-mapped-data).

#### Content-defined chunking

- **Uses:** deduplicating asset storage, derived-data caches, patch delivery.
- **Notes:** FastCDC (USENIX ATC 2016) uses the Gear rolling hash to cut chunk boundaries based on content, so an edit changes only nearby chunks. It is about 10x faster than Rabin-based chunking and 3x faster than earlier Gear-based chunking, with almost the same deduplication ratio. A seed can hide chunk boundaries from observers.
- **Variants:** FastCDC 2016 and 2020; content-addressed stores keyed by chunk hash.
- **Repo fit:** `safe`.
- **Determinism:** the hash table and seed must be fixed.
- **Builds on:** nothing.
- **Sources:** [FastCDC](https://www.usenix.org/conference/atc16/technical-sessions/presentation/xia), [fastcdc crate](https://docs.rs/crate/fastcdc/4.0.1/source/src/v2020/mod.rs).

#### Asset dependency graph

- **Uses:** hot reload, incremental builds, streaming order.
- **Notes:** a DAG from each asset to the assets it uses, invalidated when an asset changes.
- **Variants:** none noted.
- **Repo fit:** `subsystem`.
- **Determinism:** rebuild order must come from a topological sort with stable tie-breaks.
- **Builds on:** [DAG with topological sort](#dag-with-topological-sort).
- **Sources:** none checked yet.

## 29. Caching and eviction policies

#### LRU and CLOCK

- **Uses:** caches for streamed textures and meshes, shader caches, glyph caches.
- **Notes:** LRU is vulnerable to one-off scans and has lock contention; CLOCK approximates LRU with a reference bit per entry. One survey says most modern caches have replaced pure LRU with ARC, W-TinyLFU or S3-FIFO.
- **Variants:** segmented LRU.
- **Repo fit:** `safe`.
- **Determinism:** eviction must depend only on the sequence of accesses.
- **Builds on:** [Intrusive linked list](#intrusive-linked-list).
- **Sources:** [Cache algorithms notes](https://nmbr7.github.io/notes/database/cache-algorithms/).

#### ARC and W-TinyLFU

- **Uses:** caches that resist scans.
- **Notes:** ARC balances recently used and frequently used lists; its patent expired in 2024. W-TinyLFU decides admission with a frequency sketch.
- **Variants:** none noted.
- **Repo fit:** `safe`.
- **Determinism:** eviction must depend only on the sequence of accesses.
- **Builds on:** [Count-min sketch](#count-min-sketch).
- **Sources:** [memcached, cache cost](https://memcached.org/blog/cache-cost/).

#### SIEVE

- **Uses:** a simple, effective eviction policy.
- **Notes:** Zhang et al. (NSDI 2024): a FIFO queue plus a moving "hand" pointer. A hit only sets a bit, with no locking, and their prototype has twice the throughput of an optimised LRU. They report up to 63.2% fewer misses than ARC across 1559 traces. A replication found it worse than LRU on some block-cache workloads, because it has no ghost queue.
- **Variants:** ARC-SIEVE.
- **Repo fit:** `safe`.
- **Determinism:** eviction must depend only on the sequence of accesses.
- **Builds on:** [Ring buffer](#ring-buffer).
- **Sources:** [SIEVE (NSDI 2024)](https://www.usenix.org/conference/nsdi24/presentation/zhang-yazhuo), [SIEVE vs LRU replication](https://www.scs.stanford.edu/26wi-cs244c/proj/sieve_v_lru.pdf).

#### S3-FIFO

- **Uses:** a scan-resistant eviction policy built from FIFO queues.
- **Notes:** three static FIFO queues plus a ghost queue of evicted keys. Across 6594 traces it had a lower miss ratio than 12 other policies, and about 6x the throughput of an optimised LRU at 16 threads.
- **Variants:** none noted.
- **Repo fit:** `safe`.
- **Determinism:** eviction must depend only on the sequence of accesses.
- **Builds on:** [Ring buffer](#ring-buffer).
- **Sources:** [FIFO is all you need](https://systems.seas.harvard.edu/blog/fifo-is-all-you-need/), [s3_fifo in Rust](https://docs.rs/ftui-core/0.4.0/src/ftui_core/s3_fifo.rs.html).

## 30. Localization

#### Localized string table

- **Uses:** game text per locale.
- **Notes:** a table from keys to strings for each locale, with a fallback chain (for example pt-BR, then pt, then en). Fallback behaviour was not checked in this research.
- **Variants:** in-game string fixing as a backstop.
- **Repo fit:** `subsystem`.
- **Determinism:** text never reaches simulation results.
- **Builds on:** [Hashed string ID](#hashed-string-id), [String arena](#string-arena).
- **Sources:** [ICU plurals in game localization](https://loxily.com/en/blog/icu-plural-game-localization).

#### Plural rule table

- **Uses:** choosing the grammatical form for a number.
- **Notes:** CLDR defines plural categories for each language (English 2, Polish 4, Arabic 6, Japanese 1). `other` is the required fallback, and exact matches such as `=0` are checked first. ICU MessageFormat uses these rules.
- **Variants:** ICU MessageFormat.
- **Repo fit:** `subsystem`.
- **Determinism:** deterministic.
- **Builds on:** nothing.
- **Sources:** [Crowdin, ICU guide](https://crowdin.com/blog/icu-guide), [Lokalise, ICU message format](https://lokalise.com/blog/complete-guide-to-icu-message-format/).
