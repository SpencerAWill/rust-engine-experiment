# M3.5 log: Per-component hybrid prototype

- **Status:** Living (2026-11-20)

- 2026-11-20 decision Next chunk is M3.5: M3.19's spec names no Followed by chunk, and M3.5 is the first Planned M3 row (table order) whose dependencies (M3.3, M3.4) are Done. M3.10 is also eligible but comes later in the table.
- 2026-11-20 done The row cites no issue, but #82 (enable bits) and #83 (one query and command API over several storage kinds) are both "answered by M3.5's spec" and are Project items: both set to Scheduled. Tracking issue #78.
- 2026-11-20 decision Informal numbers from PRs #75, #77, #87 frame the chunk: `archetype` leads every workload except `add_remove`, where `sparse_set` (10.1 µs at 1,000) and `dense` (11.0 µs) are about twice as fast as `archetype` (20.8 µs). The hybrid's question is whether table storage plus a per-type alternative for toggled components keeps archetype iteration and gains sparse-set toggles.
- 2026-11-20 decision (maintainer) Two storage kinds per component type: archetype table and sparse set (Bevy's split). Dense is not a third kind: it ties sparse sets on `add_remove` and loses every iteration workload.
- 2026-11-20 decision (maintainer) Storage is chosen statically: a crate-local `Component` trait with `const STORAGE: StorageKind`, implemented for the harness types by a macro, so each fetch picks slices or probes at compile time (#83's design).
- 2026-11-20 decision (maintainer) Archetypes and tables key on table types only. A sparse insert or remove touches only its set (no archetype change, no move); `&S`, `With<S>` and `Without<S>` on a sparse type probe per row of each matched table.
- 2026-11-20 decision (maintainer) One backend, `hybrid`: sparse sets hold only the toggled types, `B` (`add_remove`) and `Seeker` (`agents`); every other type, `Hidden` included, is a table type. The other five workloads measure the hybrid query layer's cost over pure archetypes.
- 2026-11-20 decision (maintainer) #82 is answered by rejection in this chunk, with the reason in the spec, and a follow-up issue under #78 conditional on M3.8 picking tables (as #80, #81).
- 2026-11-20 decision (maintainer) #83: build the shared core only (spawn_with, insert, remove, get, tuple queries, filters, Commands over both kinds); capability traits, hints and the escape hatch are recorded as M4 design and added to #84.
- 2026-11-20 decision (main session) `Component` names a sealed storage marker (`type Storage = TableStorage | SparseStorage`) carrying `const KIND`, rather than `const STORAGE` on `Component` itself: a constant cannot choose the fetch type in safe stable Rust without a per-row branch. Flagged in spec §4.2 and §7 for the grill.
- 2026-11-20 decision (main session) Queries walk matched tables and probe sparse sets per row; driving a query from its smallest sparse set is deferred (no workload separates the two). Despawn visits every registered sparse set. The crate depends on neither sibling prototype; it adapts their code.
- 2026-11-20 done Spec drafted.
