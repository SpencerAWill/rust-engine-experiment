# M3.26: workload checks that can fail for frag_iter, heavy_compute and add_remove

- **Status:** Accepted (2026-11-26)
- **Milestone:** M3 (ECS storage decision) · **Depends on:** M3.2, M3.10 · **Decides:** no ADR
- **Issue:** #141
- **Estimated size:** about 200 changed lines, excluding tests, lockfiles and generated files (harness rules about 20, eight `heavy_compute` steps about 4 each, ten `add_remove` test accessors about 6 each, documents about 60)

Requirement keywords (MUST, MUST NOT, SHOULD, MAY) follow [RFC 2119](https://www.rfc-editor.org/rfc/rfc2119) as clarified by [RFC 8174](https://www.rfc-editor.org/rfc/rfc8174): they are normative only in uppercase.

## 1. Summary

Two of the M3.2 workloads stop changing during the conformance run, and a third cannot show its main pass in the checksum, so parts of their conformance tests cannot fail: `frag_iter` freezes at tick 26, `heavy_compute`'s state is the identity in exact arithmetic and changes only through rounding drift in 37 of 1,000 entities, and `add_remove`'s checksum is the same at every tick. This chunk changes the `frag_iter` and `heavy_compute` rules so that every entity changes at every tick, pins that with `vec_aos` tests, adds a per-step `add_remove` test on every ECS backend that fails when the insert pass is skipped, and re-runs the informal benchmark rows of the two changed workloads. It comes before M3.8 (#102), which reads those rows as backed by conformance.

## 2. Goals and non-goals

Goals:

- Every compared tick of `frag_iter` and `heavy_compute` conformance can fail: every entity's state changes at every tick, and no tick's checksum equals an earlier one's in the conformance run.
- On every ECS backend and on `vec_aos`, a step that skips `add_remove`'s insert pass, at any tick, fails a test.
- The changed rules are written where the rules are stated, and M3.8 has informal rows for the rules as they now are.

Non-goals:

- No change to `add_remove`'s rule or checksum, or to any other workload. A rule that makes the checksum read `B` was not chosen (§7).
- No check of the inserted `B`'s value: the test checks which entities got a `B`, not what it holds.
- No formal results; M3.8 measures under the protocol. The cache bench is not run.
- Nothing in `libs/`.

## 3. Guide-level explanation

Omitted (small chunk).

## 4. Reference-level design

### 4.1 Findings

Simulated in f32 on the pinned toolchain, at the conformance parameters (`frag_iter` count 20, `heavy_compute` and `add_remove` count 1,000, seed 42), in a scratchpad crate with path dependencies on the spike (recorded in `log.md`):

| Workload | Today | Under §4.2 |
|---|---|---|
| `frag_iter` | checksum equals the previous one at 95 of 120 ticks, from tick 26 (every value is 2.0) | all 520 entities change every tick; no state repeats an earlier tick in 120 ticks |
| `heavy_compute` | 100 inversions (an even count) return 963 of 1,000 matrices bit for bit by about tick 20, so 37 entities change per tick; 77 of 120 checksums equal an earlier tick's | all 1,000 entities change every tick; no checksum equals an earlier tick's in 120 or 1,000 ticks; the largest translation is 11.8 at tick 120 and 26.2 at tick 1,000 |
| `add_remove` | checksum equal at all 120 ticks, by design (M3.2-R8) | unchanged; §4.3 adds a direct test |

The existing `add_remove` tests on `archetype`, `hybrid` and `dense` see the insert only after the first step (an archetype with `B`, a registered `B`, a `B` column), so a step that inserts only at tick 1 passes them.

### 4.2 Rule changes

These amend the M3.2 spec, §4.4.2 and §4.4.3. Spawn and hash are unchanged, so no PRNG stream and no hash layout changes.

- **`frag_iter` step.** For every entity with `Data`: `x = data.0 + 0.618_034`, then `data.0 = x - 1.0` if `x >= 1.0`, else `x`. It reads only `Data`, as before, so the query is unchanged. Values stay in `0.0..1.0`. (Rotation by a constant near the golden ratio's conjugate; a dyadic constant such as 0.25 repeats with a short period.)
- **`heavy_compute` step.** For every entity: 100 times, `matrix.0 = matrix.0.inverse()` (written back each time, as before); then, for each row `r` in 0 to 2, `translation[r] = translation[r] + velocity.0[r] * dt` (multiply, then add; not fused), where `translation` is column 3 of the matrix and `dt` is `spike_ecs_harness::dt()`; then `position.0 = matrix.0.transform_point3(rotation.0)`. The step now reads `Velocity`, which every entity already has. The harness's `heavy_compute::update` gains the `velocity: &Velocity` parameter and MUST hold the whole rule, so backends keep calling it rather than writing the arithmetic.
- **Backends.** Every backend that implements `heavy_compute` (`vec_aos`, `sparse_set`, `sparse_set_grouped`, `archetype`, `dense`, `hybrid`, `bevy_ecs`, `hecs`) adds `&Velocity` to its step query and passes it to `update`. `sparse_set_grouped`'s group MUST be widened to `(Matrix, Position, Rotation, Velocity)`, so the step query stays an exact match of the group: a three-type group would still take the group path for the four-type query but probe `Velocity` per row (`superset_takes_the_group_path_and_probes_the_rest`), which the variant does not measure today. Its existing path test (`grouped_heavy_compute_uses_the_group`, built from the step's own query, ecs-1) still passes. *(main session's call)* Every `frag_iter` backend calls the harness's `update` already, so only the harness changes for `frag_iter`.

### 4.3 The `add_remove` per-step test

Every ECS `add_remove` step collects, between its two passes, the entities that a "has `B`" query returns (`with_b`, `with_b_list`, or `Base::with_b` for the byte-column backends), then removes `B` from each. After a step that list shows which entities the insert pass reached.

- Each of the ten ECS `add_remove` backends (`sparse_set`, `sparse_set_grouped`, `archetype`, `archetype_bytes_copy`, `archetype_bytes_cast`, `hybrid`, `dense`, `bevy_ecs`, `bevy_ecs_sparse`, `hecs`) gets a `#[cfg(test)]` accessor for the removal list of the last step, and a test in its crate that, at the conformance parameters, after each of 120 steps asserts the list holds each of the `count` live entities exactly once.
- The accessor MUST read the list the measured step filled, not a list built again by the test; non-test builds MUST NOT change.
- Red check (ecs-11): with the insert pass removed from the step, the test MUST fail on each backend, and with the insert done only at tick 1 it MUST fail at tick 2; both are run on every backend and recorded in `log.md`, with the failures listed by `cargo nextest run -p <package> --no-fail-fast` (ecs-6). *My call:* the red check runs on all ten, not one per crate, because the steps are separate code.

### 4.4 `vec_aos` tests

- **`add_remove`.** `vec_aos` has no removal list; its insert pass sets each entity's `Option<B>` to `Some`. Its step gets a test-only hook between the two passes, in the form M3.22 used for `many_queries`: the step's loop is generic over a closure, the measured `step` passes a no-op, and a `#[cfg(test)]` method runs the same loop with a closure that asserts every entity holds `Some(make_b(a))` between the passes, after each of 120 steps. Red check: removing the insert pass fails it. *(main session's call: the reference's insert is as invisible to conformance as any backend's, and its row is benchmarked too.)*

- `frag_iter` and `heavy_compute` each get a `vec_aos` test that runs 120 ticks at the conformance parameters and asserts each tick's checksum differs from every earlier tick's, tick 0 included. *My call:* "every earlier tick" rather than ecs-13's "the one before", because the current `heavy_compute` rule passes "the one before" (its checksum changes at every tick) and fails only this form.
- Red check: each test MUST fail with the old rule put back; recorded in `log.md`.
- The existing `vec_aos` tests that state the old rules (`frag_iter_data_follows_the_map`, `heavy_compute_position_is_the_transformed_rotation`, and any other a builder finds) are updated to the new rules, with `frag_iter` covering both the wrapping and the non-wrapping branch, and `heavy_compute` checking the translation moved by `velocity * dt`. The long-run finiteness tests stay.

### 4.5 Documents

- **M3.2 spec.** §4.4.2 and §4.4.3 are amended in place: the new step text, marked "amended 2026-11-26 (M3.26)", with the old rule and the reason in one sentence each; the status line names the amendment. R8's parenthesis on `add_remove` gains a pointer to §4.3 of this spec. The step sentence "The map converges to 2.0" goes, and so does §4.4.3's "Position is overwritten, not accumulated, so it stays bounded" (the translation now accumulates; it stays finite, §4.1). R9's verification row, which states `d * 0.5 + 1.0`, gets the same dated note.
- **Harness doc comments** for both `update` functions and the module docs that state a rule.
- **`spikes/ecs_storage/README.md`**: the status line, and the sentence that points to the M3.2 rules says they are amended by M3.26.
- **`CHANGELOG.md`**: one entry.

### 4.6 The informal run

`cargo xtask spike-bench ecs --save m3.26-informal` under the benchmark run protocol (`/bench-run`) on the final code commit, labelled informal (the command has no per-workload filter). Its `frag_iter` and `heavy_compute` rows go in `log.md`, the pull request, and a comment on #102 that says the rows of these two workloads in earlier runs (`m3.10`, `m3.21`, `m3.22` baselines) were measured under the old rules. One run: no noise floor (bench/README.md).

## 5. Requirements

- **M3.26-R1** The `frag_iter` and `heavy_compute` rules MUST be those of §4.2, held in the harness's `update` functions, and every backend that implements them MUST pass conformance against `vec_aos` (120 ticks) under the new rules.
- **M3.26-R2** `vec_aos` MUST have, for `frag_iter` and for `heavy_compute`, a test that every tick's checksum in 120 ticks at the conformance parameters differs from every earlier tick's (§4.4), shown failing with the old rule.
- **M3.26-R3** Each of the ten ECS `add_remove` backends MUST have the per-step test of §4.3, shown failing on each with the insert pass removed and with the insert at tick 1 only; `vec_aos` MUST have the between-passes test of §4.4, shown failing with the insert pass removed.
- **M3.26-R4** `sparse_set_grouped`'s `heavy_compute` group MUST be the four step types, and its step MUST take the group path, shown by its existing path test.
- **M3.26-R5** The documents of §4.5 MUST state the new rules; no document MAY state an old rule as current.
- **M3.26-R6** The pull request MUST show one informal `spike-bench ecs` run under the protocol with the `frag_iter` and `heavy_compute` rows; the same rows MUST be in `log.md` and in a comment on #102 (§4.6).
- **M3.26-R7** Non-test builds of the ECS `add_remove` backends MUST NOT change (§4.3); `vec_aos`'s `add_remove` step changes only by the no-op closure of §4.4.

## 6. Cross-cutting concerns

- **Determinism:** both rules are plain f32 add, multiply, compare and the existing `inverse`; no fused or transcendental operations; `dt()` is a constant. Conformance compares bits, so every backend must call the harness `update`.
- **`unsafe`:** n/a, because no `unsafe` is added and every spike crate keeps `#![forbid(unsafe_code)]`.
- **Concurrency:** n/a, because every step is single-threaded as before.
- **Heap allocation:** n/a, because no step allocates more than before; the accessors are test-only.
- **`no_std` and wasm:** n/a, because spike crates are not in those builds.
- **Frozen formats:** n/a, because spawn draws and hashes are unchanged and nothing in `libs/` changes.
- **Public API changes:** `spike_ecs_harness::heavy_compute::update` gains a `velocity` parameter; spike crates are `publish = false`. No `engine` change.
- **FFI exposure:** n/a.
- **Performance:** `frag_iter` per entity is add, compare, select instead of multiply, add; `heavy_compute` adds one `Velocity` read and three multiply-adds next to 100 4×4 inversions. Both change the rows, so they are re-measured (§4.6) and not compared with runs under the old rules.
- **Documentation:** §4.5.
- **Policy changes:** none.

## 7. Alternatives considered

- **`frag_iter`: compare only ticks 0 to 25.** The rule would still freeze, and any later check built on it would freeze too.
- **`frag_iter`: `data + 0.25`, wrapping.** Exact in f32, but its state repeats every 4 ticks.
- **`frag_iter`: a rule that reads the letter.** It would change the query from one shared type to the 26 letter types, which is not the workload.
- **`heavy_compute`: keep the rule and pin the checksum.** The step is the identity on the matrix in exact arithmetic; the checksum moves only through rounding drift in 37 entities, so for the other 963 a backend that stopped inverting would pass. (First recommended, then withdrawn; `log.md`.)
- **`heavy_compute`: 101 inversions.** Every entity changes, but the state alternates with period 2 (843 of 1,000 checksums repeat an earlier tick).
- **`add_remove`: make the checksum see `B`** (for example, update `A` from `B` before the removal). Conformance would then cover every backend, but it changes what is measured, and a backend could still compute the value without the structural change, which is the part the workload measures; a direct test of the removal list checks the structure.
- **`add_remove`: only the three backends #141 names.** The others' tests see the insert only after the first step (§4.1).

## 8. Verification

| Requirement | Verified by |
|---|---|
| R1 | The conformance tests of `frag_iter` and `heavy_compute` in each backend crate; harness `update` unit tests; `cargo xtask ci` |
| R2 | The two `vec_aos` tests; the old-rule red check in `log.md` |
| R3 | The ten tests and the `vec_aos` test; the red checks in `log.md` (`--no-fail-fast` output) |
| R4 | `grouped_heavy_compute_uses_the_group`; the `group::<...>()` call in `setup`, checked in review |
| R5 | `review-docs`; `git grep -n -e '0.5 + 1.0' -e 'converges to 2' -- ':!docs/chunks/'` finds nothing, and in `docs/chunks/m3.2-spike-harness/spec.md` it matches only the amendment notes (other chunk folders are historical records and keep the old text) |
| R6 | The pull request, `log.md` and the #102 comment; `/bench-run`'s power log |
| R7 | Accessors and their imports under `#[cfg(test)]`, checked in review |

## 9. Done when

- [ ] `frag_iter` and `heavy_compute` follow §4.2, and every backend passes their conformance.
- [ ] The two `vec_aos` tests pass and were seen failing with the old rules.
- [ ] Ten `add_remove` per-step tests pass and were seen failing with the insert removed and at tick 1 only.
- [ ] The M3.2 spec, harness docs, spike README and changelog state the new rules.
- [ ] The informal run is in the pull request, `log.md` and on #102.
- [ ] `cargo xtask ci` passes.

## 10. Open questions

None.

## 11. Prior art and sources

Omitted (small chunk). The simulation is in `log.md`; ecs-11 and ecs-13 in [docs/lessons/ecs.md](../../lessons/ecs.md).
