# Mutation testing triage

- **Status:** Living record. Last triage: 2026-11-13 (M3.1, `engine::collections`).
- **Spec:** [M2.3](chunks/m2.3-mutation-testing/spec.md)

This is the hand-written record of what `cargo xtask mutants` found in each triaged module and what was done about every missed mutant and timeout. The classes are defined in [CONTRIBUTING.md](../CONTRIBUTING.md#mutation-testing): *test gap, fixed*, *test gap, deferred*, *bug*, *equivalent* and *not worth a test* (the last two are excluded in `.cargo/mutants.toml`, with the reason there), plus *detected (hang)* for a timeout. Names carry the line and column at the commit of the triage run, so they drift as the files change; the exclusion entries do not use positions. To triage another crate, add it to `TRIAGED` in `xtask/src/mutants.rs`, run the command, and add a section here with a row for each missed mutant and timeout, in file and line order.

All runs: `cargo-mutants` 27.1.0, `cargo xtask mutants -j 2`, macOS, AC power. A mutant near the 20-second limit can flip between caught and timeout from run to run (see the confirming-run note below).

## Summary of the first triage

| | Triage run (commit `8518dde`) | Confirming run (after the new tests and exclusions) |
|---|---|---|
| Mutants tested | 458 | 454 (4 excluded by 4 entries) |
| Caught | 271 | 312 |
| Missed | 48 | 2 |
| Timeout | 15 | 16 |
| Unviable | 124 | 124 |
| Wall time | 10 min 0 s | 10 min 55 s |

Per module, before and after:

| Crate | Missed before | Missed after | Timeout before | Timeout after |
|---|---|---|---|---|
| engine::handle | 34 | 1 | 3 | 3 |
| engine::random | 14 | 1 | 12 | 13 |

The unviable mutants are all compile errors (type, arity, missing macro in `no_std`, const evaluation), none caused by a lint, so `cap_lints` works as intended.

Two things the tool does that shaped the triage:

- `cargo-mutants` 27.1 never applies a name filter (`-F`, `-E` or `exclude_re`) to its "delete field ... from struct Self expression" mutants, so they cannot be excluded. The one missed in `engine::handle` was closed with a test.
- Two equivalent mutants are **not excluded** because their description is the same as that of a caught mutant in the same function and an entry cannot tell them apart without a line or column number. They stay in the missed list. This differs from the spec's R18, which expected only deferred gaps and bugs there. No issue is opened for them, because there is nothing to fix.

Confirming-run note on timeouts: `prng.rs:130:26` and `prng.rs:130:40` were caught in the triage run and timed out in the confirming run, and `prng.rs:6:27` went the other way. With nextest's fail-fast, whether a mutant that both breaks and hangs a test shows as caught or as timeout depends on which test runs first. Both outcomes detect the mutant. The rows below are the triage run's.

## engine::handle

> **Note (2026-11-14, M3.11).** The module `engine::handle` is now `engine::genid` ([ADR 0012](adr/0012-generational-ids.md)),
> with `HandlePool` renamed `GenIdMap`, `HandleAllocator` renamed `GenIdAllocator` and `Handle` renamed `GenId`;
> `pool.rs` and `handle.rs` are now `map.rs` and `id.rs`. The rows below keep the old names. The
> `dense_allocator.rs` rows refer to a file that has been deleted, and the `slot_limit` row refers to a
> function that has been deleted.

| Mutant | Class | Action |
|---|---|---|
| `libs/engine/src/handle/allocator.rs:97:9: replace HandleAllocator<T>::with_capacity -> Self with Default::default()` | test gap, fixed | `with_capacity_reserves_slots_up_front` (allocator.rs) |
| `libs/engine/src/handle/allocator.rs:360:9: replace HandleAllocator<T>::is_empty -> bool with true` | test gap, fixed | `emptiness_follows_the_live_count` (contracts.rs) |
| `libs/engine/src/handle/allocator.rs:410:9: replace <impl Clone for HandleAllocator<T>>::clone -> Self with Default::default()` | test gap, fixed | `a_clone_is_independent_and_continues_identically` (contracts.rs) |
| `libs/engine/src/handle/dense_allocator.rs:85:9: replace DenseHandleAllocator<T>::with_capacity -> Self with Default::default()` | test gap, fixed | `fixed_capacity_containers_never_allocate_after_construction` (fixed_capacity.rs) |
| `libs/engine/src/handle/dense_allocator.rs:274:9: replace DenseHandleAllocator<T>::is_empty -> bool with true` | test gap, fixed | `emptiness_follows_the_live_count` (contracts.rs) |
| `libs/engine/src/handle/dense_allocator.rs:337:9: replace DenseHandleAllocator<T>::check_invariants -> Result<(), crate::InvariantViolation> with Ok(())` | test gap, fixed | `check_invariants_reports_a_violation_instead_of_panicking` (dense_allocator.rs) |
| `libs/engine/src/handle/dense_allocator.rs:349:9: replace <impl Clone for DenseHandleAllocator<T>>::clone -> Self with Default::default()` | test gap, fixed | `a_clone_is_independent_and_continues_identically` (contracts.rs) |
| `libs/engine/src/handle/dense_allocator.rs:411:9: replace <impl fmt::Debug for DenseAllocIter<'_, T>>::fmt -> fmt::Result with Ok(Default::default())` | test gap, fixed | `iterator_debug_output_shows_the_remaining_count` (contracts.rs) |
| `libs/engine/src/handle/handle.rs:113:37: replace | with ^ in Handle<T>::to_bits` | equivalent | excluded: the index and generation bits never overlap |
| `libs/engine/src/handle/handle.rs:196:9: replace <impl Hash for Handle<T>>::hash with ()` | test gap, fixed | `hashing_distinguishes_handles_and_agrees_for_equal_ones` (contracts.rs) |
| `libs/engine/src/handle/handle.rs:208:21: replace match guard bytes.get(i + 1) == Some(&b':') with true in write_short_type_name` | equivalent | excluded: `type_name` has no lone `:` |
| `libs/engine/src/handle/handle.rs:208:33: replace + with * in write_short_type_name` | equivalent, not excluded | same cause as the entry above; no entry can match it without also matching the caught `+` mutants at lines 209 and 215 |
| `libs/engine/src/handle/handle.rs:210:19: replace += with -= in write_short_type_name` | detected (hang) | none |
| `libs/engine/src/handle/handle.rs:219:11: replace += with *= in write_short_type_name` | detected (hang) | none |
| `libs/engine/src/handle/pool.rs:114:9: replace HandlePool<T>::with_capacity -> Self with Default::default()` | test gap, fixed | `with_capacity_reserves_room_for_the_values` (contracts.rs) |
| `libs/engine/src/handle/pool.rs:172:9: replace HandlePool<T>::slot_limit -> u32 with 1` | test gap, fixed | `allocate_panics_naming_the_slot_limit` (dense_allocator.rs) |
| `libs/engine/src/handle/pool.rs:348:16: replace < with <= in HandlePool<T>::resolve` | equivalent | excluded: occupied links are below the dense length, others far above |
| `libs/engine/src/handle/pool.rs:392:9: replace HandlePool<T>::get_mut -> Option<&mut T> with None` | test gap, fixed | `get_mut_changes_the_stored_value_and_rejects_stale_handles` (contracts.rs) |
| `libs/engine/src/handle/pool.rs:473:9: replace HandlePool<T>::is_empty -> bool with true` | test gap, fixed | `emptiness_follows_the_live_count` (contracts.rs) |
| `libs/engine/src/handle/pool.rs:566:23: replace += with *= in HandlePool<T>::retain` | detected (hang) | none |
| `libs/engine/src/handle/pool/iter.rs:74:9: replace <impl Iterator for IterMut<'a, T>>::size_hint -> (usize, Option<usize>) with (0, None)` | test gap, fixed | `iterators_report_exact_sizes_as_they_advance` (contracts.rs) |
| `libs/engine/src/handle/pool/iter.rs:74:9: replace <impl Iterator for IterMut<'a, T>>::size_hint -> (usize, Option<usize>) with (0, Some(0))` | test gap, fixed | `iterators_report_exact_sizes_as_they_advance` (contracts.rs) |
| `libs/engine/src/handle/pool/iter.rs:74:9: replace <impl Iterator for IterMut<'a, T>>::size_hint -> (usize, Option<usize>) with (0, Some(1))` | test gap, fixed | `iterators_report_exact_sizes_as_they_advance` (contracts.rs) |
| `libs/engine/src/handle/pool/iter.rs:74:9: replace <impl Iterator for IterMut<'a, T>>::size_hint -> (usize, Option<usize>) with (1, None)` | test gap, fixed | `iterators_report_exact_sizes_as_they_advance` (contracts.rs) |
| `libs/engine/src/handle/pool/iter.rs:74:9: replace <impl Iterator for IterMut<'a, T>>::size_hint -> (usize, Option<usize>) with (1, Some(0))` | test gap, fixed | `iterators_report_exact_sizes_as_they_advance` (contracts.rs) |
| `libs/engine/src/handle/pool/iter.rs:74:9: replace <impl Iterator for IterMut<'a, T>>::size_hint -> (usize, Option<usize>) with (1, Some(1))` | test gap, fixed | `iterators_report_exact_sizes_as_they_advance` (contracts.rs) |
| `libs/engine/src/handle/pool/iter.rs:127:9: replace <impl Iterator for IntoIter<T>>::size_hint -> (usize, Option<usize>) with (0, None)` | test gap, fixed | `iterators_report_exact_sizes_as_they_advance` (contracts.rs) |
| `libs/engine/src/handle/pool/iter.rs:127:9: replace <impl Iterator for IntoIter<T>>::size_hint -> (usize, Option<usize>) with (0, Some(0))` | test gap, fixed | `iterators_report_exact_sizes_as_they_advance` (contracts.rs) |
| `libs/engine/src/handle/pool/iter.rs:127:9: replace <impl Iterator for IntoIter<T>>::size_hint -> (usize, Option<usize>) with (0, Some(1))` | test gap, fixed | `iterators_report_exact_sizes_as_they_advance` (contracts.rs) |
| `libs/engine/src/handle/pool/iter.rs:127:9: replace <impl Iterator for IntoIter<T>>::size_hint -> (usize, Option<usize>) with (1, None)` | test gap, fixed | `iterators_report_exact_sizes_as_they_advance` (contracts.rs) |
| `libs/engine/src/handle/pool/iter.rs:127:9: replace <impl Iterator for IntoIter<T>>::size_hint -> (usize, Option<usize>) with (1, Some(0))` | test gap, fixed | `iterators_report_exact_sizes_as_they_advance` (contracts.rs) |
| `libs/engine/src/handle/pool/iter.rs:127:9: replace <impl Iterator for IntoIter<T>>::size_hint -> (usize, Option<usize>) with (1, Some(1))` | test gap, fixed | `iterators_report_exact_sizes_as_they_advance` (contracts.rs) |
| `libs/engine/src/handle/pool/iter.rs:136:9: replace <impl fmt::Debug for IntoIter<T>>::fmt -> fmt::Result with Ok(Default::default())` | test gap, fixed | `iterator_debug_output_shows_the_remaining_count` (contracts.rs) |
| `libs/engine/src/handle/slots.rs:63:28: replace | with ^ in Slot::set_state` | equivalent | excluded: `next` never has the top bit |
| `libs/engine/src/handle/slots.rs:102:13: delete field slots from struct Self expression in SlotTable::with_capacity` | test gap, fixed | `with_capacity_reserves_slots_up_front` (slots.rs) |
| `libs/engine/src/handle/slots.rs:272:34: replace += with -= in SlotTable::release_all` | test gap, fixed | `clear_retires_slots_whose_generation_is_exhausted` (allocator.rs) |
| `libs/engine/src/handle/slots.rs:272:34: replace += with *= in SlotTable::release_all` | test gap, fixed | `clear_retires_slots_whose_generation_is_exhausted` (allocator.rs) |

## engine::random

| Mutant | Class | Action |
|---|---|---|
| `libs/engine/src/random/prng.rs:6:27: replace / with *` | detected (hang) | none |
| `libs/engine/src/random/prng.rs:8:27: replace / with %` | detected (hang) | none |
| `libs/engine/src/random/prng.rs:8:27: replace / with *` | detected (hang) | none |
| `libs/engine/src/random/prng.rs:88:9: replace Prng::next_u64 -> u64 with 0` | detected (hang) | none |
| `libs/engine/src/random/prng.rs:92:13: replace ^= with &= in Prng::next_u64` | detected (hang) | none |
| `libs/engine/src/random/prng.rs:130:9: replace Prng::next_f32 -> f32 with 1.0` | detected (hang) | none |
| `libs/engine/src/random/prng.rs:148:9: replace Prng::next_f64 -> f64 with 1.0` | detected (hang) | none |
| `libs/engine/src/random/prng.rs:148:40: replace * with + in Prng::next_f64` | detected (hang) | none |
| `libs/engine/src/random/prng.rs:148:40: replace * with / in Prng::next_f64` | detected (hang) | none |
| `libs/engine/src/random/prng.rs:222:21: replace | with & in Prng::from_state` | test gap, fixed | `from_state_rejects_only_the_all_zero_state` (edge_cases.rs) |
| `libs/engine/src/random/prng.rs:222:21: replace | with ^ in Prng::from_state` | test gap, fixed | `from_state_rejects_only_the_all_zero_state` (edge_cases.rs) |
| `libs/engine/src/random/prng.rs:222:32: replace | with & in Prng::from_state` | test gap, fixed | `from_state_rejects_only_the_all_zero_state` (edge_cases.rs) |
| `libs/engine/src/random/prng.rs:222:32: replace | with ^ in Prng::from_state` | test gap, fixed | `from_state_rejects_only_the_all_zero_state` (edge_cases.rs) |
| `libs/engine/src/random/prng.rs:222:43: replace | with ^ in Prng::from_state` | test gap, fixed | `from_state_rejects_only_the_all_zero_state` (edge_cases.rs) |
| `libs/engine/src/random/sample.rs:182:15: replace < with == in Prng::bounded_u64` | test gap, fixed | `bounded_ranges_match_the_rejection_rule_for_spans_that_reject_often` (oracle.rs) |
| `libs/engine/src/random/sample.rs:182:15: replace < with > in Prng::bounded_u64` | test gap, fixed | `bounded_ranges_match_the_rejection_rule_for_spans_that_reject_often` (oracle.rs) |
| `libs/engine/src/random/sample.rs:182:15: replace < with <= in Prng::bounded_u64` | equivalent, not excluded | `lo == n` never enters the loop either way; no entry can match it without also matching the caught `<=` at line 184 |
| `libs/engine/src/random/sample.rs:183:46: replace % with / in Prng::bounded_u64` | test gap, fixed | `bounded_ranges_match_the_rejection_rule_for_spans_that_reject_often` (oracle.rs) |
| `libs/engine/src/random/sample.rs:184:22: replace < with == in Prng::bounded_u64` | test gap, fixed | `bounded_ranges_match_the_rejection_rule_for_spans_that_reject_often` (oracle.rs) |
| `libs/engine/src/random/sample.rs:184:22: replace < with > in Prng::bounded_u64` | test gap, fixed | `bounded_ranges_match_the_rejection_rule_for_spans_that_reject_often` (oracle.rs) |
| `libs/engine/src/random/sample.rs:184:22: replace < with <= in Prng::bounded_u64` | test gap, fixed | `a_draw_exactly_at_the_rejection_threshold_is_accepted` (oracle.rs) |
| `libs/engine/src/random/sample.rs:185:49: replace * with + in Prng::bounded_u64` | test gap, fixed | `bounded_ranges_match_the_rejection_rule_for_spans_that_reject_often` (oracle.rs) |
| `libs/engine/src/random/sample.rs:185:49: replace * with / in Prng::bounded_u64` | detected (hang) | none |
| `libs/engine/src/random/sample.rs:274:25: replace < with <= in Prng::chance` | test gap, fixed | `chance_is_true_only_strictly_below_p` (edge_cases.rs) |
| `libs/engine/src/random/splitmix.rs:9:5: replace mix64 -> u64 with 0` | detected (hang) | none |
| `libs/engine/src/random/splitmix.rs:27:9: replace SplitMix64::next_u64 -> u64 with 0` | detected (hang) | none |

## engine::collections

Triaged in M3.1. Every missed mutant of the first run was a test gap in behaviour the model and law tests do not observe (capacity, mutable access by position, iterator sizes, `Debug` output, the set's `Hash`); all were closed with tests in `tests/contracts.rs`. No mutant timed out and none needed an exclusion. The names carry the line and column at the first run.

| | Triage run | Confirming run (after the new tests) |
|---|---|---|
| Mutants tested | 178 | 178 |
| Caught | 81 | 117 |
| Missed | 36 | 0 |
| Timeout | 0 | 0 |
| Unviable | 61 | 61 |

Both runs: `cargo-mutants` 27.1.0, `cargo xtask mutants --module collections -j 2`, macOS.

| Mutant | Class | Action |
|---|---|---|
| `libs/engine/src/collections/index_map.rs:98:9: replace IndexMap<K, V>::shrink_to_fit with ()` | test gap, fixed | `shrink_to_fit_releases_unused_capacity` (contracts.rs) |
| `libs/engine/src/collections/index_map.rs:122:9: replace IndexMap<K, V>::get_index_mut -> Option<(&K, &mut V)> with None` | test gap, fixed | `positional_mutable_access_changes_the_stored_value` (contracts.rs) |
| `libs/engine/src/collections/index_map.rs:132:9: replace IndexMap<K, V>::first_mut -> Option<(&K, &mut V)> with None` | test gap, fixed | `positional_mutable_access_changes_the_stored_value` (contracts.rs) |
| `libs/engine/src/collections/index_map.rs:142:9: replace IndexMap<K, V>::last_mut -> Option<(&K, &mut V)> with None` | test gap, fixed | `positional_mutable_access_changes_the_stored_value` (contracts.rs) |
| `libs/engine/src/collections/index_map/entry.rs:65:9: replace <impl fmt::Debug for Entry<'_, K, V>>::fmt -> fmt::Result with Ok(Default::default())` | test gap, fixed | `owning_and_mutable_iterators_and_entries_have_nonempty_debug_output` (contracts.rs) |
| `libs/engine/src/collections/index_map/iter.rs:67:9: replace <impl fmt::Debug for Iter<'_, K, V>>::fmt -> fmt::Result with Ok(Default::default())` | test gap, fixed | `iterator_debug_output_shows_the_remaining_items` (contracts.rs) |
| `libs/engine/src/collections/index_map/iter.rs:101:9: replace <impl fmt::Debug for Keys<'_, K, V>>::fmt -> fmt::Result with Ok(Default::default())` | test gap, fixed | `iterator_debug_output_shows_the_remaining_items` (contracts.rs) |
| `libs/engine/src/collections/index_map/iter.rs:129:9: replace <impl fmt::Debug for IterMut<'_, K, V>>::fmt -> fmt::Result with Ok(Default::default())` | test gap, fixed | `owning_and_mutable_iterators_and_entries_have_nonempty_debug_output` (contracts.rs) |
| `libs/engine/src/collections/index_map/iter.rs:162:9: replace <impl fmt::Debug for Values<'_, K, V>>::fmt -> fmt::Result with Ok(Default::default())` | test gap, fixed | `iterator_debug_output_shows_the_remaining_items` (contracts.rs) |
| `libs/engine/src/collections/index_map/iter.rs:190:9: replace <impl fmt::Debug for ValuesMut<'_, K, V>>::fmt -> fmt::Result with Ok(Default::default())` | test gap, fixed | `owning_and_mutable_iterators_and_entries_have_nonempty_debug_output` (contracts.rs) |
| `libs/engine/src/collections/index_map/iter.rs:215:9: replace <impl fmt::Debug for IntoIter<K, V>>::fmt -> fmt::Result with Ok(Default::default())` | test gap, fixed | `owning_and_mutable_iterators_and_entries_have_nonempty_debug_output` (contracts.rs) |
| `libs/engine/src/collections/index_map/iter.rs:240:9: replace <impl fmt::Debug for IntoKeys<K, V>>::fmt -> fmt::Result with Ok(Default::default())` | test gap, fixed | `owning_and_mutable_iterators_and_entries_have_nonempty_debug_output` (contracts.rs) |
| `libs/engine/src/collections/index_map/iter.rs:265:9: replace <impl fmt::Debug for IntoValues<K, V>>::fmt -> fmt::Result with Ok(Default::default())` | test gap, fixed | `owning_and_mutable_iterators_and_entries_have_nonempty_debug_output` (contracts.rs) |
| `libs/engine/src/collections/index_set.rs:73:9: replace IndexSet<T>::capacity -> usize with 0` | test gap, fixed | `set_capacity_follows_reservations` (contracts.rs) |
| `libs/engine/src/collections/index_set.rs:73:9: replace IndexSet<T>::capacity -> usize with 1` | test gap, fixed | `set_capacity_follows_reservations` (contracts.rs) |
| `libs/engine/src/collections/index_set.rs:84:9: replace IndexSet<T>::reserve with ()` | test gap, fixed | `set_capacity_follows_reservations` (contracts.rs) |
| `libs/engine/src/collections/index_set.rs:98:9: replace IndexSet<T>::try_reserve -> Result<(), TryReserveError> with Ok(())` | test gap, fixed | `set_try_reserve_overflow_is_an_error_and_leaves_the_set_usable` (contracts.rs) |
| `libs/engine/src/collections/index_set.rs:105:9: replace IndexSet<T>::shrink_to_fit with ()` | test gap, fixed | `shrink_to_fit_releases_unused_capacity` (contracts.rs) |
| `libs/engine/src/collections/index_set.rs:267:9: replace <impl Hash for IndexSet<T>>::hash with ()` | test gap, fixed | `set_hash_depends_on_length_and_contents` (contracts.rs) |
| `libs/engine/src/collections/index_set/iter.rs:32:9: replace <impl Iterator for Iter<'a, T>>::size_hint -> (usize, Option<usize>) with (0, None)` | test gap, fixed | `set_iterators_report_exact_sizes_as_they_advance` (contracts.rs) |
| `libs/engine/src/collections/index_set/iter.rs:32:9: replace <impl Iterator for Iter<'a, T>>::size_hint -> (usize, Option<usize>) with (0, Some(0))` | test gap, fixed | `set_iterators_report_exact_sizes_as_they_advance` (contracts.rs) |
| `libs/engine/src/collections/index_set/iter.rs:32:9: replace <impl Iterator for Iter<'a, T>>::size_hint -> (usize, Option<usize>) with (0, Some(1))` | test gap, fixed | `set_iterators_report_exact_sizes_as_they_advance` (contracts.rs) |
| `libs/engine/src/collections/index_set/iter.rs:32:9: replace <impl Iterator for Iter<'a, T>>::size_hint -> (usize, Option<usize>) with (1, None)` | test gap, fixed | `set_iterators_report_exact_sizes_as_they_advance` (contracts.rs) |
| `libs/engine/src/collections/index_set/iter.rs:32:9: replace <impl Iterator for Iter<'a, T>>::size_hint -> (usize, Option<usize>) with (1, Some(0))` | test gap, fixed | `set_iterators_report_exact_sizes_as_they_advance` (contracts.rs) |
| `libs/engine/src/collections/index_set/iter.rs:32:9: replace <impl Iterator for Iter<'a, T>>::size_hint -> (usize, Option<usize>) with (1, Some(1))` | test gap, fixed | `set_iterators_report_exact_sizes_as_they_advance` (contracts.rs) |
| `libs/engine/src/collections/index_set/iter.rs:60:9: replace <impl fmt::Debug for Iter<'_, T>>::fmt -> fmt::Result with Ok(Default::default())` | test gap, fixed | `iterator_debug_output_shows_the_remaining_items` (contracts.rs) |
| `libs/engine/src/collections/index_set/iter.rs:85:9: replace <impl Iterator for IntoIter<T>>::next -> Option<T> with None` | test gap, fixed | `set_iterators_report_exact_sizes_as_they_advance` (contracts.rs) |
| `libs/engine/src/collections/index_set/iter.rs:89:9: replace <impl Iterator for IntoIter<T>>::size_hint -> (usize, Option<usize>) with (0, None)` | test gap, fixed | `set_iterators_report_exact_sizes_as_they_advance` (contracts.rs) |
| `libs/engine/src/collections/index_set/iter.rs:89:9: replace <impl Iterator for IntoIter<T>>::size_hint -> (usize, Option<usize>) with (0, Some(0))` | test gap, fixed | `set_iterators_report_exact_sizes_as_they_advance` (contracts.rs) |
| `libs/engine/src/collections/index_set/iter.rs:89:9: replace <impl Iterator for IntoIter<T>>::size_hint -> (usize, Option<usize>) with (0, Some(1))` | test gap, fixed | `set_iterators_report_exact_sizes_as_they_advance` (contracts.rs) |
| `libs/engine/src/collections/index_set/iter.rs:89:9: replace <impl Iterator for IntoIter<T>>::size_hint -> (usize, Option<usize>) with (1, None)` | test gap, fixed | `set_iterators_report_exact_sizes_as_they_advance` (contracts.rs) |
| `libs/engine/src/collections/index_set/iter.rs:89:9: replace <impl Iterator for IntoIter<T>>::size_hint -> (usize, Option<usize>) with (1, Some(0))` | test gap, fixed | `set_iterators_report_exact_sizes_as_they_advance` (contracts.rs) |
| `libs/engine/src/collections/index_set/iter.rs:89:9: replace <impl Iterator for IntoIter<T>>::size_hint -> (usize, Option<usize>) with (1, Some(1))` | test gap, fixed | `set_iterators_report_exact_sizes_as_they_advance` (contracts.rs) |
| `libs/engine/src/collections/index_set/iter.rs:101:9: replace <impl ExactSizeIterator for IntoIter<T>>::len -> usize with 0` | test gap, fixed | `set_iterators_report_exact_sizes_as_they_advance` (contracts.rs) |
| `libs/engine/src/collections/index_set/iter.rs:101:9: replace <impl ExactSizeIterator for IntoIter<T>>::len -> usize with 1` | test gap, fixed | `set_iterators_report_exact_sizes_as_they_advance` (contracts.rs) |
| `libs/engine/src/collections/index_set/iter.rs:109:9: replace <impl fmt::Debug for IntoIter<T>>::fmt -> fmt::Result with Ok(Default::default())` | test gap, fixed | `owning_and_mutable_iterators_and_entries_have_nonempty_debug_output` (contracts.rs) |

### Bitsets (M3.14)

`BitSet`, `BitArray`, their iterators and the shared word routines. Three tests closed the `Hash`, `Extend<&usize>` and `From<[usize; N]>` gaps and one closed the `MAX_BITS` boundary of `try_ensure_capacity`. Two comparisons were removed or restructured so that their mutants no longer exist (an `if` that only skipped a no-op reservation, and a reserved amount that `resize` makes irrelevant), and two equivalent mutants are excluded in `.cargo/mutants.toml`. The eight timeouts are mutants that make an iterator or its loop never end; a test that does not finish detects them, so they need no action.

| | Triage run | Confirming run (after the new tests) |
|---|---|---|
| Mutants tested | 453 | 446 |
| Caught | 344 | 345 |
| Missed | 9 | 1 (code then restructured, see below) |
| Timeout | 8 | 8 |
| Unviable | 92 | 92 |

Both runs: `cargo-mutants` 27.1.0, `cargo xtask mutants --module collections -j 1`, in a Linux (aarch64) container limited to 4 GB of memory, with the other settings of `.cargo/mutants.toml`. They were not run on macOS with `-j 2`, because the first attempt that way exhausted the machine's memory (below). Peak container memory was 1,541 MiB in the triage run and 922 MiB in the confirming run. The counts include the `IndexMap` and `IndexSet` mutants of M3.1, which are unchanged.

After the confirming run, the code review of the branch led to `#[inline]` on the hot methods, a `get_mut` fast path in `try_insert`, and panic messages that name `from_iter` and `from`. A targeted run of the 28 mutants in `try_insert`, `grow_to_hold`, `extend`, `from_iter`, `From`, `try_ensure_capacity` and `words_for_bits` caught all 28, which includes a new `Extend<usize>` test that closed a gap the change opened.

**Memory.** A mutant that makes an iterator endless (`next` replaced by `Some(0)`) turns every test that collects it into an unbounded `Vec`, and every test that formats it into an unbounded `String`, until the timeout. That used up the memory of the first attempt. Tests that walk an iterator or `Debug` output of the code under test now use `bounded` and `debug_bounded` (`collections/tests/mod.rs`), which stop at a limit, and mutation runs go through a memory-limited container.

| Mutant | Class | Action |
|---|---|---|
| `bit_set.rs:451:9: replace <impl Hash for BitSet>::hash with ()` | test gap, fixed | `hash_depends_on_the_members` (bit_set.rs) |
| `bit_set.rs:494:9: replace <impl Extend<&'a usize> for BitSet>::extend with ()` | test gap, fixed | `extend_from_references_and_from_array_add_the_indices` (bit_set.rs) |
| `bit_set.rs:505:9: replace <impl From<[usize; N]> for BitSet>::from -> Self with Default::default()` | test gap, fixed | `extend_from_references_and_from_array_add_the_indices` (bit_set.rs) |
| `bit_set.rs:223:17: replace > with >= in BitSet::try_ensure_capacity` | test gap, fixed | `try_with_capacity_accepts_max_bits_and_rejects_one_more` (bit_set.rs; 64-bit targets) |
| `bit_set.rs:227:19: replace > with >= in BitSet::try_ensure_capacity` | equivalent (code removed) | the guard only skipped a reservation that is a no-op when the capacity already covers it, so it was removed |
| `bit_set.rs:229:43: replace - with + in BitSet::try_ensure_capacity` | not worth a test (code restructured) | over-reserving by the current length is space only and `ensure_capacity` promises `capacity() >= bits`; the amount is now `saturating_sub`, which has no operator to mutate |
| `bit_set.rs:274:35: replace + with * in BitSet::try_insert` | equivalent (code restructured) | `resize` grows the vector whatever was reserved; growth now lives in `grow_to_hold`, so the reservation is no longer a separate expression |
| `bit_set.rs:340:22: replace > with >= in BitSet::union_with` | equivalent | excluded: an equal length makes the resize a no-op |
| `bit_set.rs:361:22: replace > with >= in BitSet::symmetric_difference_with` | equivalent | excluded: as for `union_with` |
| `bit_set.rs:272:19: replace > with >= in BitSet::try_insert` (confirming run) | equivalent (code restructured) | `needed > len` no longer exists: `try_insert` tries `get_mut` first and calls `grow_to_hold` only when the word is missing |
| `bit_set/iter.rs:34:9: replace Cursor::next -> Option<usize> with Some(0)` and `with Some(1)` | timeout, detected | the iterator never ends |
| `bit_set/iter.rs:37:33: replace &= with \|= in Cursor::next` | timeout, detected | the lowest bit is never cleared |
| `bit_set/iter.rs:37:52: replace - with + in Cursor::next` and `replace - with /` | timeout, detected | the clearing expression never clears the lowest bit |
| `bit_set/iter.rs:46:24: replace += with *= in Cursor::next` | timeout, detected | the word index never advances |
| `bit_set/iter.rs:118:9: replace <impl Iterator for Iter<'_>>::next -> Option<usize> with Some(0)` and `with Some(1)` | timeout, detected | the iterator never ends |
