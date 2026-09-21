# SplitEdge

[Command reference](README.md) · [MergeEdge](merge-edge.md) · [Rhino reference](https://docs.mcneel.com/rhino/8/help/en-us/commands/splitedge.htm)

Enter `SplitEdge`, pick a surface or polysurface edge, then pick locations along
that edge. Overlapping components use the shared numbered ambiguity menu; hover
a number to highlight it. Whole-object preselection is cleared. Mesh wires and
surface isocurves are not B-rep edge components.

The point cursor follows the selected original edge in screen space, including
edges away from the construction plane; clicks need not be near its outline.
Osnap uses the shared visible-object feature capture (including other objects),
then constrains the snapped model point to the edge. A feature label and connector
distinguish the actual snap target from the constrained location. Typed
coordinates use the usual world/CPlane/relative point parser and are constrained
to the edge by closest-point search. Collected locations are marked with circles.
The source geometry stays unchanged until finishing. Transparent CPlane edits
and camera/display controls preserve the collected points.

After accepting a point, type a number to constrain the next location by **arc
length along the edge**, not straight-line distance. The constraint persists and
its reference advances with each accepted point. Type another number to change
it, or `0` to clear it; negative numbers use their magnitude. Use `0,0,0` or
`w0,0,0` to enter the origin as a point. Typed coordinates also honor an active
constraint. Without a snap, the cursor chooses the nearest projected reachable candidate; a
sole candidate remains selectable even when it is away from the cursor. If no
candidate is reachable, no point is accepted. Closed edges can cross their seam,
but a distance longer than one circuit has no candidate.
With a snap, its model point chooses the nearest reachable candidate in 3D.

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

Another [19 distance-constraint observations](../split-edge-distances.md) cover
typed and real mouse points, persistent/reset/negative/oversized distances,
curved arc length and closed-edge wrapping. Their raw curved positions have
measurable Rhino inversion residuals. These additional curved records use an
explicit absolute comparison bound of `1e-6`; the original 21 fixtures and the
new straight-edge records retain `1e-9`. Every numeric field is still compared.

Another [17 snap observations](../split-edge-snaps.md) cover Point, End, Mid,
Cen and Quad, off-edge projection, curved edges and distance-candidate choice.
Fifteen declared model-space snap locations replay at absolute epsilon `1e-9`
and relative epsilon `1e-10`. Two raw NoSnap screen controls remain explicitly
unsupported in native replay because their camera calibration was not recorded.
Nonuniform NURBS and B-rep Mid features use half arc length, not parameter midpoints;
the shared viewport cache retains these expensive features across redraws.

Verification checkpoint: 2,990 release-mode workspace tests, 263 Python tests,
seven offscreen GPU tests, formatting, and Clippy/Rustdoc with warnings denied.
The new UI tests exercise real pointer press/release events, all four camera
projections, endpoint capture, typed points, nested CPlane input, stale picks,
cached distance-constrained pointer input, off-edge feature capture and cache
invalidation. See the snap audit for replay and performance boundaries.

## Remaining limits

Component selection uses the existing sampled display boundaries. Point location
uses a bounded screen-distance search: 16 intervals per span, the eight best
sampled neighborhoods, up to 48 refinement iterations, and at most 4,096 spans.
Projection retains double precision until drawing. Neither screen search nor
typed closest-point search certifies a global minimum for arbitrary rational
curves. Camera-plane crossings, general occlusion behavior and pathological
high-zoom/multimodal curves need further coverage.

Shared snapping does not yet implement every Rhino feature/type or one-shot mode.
A distance currently requires an accepted point on this edge; distance entry
before that point, use of a prior command's last point, and ambiguous equal-distance
candidate choices have not been measured against Rhino. Numerical integration
and native parameter resolution limit very short or extremely scaled offsets.
