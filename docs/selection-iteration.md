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

Tests check both sides of the crossover, reversed and sparse pick order, removal
from selection, group-expanded hidden members, deletion/Undo replay, read-only
behavior, iterator exhaustion, and lazy first-object access.

A 20,000-object full-selection traversal took about 1.04 s before the change and
35 ms afterward in the native test build. This is a local diagnostic measurement,
not an end-to-end command speedup or a performance comparison against Rhino.

```sh
cargo test -p viboceros-document benchmark_ordered_selection_iteration -- --ignored --nocapture
```
