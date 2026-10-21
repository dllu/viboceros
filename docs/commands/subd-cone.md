# SubDCone

`SubDCone` creates a subdivision cone with a rounded smooth apex. A solid cone
also has a smooth cap; an open cone has a creased base boundary. Dimensions
locate the control cage, so the rounded limit surface stays inside that cage.
The defaults are eight faces around, four vertically, a Tri cap, radius 1 and
height 10. Its settings are independent of SubDCylinder and the other primitives.

```text
SubDCone 0,0,0 3 5
SubDCone AroundFaces=8 VerticalFaces=2 CapFaceStyle=Quad 1,2,3 2 -5
SubDCone Solid=No 2Point -3,0,0 3,0,0 DirectionConstraint=None 3,4,6
SubDCone 0,0,0 3 OrBaseAngle=30
SubDCone 0,0,0 3 OrApexAngle=-90
SubDCone FitPoints
```

At the base prompt, set AroundFaces (at least 3), VerticalFaces (at least 2),
Solid=Yes|No and CapFaceStyle=Tri|Quad. Odd counts use triangle caps. Three sides
use one triangular cap; four sides use one face with Quad or a fan with Tri.
Construction is bounded to 100,000 control vertices and faces.

The circle getter accepts center/radius, a spatial radius point, Diameter,
Circumference, Area, 2Point and spatial 3Point. Tangent accepts point contacts,
curves or surface edges, a fixed radius, late Radius, FromFirstPoint and Enter
for two contacts. FitPoints uses selected point objects and displayed selected
control points, including rational curve controls. With fewer than three
eligible preselected points, select points or grips and press Enter.

After the circle, enter a signed height or pick an endpoint.
DirectionConstraint=Vertical follows the base normal; None rotates the base
perpendicular to the picked axis. A numeric free-axis height follows the cursor
direction. Enter accepts the remembered radius/diameter or height.

OrBaseAngle and OrApexAngle compute height from a signed angle in degrees.
The base angle must lie strictly between -90 and 90, excluding zero; the apex
angle between -180 and 180, excluding zero. Enter after choosing an angle option
uses its remembered value. These options are unavailable after tangent curve contacts; a point-only tangent
base retains them.
The captured Rhino build also rejects BothSides, AroundCurve, Circle Orientation
and VerticalFaces=1; Vertical resolves to VerticalFaces.

Scripts use world coordinates. The viewport getter uses shared unit, coordinate,
filter and snap tools and latches the first pick's construction plane. Preview
circles, apex edges and the axis stay outside the document. Completion adds one
unselected SubD on the current layer in one Undo record. Escape, stale sources,
changed history or changed tolerance cancel the draft. Cancellation and failed
construction preserve pending Redo; external document edits retain their own
history effects. Degenerate point fits reach the height prompt, then fail without
adding an object. Existing SubD display, control editing, transforms and 3DM
interchange apply.

The [qualification report](../subd-cone-qualification.json) retains 56 primary
Rhino recipes: 44 constructions, nine cancellations, two degenerate-fit failures
and one rejected tangent-angle diagnostic, cancelled by the owned driver after a
timeout. It compares 46 command cases and 55
getter cases at `1e-6`, covering topology, winding, tags, control points, vertex
limit stations, sampled density-two surfaces, source attributes/selection, grips
and real Undo/Redo. Physical native picks replay acknowledged projected contacts.
Another 29 exploratory recipes retain option diagnostics and fallback effects;
they are excluded from supported comparisons.

General option clocks, arbitrary fitted seam/normal choices, surface-edge tangent
parity, native face packing, material/texture channels and comparative speed need
further qualification. Facet sampling does not certify a continuous Hausdorff bound.
