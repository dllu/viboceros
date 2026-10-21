# SubDCylinder

`SubDCylinder` creates a subdivision cylinder using OpenNURBS, with native
surface-radius placement. Its default is eight faces around, four along the
axis, and solid ends capped by triangle fans.

```text
SubDCylinder 0,0,0 3 5
SubDCylinder AroundFaces=8 VerticalFaces=2 CapFaceStyle=Quad 1,2,3 2 -5
SubDCylinder Solid=No 2Point -3,0,0 3,0,0 DirectionConstraint=None 3,4,6
SubDCylinder 0,0,0 Diameter 6 BothSides=Yes 5
```

Choose `AroundFaces` (at least 3), `VerticalFaces` (at least 2), `Solid=Yes|No`
and `CapFaceStyle=Tri|Quad` at the first prompt. Odd around-face counts use
triangle caps when Quad is requested. Three-sided and four-sided caps use
single faces where appropriate. Open boundary rings have crease tags.

The shared Circle getter supports center/radius, a point on the radius,
`Diameter`, `Circumference`, `Area`, `2Point`, and spatial `3Point` bases.
`Tangent` and `FitPoints` are not implemented for this command. Circle's
`Vertical` and `Orientation` base modes are unavailable in the captured Rhino
build. Use a spatial 3Point base or change the construction plane instead.

After the base, enter a signed height or pick an end point.
`DirectionConstraint=Vertical` follows the base circle's normal; `None` rotates
the circle perpendicular to the picked axis. `BothSides=Yes` extends the entered
height in both directions from the base. Set DirectionConstraint before
BothSides, because changing the direction option resets BothSides to No.
Enter accepts remembered radius/diameter or height at their respective prompts.
Counts, cap settings and dimensions are independent of SubDBox and SubDPlane.

Scripts use world coordinate triples. The viewport getter also uses the shared
point, unit, constraint and snap tools, latching the construction plane on the
first pick. Construction circles and an axis preview stay outside the document.
Escape cancels and preserves Redo. Changed history or model tolerance invalidates
a pending draft. Completion adds one unselected SubD on the current layer with
one Undo record. Existing SubD display, control-point editing, transforms and
native 3DM interchange apply. Construction is limited to 100,000 control vertices
and faces.

The [qualification report](../subd-cylinder-qualification.json) records 28 Rhino
`8.32.26160.13001` captures: 23 successful constructions, two cancellations and
three raw option diagnostics. Both command and interactive getter comparisons
check topology, winding, tags, control points, vertex limit stations, sampled
surfaces, attributes, selection and grouped history at `1e-6`. The rejected
option diagnostics retain any subsequent fallback macro effects and are excluded
from supported comparisons. Remembered-size sequences, cursor-driven numeric
free-axis input, every mouse trajectory, native material/texture channels and
comparative speed need further native qualification. Facet sampling carries no
continuous Hausdorff certificate.
