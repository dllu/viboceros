# Batch object deletion

[Architecture](architecture.md) · [PointCloud](commands/point-cloud.md)

`Document::delete_objects` removes an explicit set of object IDs atomically.
Duplicate IDs are coalesced; empty input is a no-op and missing IDs fail before
mutation. Like the single-object API, it does not apply a selection filter or
reject explicit hidden/locked objects. Commands own selection policy.

The independent `object_deletion` module partitions the object table once and
moves removed objects into a single history record. It retains original table
positions and ordered memberships without cloning geometry. Empty group
definitions survive; reverse membership indexes are updated in batches.
Undo merges removed objects with survivors in original document order. Redo
partitions again, avoiding repeated vector insertion/removal. ID membership
checks use ordered sets, so total work is not claimed to be strictly linear.

Selection membership is captured again on each Redo, honoring selection changes
made after Undo. Restored objects retain their relative pick order and unrelated
selection is untouched; this does not promise restoration of interleaving with
unrelated picks. Transaction rollback restores the complete pre-transaction
selection and its memories. Pure deletion retains the existing SelLast policy.

`Delete` and `PointCloud` use this path. Ungrouped selection validation also scans
the object table instead of performing one linear object lookup per restored ID.
PointCloud command-first ordering uses a selection-rank map and a table scan.

## Checks and timing

Tests exercise all 63 nonempty subsets of six grouped objects, duplicate/missing
IDs, empty groups, exact object and membership restoration, repeated replay,
selection changes between Undo and Redo, and batches interleaved with object and
group creation. Existing command and oracle checks cover PointCloud output.

A manual test-build measurement of 10,000 point objects changed from roughly
979/439/140 ms for conversion/Undo/Redo to 98/51/36 ms after batching and selection
validation changes. These are single-machine diagnostic timings, not release
benchmarks or a performance comparison against Rhino. Run the opt-in test with:

```sh
cargo test -p viboceros-command benchmark_large_point_conversion -- --ignored --nocapture
```
