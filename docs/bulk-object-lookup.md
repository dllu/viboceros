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
reverse group indexes. Large grouped copies and history replay can still incur
repeated object lookups; this is not a global object-index implementation.

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
