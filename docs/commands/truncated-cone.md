# TruncatedCone

Draw the base circle, specify the signed height or pick the top center, then
set the top radius or diameter. The live preview follows the cursor at each
stage. `Solid=Yes` caps the openings. A top size of zero or less creates an
apex and leaves only the base opening to cap.

The base circle supports the same center, two-point, three-point, tangent,
fitted, and curve-guided inputs as [Cone and Cylinder](cone-cylinder.md).
At the top prompt, a point is projected onto the plane through the accepted
end center, perpendicular to the axis. Its projected distance is the value
for the active radius or diameter mode. Enter accepts the previous top size.

```text
TruncatedCone Solid=Yes 0,0,0 Diameter=6 5 Radius=1
TruncatedCone DirectionConstraint=None 0,0,0 3 4,2,5 Diameter=2
TruncatedCone Solid=No 0,0,0 3 5 Radius=0
```

The short positional script form retains the open NURBS output default
and accepts an explicit `Axis=x,y,z` option. Use `Radius=0` at the top
prompt in a complete script to create an apex.
