# Cone and Cylinder

[Surfaces and solids](surfaces-index.md) · [Command input](../command-line.md)

Start with a center, or choose `2Point`, `3Point`, `Tangent` or `FitPoints`
to construct the base circle. A centered base accepts radius, diameter,
circumference or area. `ProjectOsnap` controls projection of snapped radius
and diameter points. Set `Solid` at the base prompt to choose caps.

`DirectionConstraint=Vertical` measures height along the base-circle normal.
`None` points the axis toward the end pick. For typed heights in `None`
mode, the cursor establishes the positive direction. Choose
`DirectionConstraint=AroundCurve` at the base prompt to pick a guide and
place the base perpendicular to its tangent.

Pick the endpoint or type a signed height. Cylinder offers `BothSides`,
which extends the given distance in both directions from the base.
Cylinder honors the `UseExtrusions` setting. The preview shows the base
circle and then the rims and rulings of the candidate wall.

Short positional scripts still accept `Axis=x,y,z`. The form
`Cone center radius height` produces an open NURBS surface by default.
In the staged command, `Solid` defaults to `Yes`.

`FitPoints` uses Viboceros's least-squares circle. Its fitted normal can
point the opposite way from Rhino's, which changes the side chosen by a
typed height in `Vertical` mode. A point pick specifies the intended side.

```text
Cylinder DirectionConstraint=Vertical 0,0,0 Diameter=6 BothSides=Yes 5
Cone Solid=No DirectionConstraint=None 0,0,0 3 4,2,5
Cone DirectionConstraint=AroundCurve Curve=12 0,3,3 6,3,8 5
```
