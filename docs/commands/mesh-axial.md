# MeshCone, MeshCylinder, and MeshTruncatedCone

Draw the base circle from a center and size, two or three points, tangent
contacts, fitted points, or a guide curve. Set the signed height with a point or
number. MeshTruncatedCone then accepts a top radius, diameter, or projected
point. A zero top size collapses the top ring to the apex.

`VerticalFaces` and `AroundFaces` control the wall grid. Set `Solid` to
add caps. The cap style appears for solid meshes with even circumferential
counts. Odd counts use triangles and keep the chosen cap style for the
next even count. MeshCylinder offers `BothSides` at the height prompt.

The wireframe preview shows the candidate mesh. High face counts use the
shape outline during preview. Accepted geometry uses the specified counts.
Short positional scripts also accept the explicit `Axis=x,y,z` option.

```text
MeshCone VerticalFaces=3 AroundFaces=12 CapFaceStyle=Quad 0,0,0 4 8
MeshCylinder DirectionConstraint=None 0,0,0 Diameter=6 BothSides=Yes 2,3,5
MeshTruncatedCone Solid=No 0,0,0 4 5 Diameter=3
```
