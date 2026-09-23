# Offset

[Command reference](README.md) · [Project overview](../../README.md)

Select one or more lines, circles, circular arcs, ellipses, planar NURBS curves,
planar polylines, or supported polycurves, then enter
`Offset distance side-point`. The point chooses the side of each curve; the
distance must be positive. `Offset distance BothSides=Yes` creates one curve on
each side without a point. The selected originals remain in the document, and
the new curves become selected.

`Offset ThroughPoint=x,y,z` derives a distance independently for every selected
curve. The picked point must lie in each curve's offset plane and on the
resulting finite curve within document tolerance. Polyline offsets try the
nearest source segment; round and chamfer corners also solve from the nearest
vertex when the point lies on a corner join.
If no supported offset passes through the point, the command leaves every
selected source unchanged. `BothSides` is unavailable with `ThroughPoint`.

Lines offset to the left or right of their direction in the current
construction plane. Circles and arcs offset radially in their own planes.
Ellipses offset along their analytic normals. Noncircular ellipse offsets are
closed cubic NURBS fitted adaptively and checked against the analytic locus at
interior stations of every span. Inward distances reaching the first cusp are
rejected. The fit uses the document absolute tolerance and an 8,192-span cap.
Planar NURBS offsets are likewise fitted with cubic spans from the source
curve's position and derivatives, then checked at interior stations against
the offset locus. Source kinks, stationary points, and offsets that develop a
cusp are rejected. The output retains the source parameter interval.
Polycurves made entirely of lines and polylines use the same corner rules as
polylines and retain their outer parameter interval. Smooth mixed polycurves
are converted to one NURBS curve before offset fitting. Curved junctions with
sharp kinks remain unsupported.
Planar polylines use `Corner=Sharp` by default, extending neighboring offset
segments to their intersection. `Corner=Chamfer` bridges convex gaps with a
straight segment; concave corners still meet at the segment intersection.
`Corner=Round` fills each convex gap with a tangent circular arc and returns a
polycurve when any arcs are needed.
`Corner=None` leaves convex gaps open, creating separate curves where needed.
Concave corners still meet at the offset segment intersection.
Sharp results retain one parameter per source vertex. Chamfer results retain
the source domain and assign new parameters when extra vertices are added.
Round results retain the source domain across their line and arc segments.
Disconnected `None` results receive separate natural parameter domains.
Collinear polylines use the construction
plane; other planar polylines use their own plane, aligned with the
construction plane normal when possible. A side point chooses the nearest
polyline segment's side. Results can self-intersect; automatic trimming
remains to be implemented. Reversed or degenerate output
segments are rejected.

Offsets retain their source parameter intervals. An inward
offset that would collapse a circle or arc is rejected. A side point on the
supporting line or circle is ambiguous and rejected.

`OffsetMultiple distance side-point OffsetCount=n` creates `n` offsets per
selected curve at successive multiples of the distance (default `n=2`).
Open curves offset toward the picked side. Closed curves use a shared inward
or outward choice: a point inside any selected closed region chooses inward.
Each nested island reverses that direction, including successive nesting
levels. Closed circles, full circular arcs, ellipses, simple planar polylines,
certified simple planar NURBS curves, and their supported polycurve equivalents
can form regions. Selected closed
boundaries that intersect or touch are rejected. Self-intersecting closed
polylines and NURBS curves are rejected as ambiguous regions. NURBS region
validation has an 8,192-piece resource cap.
The command supports the same `Corner` and `OutputLayer` options as `Offset`;
at most 100,000 source/count combinations can be requested. All outputs are
staged before the document changes.

`OutputLayer=Current` is the default; `OutputLayer=Input` uses each source's
layer. Both choices create fresh object attributes. The command stages every
result before changing the document, so an unsupported or degenerate selected
curve leaves the document unchanged. NURBS and mixed polycurve offsets across
sharp corners, as well as smooth corners,
trim, cap, and construction-plane overrides, remain
to be implemented.

## Ellipse oracle probe

The [ellipse offset fixture](../../tools/rhino_oracle/fixtures/ellipse_offset.json)
compares 65 geometric stations on inward, outward, rotated, and circular
ellipses. The native probe uses the offset's retained angle parameterization;
the Rhino worker finds the nearest output point to each source station, so
different spline knot layouts can still be compared by location. Run:

```sh
tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/ellipse_offset.json \
  --absolute-epsilon 1e-8 --relative-epsilon 1e-9 --timeout 360
```

The native fixture passes. A Rhino observation is not recorded yet.
