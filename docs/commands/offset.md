# Offset

[Command reference](README.md) · [Project overview](../../README.md)

Select one or more lines, circles, circular arcs, or planar polylines, then enter
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
levels. Closed circles, full circular arcs, and simple planar polylines can
form regions. Selected closed boundaries that intersect or touch are rejected.
Self-intersecting closed polylines are also rejected as ambiguous regions.
The command supports the same `Corner` and `OutputLayer` options as `Offset`;
at most 100,000 source/count combinations can be requested. All outputs are
staged before the document changes.

`OutputLayer=Current` is the default; `OutputLayer=Input` uses each source's
layer. Both choices create fresh object attributes. The command stages every
result before changing the document, so an unsupported or degenerate selected
curve leaves the document unchanged. Ellipse, NURBS, and polycurve
offsets, as well as smooth corners, trim, cap, and construction-plane overrides, remain
to be implemented.
