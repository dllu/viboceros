# Offset

[Command reference](README.md) · [Project overview](../../README.md)

Select one or more lines, circles, or circular arcs, then enter
`Offset distance side-point`. The point chooses the side of each curve; the
distance must be positive. `Offset distance BothSides=Yes` creates one curve on
each side without a point. The selected originals remain in the document, and
the new curves become selected.

Lines offset to the left or right of their direction in the current
construction plane. Circles and arcs offset radially in their own planes.
Offsets are analytic and retain their source parameter intervals. An inward
offset that would collapse a circle or arc is rejected. A side point on the
supporting line or circle is ambiguous and rejected.

`OutputLayer=Current` is the default; `OutputLayer=Input` uses each source's
layer. Both choices create fresh object attributes. The command stages every
result before changing the document, so an unsupported or degenerate selected
curve leaves the document unchanged. Polyline, ellipse, NURBS, and polycurve
offsets, as well as corner, trim, cap, and construction-plane overrides, remain
to be implemented.
