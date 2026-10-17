# Deformable Circle

Choose `Deformable` at Circle’s first prompt, or start `Circle Deformable`
(alias `C Deformable`). Set `Degree` and `PointCount`, then construct the circle.
Settings accept `Degree=5`, `PointCount=12`, or a separate integer prompt.
Enter retains the integer setting; Escape and `Cancel` cancel the command.

Deformable output works with center/size, 2Point, 3Point, 3Point Radius,
Vertical, Orientation, [AroundCurve](circle-around-curve.md), and FitPoints.
Radius, Diameter, Circumference and Area use the ordinary Circle sizing rules.
FitPoints reuses the existing point/grip fitter and selection workflow.
World, CPlane and relative points, snapping, filters and constraints use the
common drafting pipeline. Previews show the actual output NURBS without
editing the document. The construction retains its original plane when the
active view changes.

```text
Circle Deformable Degree=5 PointCount=12 0,0,0 3
Circle Deformable Degree=4 PointCount=12 Vertical 0,0,0 3 3,4,5
Circle Deformable Degree=3 PointCount=8 AroundCurve Curve=<id> 0,0,5 3
Circle Deformable Degree=5 PointCount=12 FitPoints
```

The output is a closed, non-rational periodic curve, with an angular domain
`[0,2*pi]`. PointCount counts unique controls; the stored NURBS repeats Degree
controls at the seam. Equal-angle interpolation uses the shared periodic
B-spline basis and a linear-time first-harmonic solve. Even degrees interpolate
the seam stations, unlike the general Rebuild command’s half-span stations.

The initial settings are degree 5 and 10 points. Successful constructions
remember degree and point count per command registry, across model Undo/Redo.
Deformable output must be chosen again for each command; starting an ordinary
Circle produces an exact circle. Each completed construction creates one
`Circle` history entry. Cancel creates no geometry or history. AroundCurve
retains its existing preselection, postselection and source freshness policy.

Degree accepts 2 through 11. PointCount accepts Degree+2 through 1000 at its
integer getter. Raising Degree after choosing a lower degree’s point count can
retain a periodic layout with Degree+1 unique controls. The periodic output
branch has these measured minimum counts:

| Degree | Minimum unique controls |
| --- | --- |
| 2–3 | 4 |
| 4 | 5 |
| 5 | 6 |
| 6 | 7 |
| 7 | 9 |
| 8 | 10 |
| 9 | 12 |
| 10 | 13 |
| 11 | 15 |

Rhino uses a different, nonperiodic approximation below these limits. That
branch is not implemented. The ordinary Circle getter rejects those settings
before accepting a point, allowing PointCount to be increased in the same
session. AroundCurve and FitPoints also reject these layouts without creating
output.
Tangent construction and native preference updates after cancellation remain
unimplemented or unqualified. There is no performance-parity claim.

[Validation](../circle-deform-validation.json) compares 57 successful Rhino 8.32
workflows (56 periodic curves and one exact-output reset), plus four clean
cancellations. It records full native NURBS,
selection and actual Undo/Redo comparisons through command and frontend paths
on both backends, together with local preview, input and regression checks.
Unsupported attempts and incomplete input receipts stay outside the qualified
corpus.

References: McNeel’s [Circle command](https://docs.mcneel.com/rhino/8/help/en-us/commands/circle.htm)
and public [CreateFromCircle API](https://developer.rhino3d.com/api/RhinoCommon/html/M_Rhino_Geometry_NurbsCurve_CreateFromCircle_1.htm).
