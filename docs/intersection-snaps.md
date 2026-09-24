# Intersection object snap

`Int` in the Snap modes menu enables persistent intersection capture. At a
point prompt, enter `Int` or `Intersection` for one pick. It is initially off.
The mode finds crossings of straight line and polyline segments, degree-one
NURBS spans, straight surface and B-rep edges, mesh face-boundary wires, and
circles, circular arcs, and ellipses against straight wires and each other. Mesh wires
require `SnapToMeshes Enable`; ordinary curves do not. It works in parallel
and perspective viewports and leaves the source geometry unchanged.

This is a screen-space snap: lines at different depths can cross in the view.
Segments of one polyline can cross each other, and its ordinary corners also
produce Int targets, matching Rhino's measured behavior.
Wires within one mesh do not generate Int targets at their shared vertex.
The source whose projected wire is closer to the cursor supplies the 3D point;
projected wire distances are rounded to pixels for ownership, then frontmost
depth resolves measured ties. A circle or transverse arc wins a rounded tie
against a line; the line wins against an ellipse or tangent arc.
Conic pairs use the exact cursor distance for ownership; at a tangent seam,
the seam's curve supplies the point. Coincident circles expose quadrant
targets from either circle's frame. Projected segment interpolation returns a
point on the original 3D locus.
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
The [three single-polyline picks](../tools/rhino_oracle/fixtures/intersection_self_snaps.json)
and [results](../tools/rhino_oracle/observations/intersection_self_snaps.json)
cover a planar self-crossing, an adjacent corner, and an apparent crossing at
two depths. [Mesh corner picks](../tools/rhino_oracle/fixtures/intersection_mesh_self_snaps.json)
and [results](../tools/rhino_oracle/observations/intersection_mesh_self_snaps.json)
show two misses with the mesh switch off and on. The [Rhino object snap reference](https://docs.mcneel.com/rhino/8/help/en-us/user_interface/object_snaps.htm)
describes Int for curves, edges and mesh wires.

The [circle-line inputs](../tools/rhino_oracle/fixtures/intersection_circle_line_snaps.json)
and [observations](../tools/rhino_oracle/observations/intersection_circle_line_snaps.json)
include transverse intersections, a tangent, an apparent crossing, and a
perspective pick. [Perspective ownership inputs](../tools/rhino_oracle/fixtures/intersection_circle_line_detail_snaps.json)
and [observations](../tools/rhino_oracle/observations/intersection_circle_line_detail_snaps.json)
add seven cursor/source-order cases, including perspective tangency. Affine
views solve conic-line roots analytically. Projective views fit
and validate the rational trigonometric line equation; partially clipped
conics use visible sign brackets.

[Arc and ellipse inputs](../tools/rhino_oracle/fixtures/intersection_arc_ellipse_snaps.json)
and [observations](../tools/rhino_oracle/observations/intersection_arc_ellipse_snaps.json)
cover finite arc sweeps, tangent and endpoint contacts, and perspective and
apparent ellipse crossings. [Reverse source-order inputs](../tools/rhino_oracle/fixtures/intersection_arc_ellipse_priority_snaps.json)
with [results](../tools/rhino_oracle/observations/intersection_arc_ellipse_priority_snaps.json)
confirm ownership at six competing picks.

[Circle pair inputs](../tools/rhino_oracle/fixtures/intersection_circle_circle_snaps.json)
and [observations](../tools/rhino_oracle/observations/intersection_circle_circle_snaps.json)
cover transverse, tangent, apparent, disjoint, perspective, and coincident
circles. Further [coincident picks](../tools/rhino_oracle/fixtures/intersection_circle_circle_detail_snaps.json),
[rotated frames](../tools/rhino_oracle/fixtures/intersection_circle_circle_overlap_snaps.json),
[seams](../tools/rhino_oracle/fixtures/intersection_circle_circle_seams_snaps.json), and
[rotated quadrants](../tools/rhino_oracle/fixtures/intersection_circle_circle_quadrants_snaps.json)
record discrete overlap targets and their source attribution.
[Other conic pair inputs](../tools/rhino_oracle/fixtures/intersection_conic_pairs_snaps.json)
and [observations](../tools/rhino_oracle/observations/intersection_conic_pairs_snaps.json)
cover circle-ellipse, circle-arc, ellipse-ellipse, and tangencies. Projected
conics are fitted in normalized screen coordinates and validated against
interleaved samples. Roots on the first exact locus are isolated with sign
brackets and stationary points; candidates are checked against the second
locus. All 94 retained picks replay with matching kind and source and points
within `1e-9` model units.

[Near-tangent inputs](../tools/rhino_oracle/fixtures/intersection_near_tangent_snaps.json)
with [Rhino picks](../tools/rhino_oracle/observations/intersection_near_tangent_snaps.json),
[cursor sweeps](../tools/rhino_oracle/fixtures/intersection_near_tangent_cursor_snaps.json),
[rotated frames](../tools/rhino_oracle/fixtures/intersection_near_tangent_frames_snaps.json),
and [vertical pairs](../tools/rhino_oracle/fixtures/intersection_near_tangent_vertical_snaps.json)
add 22 retained picks. Two close circle crossings can fall within one root
sampling interval, so a stationary point also divides that interval into two
root brackets. Rhino assigns both close targets to the screen-left circle in
horizontal pairs and the lower circle in vertical pairs; both have the
lexicographically smaller center. All 22 picks match in kind and
source; model points agree within `5e-8`. Rhino's observed points differ from
the exact circle intersection by about `2.4e-8` in the rotated-frame case.

Other curved loci, curved self-intersections, surface isocurves,
occlusion, and multi-object intersection priority need further work. Candidate
mesh wires use the existing snapshot-cached bounds hierarchy; the remaining
near-cursor segment pairs are examined for crossings. Worst-case pair counts
can still grow quadratically where many projected wires or conics overlap.
