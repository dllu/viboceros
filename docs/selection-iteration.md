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

## Building explicit selections

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
