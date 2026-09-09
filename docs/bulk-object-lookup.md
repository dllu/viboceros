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
reverse group indexes. Grouped copies retain destination indices from insertion and use
the same checked transition without repeated object searches. History replay
still resolves individual object IDs; this is not a global object index.

Tests cover all subsets of eight scattered objects, reversed and duplicate IDs,
missing-ID precedence, unchanged state on failures across all seven consumers,
redo preservation, and exact mixed grouped/ungrouped copy Undo/Redo replay.
Late geometry-overflow coverage also stages valid sources before a failing
source in both in-place transforms and multi-instance copies. It compares the
complete document's debug representation before and after failure, including
overlapping groups, selection, redo history, and a caller-owned transaction
with an earlier pending edit. Both standalone and caller-transaction cases
must remain unchanged.

The private `object_geometry` module shares geometry-only staging and commit
between in-place transforms, morphs, morph copies, and replacements. All requested objects
are checked for editability before invoking a transform or morph. Staging
holds only new geometry; unchanged geometry is discarded before opening a
transaction. In-place commit moves the old object into history and clones the new
geometry once for the independent live and redo states, avoiding temporary
clones of the old mesh or B-rep. Morphs reuse the resolved indices rather than
looking up the same source set again through the replacement API. In-place object
identity, attributes, isolation, memberships, and selection are retained.
Morph copies use the same preflight before any user-supplied callback runs,
then pass staged geometry to the separate group-aware copy commit. Tests check
callback order and duplicate coalescing, late callback overflow without
document mutation, and grouped copy history in standalone and caller-owned
transactions.

Replacement-geometry copies into existing source groups retain source and
destination indices through insertion and membership assignment. They move
the supplied geometry out of the deduplicated input map without cloning it,
and skip empty membership updates. Source membership validity is checked
before insertion, including duplicate memberships, missing definitions, and
inconsistent reverse indexes. Regression tests cover unchanged document and
caller-transaction state on these failures, both table-order and caller-order
copies, last-value/last-position duplicate handling, and exact undo/redo.
History replay still uses its existing object lookup strategy.

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
Layer copies also preflight memberships of the sources that will actually be
copied and reserve object capacity before opening a transaction. Corrupt
memberships on a later source must leave the document and any caller-owned
transaction unchanged. Same-layer sources remain skipped after editability
validation. Layer moves stage only indices and record geometry-free property
history, leaving the original geometry allocation untouched during edit and replay.

The 20,000-point debug diagnostic measured layer reassignment at about 1.13 s
before consolidation and 83 ms afterward; copy-to-layer went from 1.18 s to
139 ms. All resulting geometries and layer assignments are checked. These
timings exclude GUI work and are not comparisons against Rhino.

```sh
cargo test -p viboceros-document benchmark_large_layer_transfers -- --ignored --nocapture
```

## Grouped copies

Affine arrays and morph copies validate source memberships before inserting
objects or definitions when groups are preserved. This also applies to the
definitions-only policy. Required group capacity is counted from unique source
memberships rather than scanning every group's reverse member index. The omit
policy does not inspect source groups or build a source-to-copy group map.
Failure tests exercise missing definitions, duplicate memberships, and missing
reverse membership entries across these group-preserving copy paths, including
caller transactions with earlier edits and redo history.

Copy membership assignment receives validated source/destination index pairs
directly from affine/morph and layer-copy insertion. It does not scan the
growing object table or resolve destination IDs for each array instance.
New group definitions still follow source
table order and each source's ordered memberships. Creating those definitions
does not reorder objects, so the indices remain valid. Group-list and reverse
member-index validation is shared with ordinary edits and history replay; only
object resolution differs. Group-table searches and per-object history records
remain, so this does not remove every large-group scaling cost.

Automatic naming uses an ephemeral allocator shared across a copy operation's
instances. It captures live names once, lazily on the first required definition,
and advances through unused `GroupNN` candidates without restarting the search.
The allocator is not persistent document state and is discarded after the copy;
unrelated group edits cannot leave a stale cache. Tests compare 128 successive
allocations against an independent first-unused-name search with numbering gaps,
case variants, alternate zero padding, and unnamed groups. Individual group
insertion still validates name uniqueness through the normal document API.

Corruption tests check mismatched prior memberships, duplicate memberships,
missing group definitions, and inconsistent reverse indexes, requiring complete
state preservation on failure. Existing copy tests cover membership order across
copy modes and undo/redo. Sparse-array coverage uses reversed and duplicate
requests among unrelated objects and checks independent group definitions and
exact history replay across three instances. The debug diagnostic copying 20,000 points in one group
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
that follows both operations. This does not optimize history replay or establish
release-build or Rhino performance parity.

```sh
cargo test -p viboceros-document benchmark_large_group_creation -- --ignored --nocapture
```

The same diagnostic also removes the first group while retaining the second.
Group deletion resolves member indices once and checks that object memberships
agree with the group's reverse index before beginning its transaction. Missing
member objects return an error instead of panicking; an incomplete reverse index
is rejected instead of leaving dangling memberships after definition removal.
Corruption tests cover these cases with and without a caller-owned transaction,
including unchanged redo and document state. Empty-definition removal and
overlapping ordered memberships have Undo/Redo coverage.

Deleting the group of 20,000 points improved from 2.21 s to 161 ms in the debug
diagnostic, including the preflight consistency check. Surviving membership
order is retained; history replay still uses per-object ID resolution.
