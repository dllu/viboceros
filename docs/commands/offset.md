# Offset

[Command reference](README.md) · [Project overview](../../README.md)

Select one or more lines, circles, circular arcs, or planar polylines, then enter
`Offset distance side-point`. The point chooses the side of each curve; the
distance must be positive. `Offset distance BothSides=Yes` creates one curve on
each side without a point. The selected originals remain in the document, and
the new curves become selected.

Lines offset to the left or right of their direction in the current
construction plane. Circles and arcs offset radially in their own planes.
Planar polylines use sharp intersections of neighboring offset segments and
retain one parameter per vertex. Collinear polylines use the construction
plane; other planar polylines use their own plane, aligned with the
construction plane normal when possible. A side point chooses the nearest
polyline segment's side. The sharp result can self-intersect; automatic trimming
and corner styles remain to be implemented. Reversed or degenerate output
segments are rejected.

Offsets retain their source parameter intervals. An inward
offset that would collapse a circle or arc is rejected. A side point on the
supporting line or circle is ambiguous and rejected.

`OutputLayer=Current` is the default; `OutputLayer=Input` uses each source's
layer. Both choices create fresh object attributes. The command stages every
result before changing the document, so an unsupported or degenerate selected
curve leaves the document unchanged. Ellipse, NURBS, and polycurve
offsets, as well as round/smooth/chamfer corners, trim, cap, and construction-plane overrides, remain
to be implemented.
