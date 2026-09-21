# SplitEdge object snaps

[Command](commands/split-edge.md) · [Provenance and hashes](split-edge-snaps-provenance.json)

See the later [polycurve/surface audit](composite-feature-snaps.md) for per-segment
Mid, corrected surface boundary features and retained Center-capture discrepancies.

The [17 requests](../tools/rhino_oracle/fixtures/split_edge_snaps_command.json)
and [raw result values](../tools/rhino_oracle/observations/split_edge_snaps_command.json)
were measured with public Rhino 8.32 commands in owned private Xvfb sessions.
All include real component selection, bounded mouse picks and actual Undo/Redo.
External snap targets remain unchanged. No proprietary implementation was inspected.

## Measured behavior

Point, End, Mid, Cen and Quad can supply locations away from the selected edge.
Rhino constrains the snapped model point to the edge in 3D, not by screen distance.
The constrained point becomes the next arc-distance anchor. When distance is
active, the snapped model point chooses between reachable candidates in 3D,
even when its screen position prefers the other candidate.

Five-pixel offset clicks still reach exact snap locations. Two NoSnap controls
produce different positions, demonstrating real feature capture rather than
typed locations: the on-edge aim at x=2 yields x=2.318772657907932 without a snap;
the off-edge aim (2,-2,0) yields x=1.1224577718024389. Once selected, the edge
constrains clicks away from its screen outline too; a proximity cutoff belongs
to component selection, not the subsequent point prompt.

Two nonuniform quadratic witnesses distinguish Mid policies. A straight NURBS
with control x coordinates [2,3,8] has parameter midpoint x=4 but half-arc-length
point x=5. A B-rep edge with controls [0,2,10] has parameter midpoint x=3.5 and
half-arc-length point x=5. Rhino snaps at x=5 in both cases. Native shared snaps
now supply NURBS Mid and correct the former B-rep parameter-midpoint behavior.

## Replay boundaries

Native replay compares all ordered geometry, topology, attributes, selection and
before/after/Undo/Redo states for 15 declared snap locations at absolute epsilon
`1e-9` and relative epsilon `1e-10`. Rhino-only event/history transcripts remain
in the archive. No geometry fields are dropped or components reordered.

This adapter replays declared model-space snap targets, not pixel hit testing.
The two NoSnap controls lack recorded camera calibration and explicitly return
an unsupported-fixture error; they are not counted as successful comparisons.
Production viewport tests separately cover real pointer press/release, all four
views, edge-on CPlanes, off-edge features, locked/hidden targets and cache invalidation.
The original 21 SplitEdge and 19 distance observations are unchanged.

## Implementation and safety

Ordinary drafting and SplitEdge share camera-space feature capture. Parallel
queries retain indexed point-cloud lookup and camera-local precision; perspective
queries keep double precision through projection. A separate edge-snap cache
retains the closest parameter by curve, target point and modeling tolerance.

`ObjectSnapCache` retains NURBS and B-rep edge arc-length midpoints independently
of the camera. Exact source-curve and tolerance comparisons invalidate entries,
including after Undo; deletion/conversion releases entries on the next query.
B-rep entries copy only spatial edge curves, not surfaces or trims. Stationary
queries reuse integrations. Failed integration omits that Mid feature without
removing endpoints. Cold queries still compute all visible eligible midpoints,
and hot queries compare source curves; this is not constant-time scene lookup
or a cross-engine performance claim.

The oracle adds a bounded input form:

```json
{"pick":{"point":[2,-2,0],"osnap":"Point","offset":[5,0]}}
```

Modes are restricted to NoSnap, Point, End, Mid, Cen and Quad; point coordinates
must be finite, pixel offsets are optional integer pairs in [-32,32], and at
most 64 inputs are allowed. Clicks must remain inside the owned viewport and
are gated on the next recorded command pause. The probe snapshots and restores
ModelAid/SmartTrack settings even after failure. An initial Rhino-9-only setting
failed before any command; the retained runs use supported Rhino 8 settings.

The native UI does not yet implement all Rhino snap modes/types or its one-shot
override UI. General occlusion, equal-distance priorities, arbitrary-camera pixel
equivalence and globally certified closest points remain unestablished. See
[Rhino's snap reference](https://docs.mcneel.com/rhino/8/help/en-us/user_interface/object_snaps.htm)
for the broader interface contract, not as evidence of native parity.
