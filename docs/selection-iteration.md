# Ordered selection iteration

[Architecture](architecture.md) · [Batch deletion](batch-deletion.md)

`Document::selected_objects` returns borrowed objects in selection action order,
not object-table or UUID order. Group-expanded hidden/locked peers remain part
of that selected set; the iterator does not apply a new visibility filter.

The independent `selection/objects` module keeps the first object lookup lazy.
If traversal continues with at least 16 IDs remaining, it builds a temporary
ordered ID-to-object lookup by scanning the object table once. Smaller selections
use direct lookups without allocating. The crossover is a heuristic, not an API
limit or a universally optimal threshold.

For a document of N objects and a selection of K objects, the indexed path uses
O(K) temporary storage and O(N log K + K log K) work instead of K full object-table
searches. It stores references, not cloned geometry, and changes no document,
selection, or history state. Each iterator owns its lookup; there is no persistent
cache to invalidate. PointCloud command-first creation uses this shared path.
Mesh-face source and extraction staging also use it, reusing each borrowed
object's geometry, attributes, and memberships instead of resolving the ID twice.
Face-extraction source records borrow all three until owned output plans have
been built. Tests check pointer identity for the mesh, attributes, and ordered
membership slice, as well as value preservation and failure behavior.
A 20-mesh regression crosses the indexed-iterator threshold in non-table action
order, includes a hidden group-selected peer, and checks that a late extraction
callback failure leaves the document unchanged.
`SplitDisjointMesh` also stages from borrowed selected objects. It moves all
components into fresh objects, preserving restricted sources and batch-deleting
ordinary split sources after assigning output groups. All pieces use one
`copy_object_pieces_into_source_groups` batch: repeated source IDs retain every
piece, while source editability and group validity are checked before insertion.
The document regression checks 128 interleaved pieces, locked-layer inheritance,
history replay, and atomic late missing/locked/invalid-membership failures in
standalone and caller-owned transactions. Command tests cover mixed
connected/disconnected inputs and three-piece source-face ordering through
undo/redo.
`CollapseMeshEdge` consumes staged results into owned replacements and one
batch deletion for empty meshes, without cloning surviving results a second
time. Mixed-outcome coverage checks unrelated object order, retained groups
and selection, and exact undo/redo replay.
The native explicit-deletion policy permits hidden/locked IDs. A collapse
regression covers locked, group-selected peers whose empty results are removed,
then restores their lock state, geometry, groups, and selection through Undo.
It also checks that a successful collapse replaces an existing redo branch.
This command-level history check is not a separate live Rhino parity probe.
`Distribute` carries borrowed objects from this iterator into rigid-unit
grouping, local bounds, and transform staging, avoiding later per-ID scans.
Units follow the first selected member's action order and each object's top
membership; unselected group members are not included. A regression checks
overlapping groups, partial selection, order, and read-only borrowed identity.

## Building explicit selections

`Document::selectable_objects` is a separate, borrowed object-table-order
iterator. It filters object and layer visibility/locking without expanding
groups or looking up each object's ID again. Window selection, `Group all`, and
named-group filtering use it; duplicate and layer selection likewise reuse
their existing object records. Click picking and selection-prompt `SelAll` also
use the shared iterator. Selection prompts intersect incoming IDs with this traversal as a batch before
applying the selection mode, rather than resolving every incoming ID twice.
Layer lookup remains linear in the layer table; this removes repeated
object-table scans, not every possible selection cost.
Geometry-filtered commands (`SelCrv`, `SelMesh`, `SelPt`, `SelShortCrv`, and
the other geometry filters) also share this traversal. Their fallible geometry
predicates still finish before selection changes, and matching seeds retain
the existing group-expansion behavior. A 20,000-point debug diagnostic improved
from 1.12 seconds to 88 milliseconds and checks the complete selected ID set.
These are focused native CPU timings, not release or Rhino comparisons:

```sh
cargo test -p viboceros-command benchmark_large_geometry_selection -- --ignored --nocapture
```

A native test covers all 16 combinations of object/layer visibility and locking,
read-only traversal, and named-group selection without hidden/locked peers.

The ignored `benchmark_large_scene_click_picking` test exercises 20,000
overlapping points and checks deterministic first-object tie-breaking. On the
development machine, replacing the per-object ID lookup reduced its debug pick
time from 1.05 seconds to 5.1 milliseconds. This is a focused CPU measurement,
not a Rhino comparison or a general frame-rate guarantee. Run it with
`cargo test -p viboceros benchmark_large_scene_click_picking -- --ignored --nocapture`.

`select_objects` and `select_objects_direct` share seed validation. Large
requests scan the object table once, using a set of remaining requested IDs
and a cached set of selectable layers. Small requests retain direct checks.
All IDs are validated before mutation or group expansion. Missing IDs retain
priority over unselectable objects, with the lowest sorted ID reported in each
category. Duplicate seeds are coalesced. Hidden/locked group peers may still be
reached through valid seeds; the validator does not reject the expanded cluster.

Regression tests compare batch validation against independent per-ID checks on
both sides of the small-request threshold. They verify all four selection modes,
direct and group-aware selection, failure atomicity including redo and selection
memories, and group expansion without propagating through peers' other groups.

The same 20,000-object diagnostic separately times direct selection setup:
about 2.09 s before batched validation and 40 ms afterward. This measurement does
not cover every attribute-based selection command.

## Object-table selection filters

Select-all, inversion, name-pattern selection, and display-color selection
check the object already borrowed from the table instead of looking its ID up
again. They share the same selectability predicate as ID-based callers, retaining
object/layer visibility and locking rules. Name selection still does not expand
groups, and display-color selection still excludes grouped objects.

The 20,000-object, single-layer diagnostic measured select-all at about 1.05 s
before this change and 24 ms afterward; wildcard name selection went from about
1.05 s to 44 ms. These are native debug/test-build timings, not Rhino comparisons.
Layer lookups remain, and ID-only selection filters are not covered by this
optimization. Regression coverage checks exact selection order and unchanged
objects, layers, groups, and undo label across the four filters.

## Selection cleanup after edits

Layer visibility/locking, object attribute changes, and copy completion use the
shared batched recorded-object filter to prune selection. It scans the object
table once with a cached set of selectable layers, instead of resolving every
selected ID separately. An empty selection returns immediately. Cleanup retains
the surviving pick order and updates previous-selection memories through the
usual selection updater. It does not expand groups; history replay retains its
separate group-aware policy.

Tests compare the complete document state against independent per-ID filtering
for varied selection sizes/order and hidden/locked objects/layers, including
grouped objects, redo, previous-selection memories, and repeated cleanup. Layer
visibility and locking tests also check transaction rollback restores selection
order and memories. In the debug diagnostic, hiding the layer containing half of
20,000 selected points took about 1.07 s before batching and 55 ms afterward.
This measures the document operation, not GUI redraw or Rhino performance.

## Iteration checks and timing

Tests check both sides of the crossover, reversed and sparse pick order, removal
from selection, group-expanded hidden members, deletion/Undo replay, read-only
behavior, iterator exhaustion, and lazy first-object access.

A 20,000-object full-selection traversal took about 1.04 s before the change and
35 ms afterward in the native test build. This is a local diagnostic measurement,
not an end-to-end command speedup or a performance comparison against Rhino.

```sh
cargo test -p viboceros-document benchmark_ordered_selection_iteration -- --ignored --nocapture
```
