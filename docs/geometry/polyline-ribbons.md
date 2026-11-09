# Ordinary polyline ribbons

[Geometry guides](README.md) · [Corner validation](ribbon-corners.md)

`try_polyline_ribbon` builds ordinary planar polyline ribbons. Its
`RibbonCornerStyle` supports Sharp, Round, Smooth, Chamfer and None; existing
`CurveOffsetCornerStyle` arguments also remain accepted. BothSides sews the two
sides at original source edges. Source and offset corner identities also share
point-only contacts, including None gaps and clipped Sharp closing seams.

The signed distance chooses the first side. U spans `[0,abs(distance)]`.
Segment V parameters retain their source chart; outer fan charts follow their
offset arc or line. Sharp clips use the affine clock of the extended offset
strip. Closing seams can retain clipped intervals without added seam fans when
both incident strips were extended at other corners.
This reproduces the observed native seam behavior rather than guaranteeing a
ribbon over every source parameter at a Sharp seam.

Round and Chamfer outside corners add a constant-source fan; rational Round
fans use the same weights on both rows. Smooth adds a cubic tangent fan. Inside
corners split into strips and constant-offset triangles where required by the
corner policy. None omits outside fans. The `_with_ruled` variant delegates
tensor construction through `viboceros_smlib::ruled_surface`.

The local Python operation `polyline_ribbon_geometry` accepts `vertices`, signed
`distance`, `corner`, optional `parameters`, `both_sides` and `normal` (default Z).
It returns complete `breps`, unchanged source readbacks and false flags for a
constant-distance certificate and full command parity. Invalid distances,
nonplanar inputs and collapsed offsets fail. A conservative 4096-patch guard
bounds construction before backend calls. Point identities require both kernel
coincidence and model tolerance; undeclared contacts remain independent.

The [Ribbon command](../commands/ribbon.md) exposes these corners with selection,
distance getters and object-history policies. [Smooth NURBS ribbons](smooth-ribbons.md)
have their own constructor. Self-intersection trimming, full command parity,
face reversal, loop winding and performance parity remain unqualified.
