# SplitEdge

[Command reference](README.md) · [MergeEdge](merge-edge.md) · [Rhino reference](https://docs.mcneel.com/rhino/8/help/en-us/commands/splitedge.htm)

Enter `SplitEdge`, pick a surface or polysurface edge, then pick locations along
that edge. Overlapping components use the shared numbered ambiguity menu; hover
a number to highlight it. Whole-object preselection is cleared. Mesh wires and
surface isocurves are not B-rep edge components.

The point cursor follows the selected original edge in screen space, including
edges away from the construction plane. Osnap captures its endpoints. Typed
coordinates use the usual world/CPlane/relative point parser and are constrained
to the edge by closest-point search. Collected locations are marked with circles.
The source geometry stays unchanged until finishing. Transparent CPlane edits
and camera/display controls preserve the collected points.

Enter, the Done button, Escape, and `Cancel` finish and apply the collected batch.
In particular, **Escape does not discard already collected split points**.
An empty batch changes no geometry and creates no history. Repeated interior
locations fail the entire batch atomically. Endpoint locations do not split an
edge, but a nonempty endpoint-only batch still replaces the object and records
one Undo, even though the geometry is unchanged.

All successful batches create one Undo step; IDs, attributes, layers and groups
are preserved. Undo/Redo do not restore transient selection. Changed source
geometry or modeling tolerance, deletion, hiding or locking invalidate a pending
batch. No document transaction stays open while collecting points.

Explicit native scripts use:

```text
SplitEdge object-id edge-index parameter [parameter ...]
```

Edge indices are zero-based; parameters use the original edge's native domain.
This syntax is not Rhino's mouse macro syntax. Exact endpoint parameters are
allowed; out-of-domain/nonfinite input rejects the whole command.

## Geometry and evidence

The kernel splits the spatial edge and every incident UV trim, including both
sides of a seam, without refitting surfaces. It retains the first segment's edge
slot and appends vertices/segments in descending parameter order. Geometric
correspondence and final topology are checked. The batch is bounded to 100,000
input locations and the kernel's existing subdivision work budget. Interior
splits are not collapsed merely because their spacing is below model tolerance.

The [21 retained Rhino 8 observations](../split-edge-provenance.json) cover box,
curved-surface and kinked-face sources; reversed entry order; duplicates;
endpoints; tiny/sub-tolerance intervals; preselection; Enter/Escape; and actual
Undo/Redo. [Requests](../../tools/rhino_oracle/fixtures/split_edge_command.json)
and [complete responses](../../tools/rhino_oracle/observations/split_edge_command.json)
retain geometry, ordered spatial/UV definitions, topology, attributes and history.
Native replay matches all 21 cases at absolute epsilon `1e-9` and relative
epsilon `1e-10`, without component permutations, fitted geometry or discarded
numeric fields. This is fixture agreement, not arbitrary-input parity.

Verification checkpoint: 2,967 release-mode workspace tests, 251 Python tests,
seven offscreen GPU tests, formatting, and Clippy/Rustdoc with warnings denied.
The new UI tests exercise real pointer press/release events, all four camera
projections, endpoint capture, typed points, nested CPlane input, and stale picks.

## Remaining limits

Component selection uses the existing sampled display boundaries. Point location
uses a bounded screen-distance search: 16 intervals per span, the eight best
sampled neighborhoods, up to 48 refinement iterations, and at most 4,096 spans.
Projection retains double precision until drawing. Neither screen search nor
typed closest-point search certifies a global minimum for arbitrary rational
curves. Camera-plane crossings, general occlusion behavior and pathological
high-zoom/multimodal curves need further coverage.

Rhino's distance constraint from a prior object-snap location and other-object
snaps within this constrained point prompt are not implemented yet. These are
explicit remaining command features, not established by the retained fixtures.
