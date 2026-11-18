# Tube

Run `Tube` and pick a center and first wall. Enter a radius, use `Diameter`,
`Circumference`, or `Area`, or pick a point. The base also supports `2Point`,
`3Point`, `Tangent`, and `FitPoints`. With `DirectionConstraint=AroundCurve`,
select a guide and a station to orient the base perpendicular to the guide.

Pick the second wall or enter its radius in the current radius or diameter
mode. The second rim stays concentric with the first, and a point pick is
projected into the base plane. Use `WallThickness` to set the distance from the
first wall. A positive value moves outward and a negative value moves inward.
Enter or zero accepts the remembered thickness. Relative points for the
second wall and height use the base center as their origin.

Pick the end of the tube or enter a signed height. `Vertical` follows the base
normal, `AroundCurve` follows the guide tangent, and `None` allows a free
direction. In free direction, a typed height fixes the length and waits for a
direction pick. Set `BothSides=Yes` to extend the same height in both directions.
`Solid=Yes` adds annular caps. `Solid=No` creates two independent cylindrical
walls in one undo step. `UseExtrusions` controls their representation.

Coincident radii follow Rhino’s fallback. `Solid=Yes` returns two coincident
walls and four disk faces in one B-rep. `Solid=No` returns the four disk faces
as separate objects.

```text
Tube DirectionConstraint=Vertical w1,2,3 5 WallThickness=-2 BothSides=Yes 8
```

Existing positional scripts can specify the two radii and an explicit axis.

```text
Tube 1,2,3 5 2 8 Axis=0,0,1
```

[Surface commands](surfaces-index.md)
