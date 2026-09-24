# Intersection object snap

`Int` in the Snap modes menu enables persistent intersection capture. At a
point prompt, enter `Int` or `Intersection` for one pick. It is initially off.
The mode finds crossings of straight line and polyline segments, degree-one
NURBS spans, straight surface and B-rep edges, and mesh face-boundary wires. Mesh wires
require `SnapToMeshes Enable`; ordinary curves do not. It works in parallel
and perspective viewports and leaves the source geometry unchanged.

This is a screen-space snap: lines at different depths can cross in the view.
The source whose projected line is closer to the cursor supplies the 3D point;
frontmost depth and then curve-over-mesh priority resolve measured ties.
Projected segment interpolation returns a point on the original 3D locus.
Collinear overlaps do not produce an Int target in the measured interior pick.
The square snap aperture
applies to the crossing, not to the line endpoints.

The owned Rhino 8 [base fixture](../tools/rhino_oracle/fixtures/intersection_snaps.json)
and [observations](../tools/rhino_oracle/observations/intersection_snaps.json)
contain eight real point picks: one-source, transverse, endpoint, overlap,
apparent-height and mesh-switch cases. [Depth/source-order inputs](../tools/rhino_oracle/fixtures/intersection_depth_snaps.json)
with [results](../tools/rhino_oracle/observations/intersection_depth_snaps.json)
add three cases. [Perspective crossings](../tools/rhino_oracle/fixtures/intersection_perspective_snaps.json)
and [reverse source order](../tools/rhino_oracle/fixtures/intersection_perspective_reverse_snaps.json)
have their own [results](../tools/rhino_oracle/observations/intersection_perspective_snaps.json)
and [reverse results](../tools/rhino_oracle/observations/intersection_perspective_reverse_snaps.json).
The [four mixed-mode picks](../tools/rhino_oracle/fixtures/intersection_mixed_snaps.json)
and [Rhino results](../tools/rhino_oracle/observations/intersection_mixed_snaps.json)
show Int winning over Near at a crossing and Mid at the same location.
[Competing End and Near picks](../tools/rhino_oracle/fixtures/intersection_competing_snaps.json)
with [results](../tools/rhino_oracle/observations/intersection_competing_snaps.json),
and [competing Mid picks](../tools/rhino_oracle/fixtures/intersection_competing_mid_snaps.json)
with [results](../tools/rhino_oracle/observations/intersection_competing_mid_snaps.json)
show that a closer unrelated End or Mid can win, while Near remains a fallback.
All 21 native replays match snap kind, source and 3D point within `1e-9` model
units. The [Rhino object snap reference](https://docs.mcneel.com/rhino/8/help/en-us/user_interface/object_snaps.htm)
describes Int for curves, edges and mesh wires.

Curved loci, self-intersections within one object, surface isocurves,
occlusion, and multi-object intersection priority need further work. Candidate
mesh wires use the existing snapshot-cached bounds hierarchy; the remaining
near-cursor segment pairs are examined for crossings. Worst-case pair counts
can still grow quadratically where many projected wires overlap.
