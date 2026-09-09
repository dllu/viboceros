# Bulk object lookup

[Architecture](architecture.md) · [Selection](selection-iteration.md)

The document's private `object_lookup` module resolves requested IDs to temporary
object-table indices. It coalesces duplicates, visits objects in document order,
and reports the lowest missing ID before callers check editability or transform
geometry. Empty requests return immediately. The resolver neither clones
geometry nor changes selection or history, and creates no persistent cache.
Callers must not remove or reorder objects while using the indices.

For N document objects and K unique requested IDs, resolution uses O(K) storage
and O(K log K + N log K) work, stopping once all IDs are found. Affine transform,
affine copy/arrays, morph and morph-copy, geometry replacement and geometry-copy,
editable attribute changes, and layer transfers share it. Each caller still stages all changes
before mutation; caller-specific ordering and group policies remain separate.

Fresh ungrouped copies also skip empty membership transitions. Grouped copies
still use the membership transition path, preserving ordered memberships and
reverse group indexes. Grouped copies resolve destination indices once and use
the same checked transition without repeated object searches. History replay
still resolves individual object IDs; this is not a global object index.

Tests cover all subsets of eight scattered objects, reversed and duplicate IDs,
missing-ID precedence, unchanged state on failures across all seven consumers,
redo preservation, and exact mixed grouped/ungrouped copy Undo/Redo replay.

A native debug-build diagnostic on 20,000 ungrouped points measured transform
at about 1.14 s before these changes and 108 ms afterward; copy improved from
4.39 s to 252 ms. The diagnostic checks every resulting point and output count.
These are document-operation timings, not release, redraw, or Rhino comparisons.

```sh
cargo test -p viboceros-document benchmark_bulk_transform_and_copy -- --ignored --nocapture
```

## Layer transfers

Move-to-layer and copy-to-layer validate all source IDs and editability before
skipping objects already on the destination layer. Destination-layer errors
retain priority over source errors. Validation holds only indices, so skipped
objects do not require geometry clones. Copies append to the table without
invalidating source indices and retain source table order, not request order.

Regression tests cover destination/missing-object/locked-object precedence,
same-layer and empty no-ops preserving redo and complete document state,
duplicate/reversed requests, skipped sources, selection order, and rollback.
Existing tests cover group copies and hidden/locked destinations.

The 20,000-point debug diagnostic measured layer reassignment at about 1.13 s
before consolidation and 83 ms afterward; copy-to-layer went from 1.18 s to
139 ms. All resulting geometries and layer assignments are checked. These
timings exclude GUI work and are not comparisons against Rhino.

```sh
cargo test -p viboceros-document benchmark_large_layer_transfers -- --ignored --nocapture
```

## Grouped copies

Copy membership assignment builds a temporary destination-ID-to-index map for
copies with nonempty memberships. New group definitions still follow source
table order and each source's ordered memberships. Creating those definitions
does not reorder objects, so the indices remain valid. Group-list and reverse
member-index validation is shared with ordinary edits and history replay; only
object resolution differs. Group-table searches and per-object history records
remain, so this does not remove every large-group scaling cost.

Corruption tests check mismatched prior memberships, duplicate memberships,
missing group definitions, and inconsistent reverse indexes, requiring complete
state preservation on failure. Existing copy tests cover membership order across
copy modes and undo/redo. The debug diagnostic copying 20,000 points in one group
improved from 6.57 s to 385 ms and checks every copied point and both membership
directions. As above, these are not Rhino or release-build timings.

```sh
cargo test -p viboceros-document benchmark_large_group_copy -- --ignored --nocapture
```

## Group creation and member addition

Group creation and adding members also resolve all requested object IDs once.
Both use the indexed membership setter within the existing group transaction;
new definitions do not cause a second source-validation pass. Duplicate object
IDs are coalesced. Already-present members remain in their original membership
position, while new memberships append. Validation still rejects missing source
IDs before any edits, preserving redo and caller-owned transactions on failure.

Tests exercise every subset of three objects with different overlapping
membership orders, reversed/duplicate requests, no-ops, and exact Undo/Redo
restoration. A 20,000-point debug diagnostic measured group creation at about
4.29 s before batching and 118 ms afterward, and member addition at 3.35 s before
and 162 ms afterward. Timings exclude the separate exhaustive consistency check
that follows both operations. This does not optimize group deletion or history
replay, nor establish release-build or Rhino performance parity.

```sh
cargo test -p viboceros-document benchmark_large_group_creation -- --ignored --nocapture
```
