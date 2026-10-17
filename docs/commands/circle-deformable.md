# Deformable Circle

Choose `Deformable` at Circle’s first prompt, or start `Circle Deformable`
(alias `C Deformable`). Set `Degree` and `PointCount`, then construct the circle.
Settings accept `Degree=5`, `PointCount=12`, or a separate integer prompt.
Enter retains the integer setting; Escape and `Cancel` cancel the command.

Deformable output works with center/size, 2Point, 3Point, 3Point Radius,
Vertical, Orientation, [AroundCurve](circle-around-curve.md),
[Tangent](circle-tangent.md), and FitPoints.
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

The output is a closed, non-rational NURBS curve with an angular domain
`[0,2*pi]`. At or above the counts in the table below, it is periodic:
PointCount counts unique controls, with Degree controls repeated at the seam.
The shared periodic basis uses a linear-time first-harmonic solve. Even degrees
interpolate the seam stations; Rebuild uses half-span stations instead.

Smaller counts create a clamped approximation with
`max(PointCount,Degree)+1` controls. It interpolates `PointCount+1` equal-angle
samples at clamped Greville parameters. When Degree exceeds PointCount, the
curve is interpolated at PointCount degree and then elevated exactly. This
reuses the existing normalized interpolation and degree-elevation code.
Small-count approximations can deviate visibly from an analytic circle and
have a kink at the seam, matching Rhino’s construction.

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

Both branches work with the same construction modes, previews, guide picking,
FitPoints and Undo/Redo. Low-count command examples use the same setting order
as Rhino’s integer getters:

```text
Circle Deformable Degree=2 PointCount=4 Degree=11 0,0,0 3
Circle Deformable Degree=2 PointCount=4 Degree=8 AroundCurve
```

The second example opens the guide picker. Inline settings also accept bare
integer getters, for example `C Deformable Degree 2 PointCount 4 Degree 8`.

Native preference updates after cancellation remain unqualified. There is no
performance-parity claim.

[Periodic-output validation](../circle-deform-validation.json) records the
original 57 successful workflows and four cancellations. The
[clamped-output validation](../circle-clamped-validation.json) covers every
small-count layout, construction modes, saved defaults, guide/edge and FitPoints
selection, previews and actual history on both backends at absolute `1e-6`.
Eight earlier clamped observations now replay successfully. Two default native
length estimates are retained separately; explicit-accuracy recaptures verify
their geometry and length at the same comparison threshold.

References: McNeel’s [Circle command](https://docs.mcneel.com/rhino/8/help/en-us/commands/circle.htm),
the public [CreateFromCircle API](https://developer.rhino3d.com/api/RhinoCommon/html/M_Rhino_Geometry_NurbsCurve_CreateFromCircle_1.htm),
and [GetLength accuracy overloads](https://developer.rhino3d.com/api/rhinocommon/rhino.geometry.curve/getlength?version=8.x).
