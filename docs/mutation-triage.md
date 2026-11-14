# Mutation testing triage

- **Status:** Living record. Last triage: 2026-11-13 (M3.1, `engine_collections`).
- **Spec:** [M2.3](design/m2-infrastructure/m2.3-mutation-testing/spec.md)

This is the hand-written record of what `cargo xtask mutants` found in each triaged crate and what was done about every missed mutant and timeout. The classes are defined in [CONTRIBUTING.md](../CONTRIBUTING.md#mutation-testing): *test gap, fixed*, *test gap, deferred*, *bug*, *equivalent* and *not worth a test* (the last two are excluded in `.cargo/mutants.toml`, with the reason there), plus *detected (hang)* for a timeout. Names carry the line and column at the commit of the triage run, so they drift as the files change; the exclusion entries do not use positions. To triage another crate, add it to `TRIAGED` in `xtask/src/mutants.rs`, run the command, and add a section here with a row for each missed mutant and timeout, in file and line order.

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

Per crate, before and after:

| Crate | Missed before | Missed after | Timeout before | Timeout after |
|---|---|---|---|---|
| engine_handle | 34 | 1 | 3 | 3 |
| engine_random | 14 | 1 | 12 | 13 |

The unviable mutants are all compile errors (type, arity, missing macro in `no_std`, const evaluation), none caused by a lint, so `cap_lints` works as intended.

Two things the tool does that shaped the triage:

- `cargo-mutants` 27.1 never applies a name filter (`-F`, `-E` or `exclude_re`) to its "delete field ... from struct Self expression" mutants, so they cannot be excluded. The one missed in `engine_handle` was closed with a test.
- Two equivalent mutants are **not excluded** because their description is the same as that of a caught mutant in the same function and an entry cannot tell them apart without a line or column number. They stay in the missed list. This differs from the spec's R18, which expected only deferred gaps and bugs there. No issue is opened for them, because there is nothing to fix.

Confirming-run note on timeouts: `prng.rs:130:26` and `prng.rs:130:40` were caught in the triage run and timed out in the confirming run, and `prng.rs:6:27` went the other way. With nextest's fail-fast, whether a mutant that both breaks and hangs a test shows as caught or as timeout depends on which test runs first. Both outcomes detect the mutant. The rows below are the triage run's.

## engine_handle

| Mutant | Class | Action |
|---|---|---|
| `libs/engine_handle/src/allocator.rs:97:9: replace HandleAllocator<T>::with_capacity -> Self with Default::default()` | test gap, fixed | `with_capacity_reserves_slots_up_front` (allocator.rs) |
| `libs/engine_handle/src/allocator.rs:360:9: replace HandleAllocator<T>::is_empty -> bool with true` | test gap, fixed | `emptiness_follows_the_live_count` (contracts.rs) |
| `libs/engine_handle/src/allocator.rs:410:9: replace <impl Clone for HandleAllocator<T>>::clone -> Self with Default::default()` | test gap, fixed | `a_clone_is_independent_and_continues_identically` (contracts.rs) |
| `libs/engine_handle/src/dense_allocator.rs:85:9: replace DenseHandleAllocator<T>::with_capacity -> Self with Default::default()` | test gap, fixed | `fixed_capacity_containers_never_allocate_after_construction` (fixed_capacity.rs) |
| `libs/engine_handle/src/dense_allocator.rs:274:9: replace DenseHandleAllocator<T>::is_empty -> bool with true` | test gap, fixed | `emptiness_follows_the_live_count` (contracts.rs) |
| `libs/engine_handle/src/dense_allocator.rs:337:9: replace DenseHandleAllocator<T>::check_invariants -> Result<(), crate::InvariantViolation> with Ok(())` | test gap, fixed | `check_invariants_reports_a_violation_instead_of_panicking` (dense_allocator.rs) |
| `libs/engine_handle/src/dense_allocator.rs:349:9: replace <impl Clone for DenseHandleAllocator<T>>::clone -> Self with Default::default()` | test gap, fixed | `a_clone_is_independent_and_continues_identically` (contracts.rs) |
| `libs/engine_handle/src/dense_allocator.rs:411:9: replace <impl fmt::Debug for DenseAllocIter<'_, T>>::fmt -> fmt::Result with Ok(Default::default())` | test gap, fixed | `iterator_debug_output_shows_the_remaining_count` (contracts.rs) |
| `libs/engine_handle/src/handle.rs:113:37: replace | with ^ in Handle<T>::to_bits` | equivalent | excluded: the index and generation bits never overlap |
| `libs/engine_handle/src/handle.rs:196:9: replace <impl Hash for Handle<T>>::hash with ()` | test gap, fixed | `hashing_distinguishes_handles_and_agrees_for_equal_ones` (contracts.rs) |
| `libs/engine_handle/src/handle.rs:208:21: replace match guard bytes.get(i + 1) == Some(&b':') with true in write_short_type_name` | equivalent | excluded: `type_name` has no lone `:` |
| `libs/engine_handle/src/handle.rs:208:33: replace + with * in write_short_type_name` | equivalent, not excluded | same cause as the entry above; no entry can match it without also matching the caught `+` mutants at lines 209 and 215 |
| `libs/engine_handle/src/handle.rs:210:19: replace += with -= in write_short_type_name` | detected (hang) | none |
| `libs/engine_handle/src/handle.rs:219:11: replace += with *= in write_short_type_name` | detected (hang) | none |
| `libs/engine_handle/src/pool.rs:114:9: replace HandlePool<T>::with_capacity -> Self with Default::default()` | test gap, fixed | `with_capacity_reserves_room_for_the_values` (contracts.rs) |
| `libs/engine_handle/src/pool.rs:172:9: replace HandlePool<T>::slot_limit -> u32 with 1` | test gap, fixed | `allocate_panics_naming_the_slot_limit` (dense_allocator.rs) |
| `libs/engine_handle/src/pool.rs:348:16: replace < with <= in HandlePool<T>::resolve` | equivalent | excluded: occupied links are below the dense length, others far above |
| `libs/engine_handle/src/pool.rs:392:9: replace HandlePool<T>::get_mut -> Option<&mut T> with None` | test gap, fixed | `get_mut_changes_the_stored_value_and_rejects_stale_handles` (contracts.rs) |
| `libs/engine_handle/src/pool.rs:473:9: replace HandlePool<T>::is_empty -> bool with true` | test gap, fixed | `emptiness_follows_the_live_count` (contracts.rs) |
| `libs/engine_handle/src/pool.rs:566:23: replace += with *= in HandlePool<T>::retain` | detected (hang) | none |
| `libs/engine_handle/src/pool/iter.rs:74:9: replace <impl Iterator for IterMut<'a, T>>::size_hint -> (usize, Option<usize>) with (0, None)` | test gap, fixed | `iterators_report_exact_sizes_as_they_advance` (contracts.rs) |
| `libs/engine_handle/src/pool/iter.rs:74:9: replace <impl Iterator for IterMut<'a, T>>::size_hint -> (usize, Option<usize>) with (0, Some(0))` | test gap, fixed | `iterators_report_exact_sizes_as_they_advance` (contracts.rs) |
| `libs/engine_handle/src/pool/iter.rs:74:9: replace <impl Iterator for IterMut<'a, T>>::size_hint -> (usize, Option<usize>) with (0, Some(1))` | test gap, fixed | `iterators_report_exact_sizes_as_they_advance` (contracts.rs) |
| `libs/engine_handle/src/pool/iter.rs:74:9: replace <impl Iterator for IterMut<'a, T>>::size_hint -> (usize, Option<usize>) with (1, None)` | test gap, fixed | `iterators_report_exact_sizes_as_they_advance` (contracts.rs) |
| `libs/engine_handle/src/pool/iter.rs:74:9: replace <impl Iterator for IterMut<'a, T>>::size_hint -> (usize, Option<usize>) with (1, Some(0))` | test gap, fixed | `iterators_report_exact_sizes_as_they_advance` (contracts.rs) |
| `libs/engine_handle/src/pool/iter.rs:74:9: replace <impl Iterator for IterMut<'a, T>>::size_hint -> (usize, Option<usize>) with (1, Some(1))` | test gap, fixed | `iterators_report_exact_sizes_as_they_advance` (contracts.rs) |
| `libs/engine_handle/src/pool/iter.rs:127:9: replace <impl Iterator for IntoIter<T>>::size_hint -> (usize, Option<usize>) with (0, None)` | test gap, fixed | `iterators_report_exact_sizes_as_they_advance` (contracts.rs) |
| `libs/engine_handle/src/pool/iter.rs:127:9: replace <impl Iterator for IntoIter<T>>::size_hint -> (usize, Option<usize>) with (0, Some(0))` | test gap, fixed | `iterators_report_exact_sizes_as_they_advance` (contracts.rs) |
| `libs/engine_handle/src/pool/iter.rs:127:9: replace <impl Iterator for IntoIter<T>>::size_hint -> (usize, Option<usize>) with (0, Some(1))` | test gap, fixed | `iterators_report_exact_sizes_as_they_advance` (contracts.rs) |
| `libs/engine_handle/src/pool/iter.rs:127:9: replace <impl Iterator for IntoIter<T>>::size_hint -> (usize, Option<usize>) with (1, None)` | test gap, fixed | `iterators_report_exact_sizes_as_they_advance` (contracts.rs) |
| `libs/engine_handle/src/pool/iter.rs:127:9: replace <impl Iterator for IntoIter<T>>::size_hint -> (usize, Option<usize>) with (1, Some(0))` | test gap, fixed | `iterators_report_exact_sizes_as_they_advance` (contracts.rs) |
| `libs/engine_handle/src/pool/iter.rs:127:9: replace <impl Iterator for IntoIter<T>>::size_hint -> (usize, Option<usize>) with (1, Some(1))` | test gap, fixed | `iterators_report_exact_sizes_as_they_advance` (contracts.rs) |
| `libs/engine_handle/src/pool/iter.rs:136:9: replace <impl fmt::Debug for IntoIter<T>>::fmt -> fmt::Result with Ok(Default::default())` | test gap, fixed | `iterator_debug_output_shows_the_remaining_count` (contracts.rs) |
| `libs/engine_handle/src/slots.rs:63:28: replace | with ^ in Slot::set_state` | equivalent | excluded: `next` never has the top bit |
| `libs/engine_handle/src/slots.rs:102:13: delete field slots from struct Self expression in SlotTable::with_capacity` | test gap, fixed | `with_capacity_reserves_slots_up_front` (slots.rs) |
| `libs/engine_handle/src/slots.rs:272:34: replace += with -= in SlotTable::release_all` | test gap, fixed | `clear_retires_slots_whose_generation_is_exhausted` (allocator.rs) |
| `libs/engine_handle/src/slots.rs:272:34: replace += with *= in SlotTable::release_all` | test gap, fixed | `clear_retires_slots_whose_generation_is_exhausted` (allocator.rs) |

## engine_random

| Mutant | Class | Action |
|---|---|---|
| `libs/engine_random/src/prng.rs:6:27: replace / with *` | detected (hang) | none |
| `libs/engine_random/src/prng.rs:8:27: replace / with %` | detected (hang) | none |
| `libs/engine_random/src/prng.rs:8:27: replace / with *` | detected (hang) | none |
| `libs/engine_random/src/prng.rs:88:9: replace Prng::next_u64 -> u64 with 0` | detected (hang) | none |
| `libs/engine_random/src/prng.rs:92:13: replace ^= with &= in Prng::next_u64` | detected (hang) | none |
| `libs/engine_random/src/prng.rs:130:9: replace Prng::next_f32 -> f32 with 1.0` | detected (hang) | none |
| `libs/engine_random/src/prng.rs:148:9: replace Prng::next_f64 -> f64 with 1.0` | detected (hang) | none |
| `libs/engine_random/src/prng.rs:148:40: replace * with + in Prng::next_f64` | detected (hang) | none |
| `libs/engine_random/src/prng.rs:148:40: replace * with / in Prng::next_f64` | detected (hang) | none |
| `libs/engine_random/src/prng.rs:222:21: replace | with & in Prng::from_state` | test gap, fixed | `from_state_rejects_only_the_all_zero_state` (edge_cases.rs) |
| `libs/engine_random/src/prng.rs:222:21: replace | with ^ in Prng::from_state` | test gap, fixed | `from_state_rejects_only_the_all_zero_state` (edge_cases.rs) |
| `libs/engine_random/src/prng.rs:222:32: replace | with & in Prng::from_state` | test gap, fixed | `from_state_rejects_only_the_all_zero_state` (edge_cases.rs) |
| `libs/engine_random/src/prng.rs:222:32: replace | with ^ in Prng::from_state` | test gap, fixed | `from_state_rejects_only_the_all_zero_state` (edge_cases.rs) |
| `libs/engine_random/src/prng.rs:222:43: replace | with ^ in Prng::from_state` | test gap, fixed | `from_state_rejects_only_the_all_zero_state` (edge_cases.rs) |
| `libs/engine_random/src/sample.rs:182:15: replace < with == in Prng::bounded_u64` | test gap, fixed | `bounded_ranges_match_the_rejection_rule_for_spans_that_reject_often` (oracle.rs) |
| `libs/engine_random/src/sample.rs:182:15: replace < with > in Prng::bounded_u64` | test gap, fixed | `bounded_ranges_match_the_rejection_rule_for_spans_that_reject_often` (oracle.rs) |
| `libs/engine_random/src/sample.rs:182:15: replace < with <= in Prng::bounded_u64` | equivalent, not excluded | `lo == n` never enters the loop either way; no entry can match it without also matching the caught `<=` at line 184 |
| `libs/engine_random/src/sample.rs:183:46: replace % with / in Prng::bounded_u64` | test gap, fixed | `bounded_ranges_match_the_rejection_rule_for_spans_that_reject_often` (oracle.rs) |
| `libs/engine_random/src/sample.rs:184:22: replace < with == in Prng::bounded_u64` | test gap, fixed | `bounded_ranges_match_the_rejection_rule_for_spans_that_reject_often` (oracle.rs) |
| `libs/engine_random/src/sample.rs:184:22: replace < with > in Prng::bounded_u64` | test gap, fixed | `bounded_ranges_match_the_rejection_rule_for_spans_that_reject_often` (oracle.rs) |
| `libs/engine_random/src/sample.rs:184:22: replace < with <= in Prng::bounded_u64` | test gap, fixed | `a_draw_exactly_at_the_rejection_threshold_is_accepted` (oracle.rs) |
| `libs/engine_random/src/sample.rs:185:49: replace * with + in Prng::bounded_u64` | test gap, fixed | `bounded_ranges_match_the_rejection_rule_for_spans_that_reject_often` (oracle.rs) |
| `libs/engine_random/src/sample.rs:185:49: replace * with / in Prng::bounded_u64` | detected (hang) | none |
| `libs/engine_random/src/sample.rs:274:25: replace < with <= in Prng::chance` | test gap, fixed | `chance_is_true_only_strictly_below_p` (edge_cases.rs) |
| `libs/engine_random/src/splitmix.rs:9:5: replace mix64 -> u64 with 0` | detected (hang) | none |
| `libs/engine_random/src/splitmix.rs:27:9: replace SplitMix64::next_u64 -> u64 with 0` | detected (hang) | none |

## engine_collections

Triaged in M3.1. Every missed mutant of the first run was a test gap in behaviour the model and law tests do not observe (capacity, mutable access by position, iterator sizes, `Debug` output, the set's `Hash`); all were closed with tests in `tests/contracts.rs`. No mutant timed out and none needed an exclusion. The names carry the line and column at the first run.

| | Triage run | Confirming run (after the new tests) |
|---|---|---|
| Mutants tested | 178 | 178 |
| Caught | 81 | 117 |
| Missed | 36 | 0 |
| Timeout | 0 | 0 |
| Unviable | 61 | 61 |

Both runs: `cargo-mutants` 27.1.0, `cargo xtask mutants -p engine_collections -j 2`, macOS.

| Mutant | Class | Action |
|---|---|---|
| `libs/engine_collections/src/index_map.rs:98:9: replace IndexMap<K, V>::shrink_to_fit with ()` | test gap, fixed | `shrink_to_fit_releases_unused_capacity` (contracts.rs) |
| `libs/engine_collections/src/index_map.rs:122:9: replace IndexMap<K, V>::get_index_mut -> Option<(&K, &mut V)> with None` | test gap, fixed | `positional_mutable_access_changes_the_stored_value` (contracts.rs) |
| `libs/engine_collections/src/index_map.rs:132:9: replace IndexMap<K, V>::first_mut -> Option<(&K, &mut V)> with None` | test gap, fixed | `positional_mutable_access_changes_the_stored_value` (contracts.rs) |
| `libs/engine_collections/src/index_map.rs:142:9: replace IndexMap<K, V>::last_mut -> Option<(&K, &mut V)> with None` | test gap, fixed | `positional_mutable_access_changes_the_stored_value` (contracts.rs) |
| `libs/engine_collections/src/index_map/entry.rs:65:9: replace <impl fmt::Debug for Entry<'_, K, V>>::fmt -> fmt::Result with Ok(Default::default())` | test gap, fixed | `owning_and_mutable_iterators_and_entries_have_nonempty_debug_output` (contracts.rs) |
| `libs/engine_collections/src/index_map/iter.rs:67:9: replace <impl fmt::Debug for Iter<'_, K, V>>::fmt -> fmt::Result with Ok(Default::default())` | test gap, fixed | `iterator_debug_output_shows_the_remaining_items` (contracts.rs) |
| `libs/engine_collections/src/index_map/iter.rs:101:9: replace <impl fmt::Debug for Keys<'_, K, V>>::fmt -> fmt::Result with Ok(Default::default())` | test gap, fixed | `iterator_debug_output_shows_the_remaining_items` (contracts.rs) |
| `libs/engine_collections/src/index_map/iter.rs:129:9: replace <impl fmt::Debug for IterMut<'_, K, V>>::fmt -> fmt::Result with Ok(Default::default())` | test gap, fixed | `owning_and_mutable_iterators_and_entries_have_nonempty_debug_output` (contracts.rs) |
| `libs/engine_collections/src/index_map/iter.rs:162:9: replace <impl fmt::Debug for Values<'_, K, V>>::fmt -> fmt::Result with Ok(Default::default())` | test gap, fixed | `iterator_debug_output_shows_the_remaining_items` (contracts.rs) |
| `libs/engine_collections/src/index_map/iter.rs:190:9: replace <impl fmt::Debug for ValuesMut<'_, K, V>>::fmt -> fmt::Result with Ok(Default::default())` | test gap, fixed | `owning_and_mutable_iterators_and_entries_have_nonempty_debug_output` (contracts.rs) |
| `libs/engine_collections/src/index_map/iter.rs:215:9: replace <impl fmt::Debug for IntoIter<K, V>>::fmt -> fmt::Result with Ok(Default::default())` | test gap, fixed | `owning_and_mutable_iterators_and_entries_have_nonempty_debug_output` (contracts.rs) |
| `libs/engine_collections/src/index_map/iter.rs:240:9: replace <impl fmt::Debug for IntoKeys<K, V>>::fmt -> fmt::Result with Ok(Default::default())` | test gap, fixed | `owning_and_mutable_iterators_and_entries_have_nonempty_debug_output` (contracts.rs) |
| `libs/engine_collections/src/index_map/iter.rs:265:9: replace <impl fmt::Debug for IntoValues<K, V>>::fmt -> fmt::Result with Ok(Default::default())` | test gap, fixed | `owning_and_mutable_iterators_and_entries_have_nonempty_debug_output` (contracts.rs) |
| `libs/engine_collections/src/index_set.rs:73:9: replace IndexSet<T>::capacity -> usize with 0` | test gap, fixed | `set_capacity_follows_reservations` (contracts.rs) |
| `libs/engine_collections/src/index_set.rs:73:9: replace IndexSet<T>::capacity -> usize with 1` | test gap, fixed | `set_capacity_follows_reservations` (contracts.rs) |
| `libs/engine_collections/src/index_set.rs:84:9: replace IndexSet<T>::reserve with ()` | test gap, fixed | `set_capacity_follows_reservations` (contracts.rs) |
| `libs/engine_collections/src/index_set.rs:98:9: replace IndexSet<T>::try_reserve -> Result<(), TryReserveError> with Ok(())` | test gap, fixed | `set_try_reserve_overflow_is_an_error_and_leaves_the_set_usable` (contracts.rs) |
| `libs/engine_collections/src/index_set.rs:105:9: replace IndexSet<T>::shrink_to_fit with ()` | test gap, fixed | `shrink_to_fit_releases_unused_capacity` (contracts.rs) |
| `libs/engine_collections/src/index_set.rs:267:9: replace <impl Hash for IndexSet<T>>::hash with ()` | test gap, fixed | `set_hash_depends_on_length_and_contents` (contracts.rs) |
| `libs/engine_collections/src/index_set/iter.rs:32:9: replace <impl Iterator for Iter<'a, T>>::size_hint -> (usize, Option<usize>) with (0, None)` | test gap, fixed | `set_iterators_report_exact_sizes_as_they_advance` (contracts.rs) |
| `libs/engine_collections/src/index_set/iter.rs:32:9: replace <impl Iterator for Iter<'a, T>>::size_hint -> (usize, Option<usize>) with (0, Some(0))` | test gap, fixed | `set_iterators_report_exact_sizes_as_they_advance` (contracts.rs) |
| `libs/engine_collections/src/index_set/iter.rs:32:9: replace <impl Iterator for Iter<'a, T>>::size_hint -> (usize, Option<usize>) with (0, Some(1))` | test gap, fixed | `set_iterators_report_exact_sizes_as_they_advance` (contracts.rs) |
| `libs/engine_collections/src/index_set/iter.rs:32:9: replace <impl Iterator for Iter<'a, T>>::size_hint -> (usize, Option<usize>) with (1, None)` | test gap, fixed | `set_iterators_report_exact_sizes_as_they_advance` (contracts.rs) |
| `libs/engine_collections/src/index_set/iter.rs:32:9: replace <impl Iterator for Iter<'a, T>>::size_hint -> (usize, Option<usize>) with (1, Some(0))` | test gap, fixed | `set_iterators_report_exact_sizes_as_they_advance` (contracts.rs) |
| `libs/engine_collections/src/index_set/iter.rs:32:9: replace <impl Iterator for Iter<'a, T>>::size_hint -> (usize, Option<usize>) with (1, Some(1))` | test gap, fixed | `set_iterators_report_exact_sizes_as_they_advance` (contracts.rs) |
| `libs/engine_collections/src/index_set/iter.rs:60:9: replace <impl fmt::Debug for Iter<'_, T>>::fmt -> fmt::Result with Ok(Default::default())` | test gap, fixed | `iterator_debug_output_shows_the_remaining_items` (contracts.rs) |
| `libs/engine_collections/src/index_set/iter.rs:85:9: replace <impl Iterator for IntoIter<T>>::next -> Option<T> with None` | test gap, fixed | `set_iterators_report_exact_sizes_as_they_advance` (contracts.rs) |
| `libs/engine_collections/src/index_set/iter.rs:89:9: replace <impl Iterator for IntoIter<T>>::size_hint -> (usize, Option<usize>) with (0, None)` | test gap, fixed | `set_iterators_report_exact_sizes_as_they_advance` (contracts.rs) |
| `libs/engine_collections/src/index_set/iter.rs:89:9: replace <impl Iterator for IntoIter<T>>::size_hint -> (usize, Option<usize>) with (0, Some(0))` | test gap, fixed | `set_iterators_report_exact_sizes_as_they_advance` (contracts.rs) |
| `libs/engine_collections/src/index_set/iter.rs:89:9: replace <impl Iterator for IntoIter<T>>::size_hint -> (usize, Option<usize>) with (0, Some(1))` | test gap, fixed | `set_iterators_report_exact_sizes_as_they_advance` (contracts.rs) |
| `libs/engine_collections/src/index_set/iter.rs:89:9: replace <impl Iterator for IntoIter<T>>::size_hint -> (usize, Option<usize>) with (1, None)` | test gap, fixed | `set_iterators_report_exact_sizes_as_they_advance` (contracts.rs) |
| `libs/engine_collections/src/index_set/iter.rs:89:9: replace <impl Iterator for IntoIter<T>>::size_hint -> (usize, Option<usize>) with (1, Some(0))` | test gap, fixed | `set_iterators_report_exact_sizes_as_they_advance` (contracts.rs) |
| `libs/engine_collections/src/index_set/iter.rs:89:9: replace <impl Iterator for IntoIter<T>>::size_hint -> (usize, Option<usize>) with (1, Some(1))` | test gap, fixed | `set_iterators_report_exact_sizes_as_they_advance` (contracts.rs) |
| `libs/engine_collections/src/index_set/iter.rs:101:9: replace <impl ExactSizeIterator for IntoIter<T>>::len -> usize with 0` | test gap, fixed | `set_iterators_report_exact_sizes_as_they_advance` (contracts.rs) |
| `libs/engine_collections/src/index_set/iter.rs:101:9: replace <impl ExactSizeIterator for IntoIter<T>>::len -> usize with 1` | test gap, fixed | `set_iterators_report_exact_sizes_as_they_advance` (contracts.rs) |
| `libs/engine_collections/src/index_set/iter.rs:109:9: replace <impl fmt::Debug for IntoIter<T>>::fmt -> fmt::Result with Ok(Default::default())` | test gap, fixed | `owning_and_mutable_iterators_and_entries_have_nonempty_debug_output` (contracts.rs) |
