# Circle Tangent

Start `Circle Tangent` (`C Tangent`), or choose `Tangent` at Circle’s first
prompt. Pick locations on curves or surface edges. Choose `Point` before
each free point. Three contacts complete the circle; Enter accepts two.
After the first contact, a positive number locks the radius and the second
contact completes the circle. After two curve contacts, `Radius` opens the
radius getter. Enter there accepts the saved radius.

With two curves and Enter, the second picked tangent location stays fixed;
the first contact slides. Choosing `FromFirstPoint` after the first curve
holds that location instead and completes on the second contact. This option
resets each command. Circle seams and directions follow these contact rules.

Two free points and Enter make a diameter circle, starting at the first
point. For a free point followed by a radius and another free point, the
last point specifies the center direction. Mixed point/curve constraints
are also supported.

```text
Circle Tangent Curve=<id> 3,0,0 Curve=<id> 0,4,0 Enter
Circle Tangent Curve=<id> 3,0,0 3 Curve=<id> 0,4,0
Circle Tangent Curve=<id> 3,0,0 FromFirstPoint Point 0,4,0
Circle Tangent Point 3,0,0 Point 0,4,0 Enter
Circle Tangent Edge=<id>,<index> 3,0,0 Curve=<id> 0,4,0 Enter
Circle Deformable Degree=4 PointCount=12 Tangent
```

The final example opens the same picker with Deformable output settings.
Point input uses the common world/CPlane/relative, snapping, filter and
constraint pipeline. The command retains its construction plane. Hover
previews use the same contact solver and output conversion as completion.

Sources and their selection remain unchanged. Edges are captured from their
parents without temporary document objects. If a source changes, disappears,
becomes unavailable, or the document tolerance changes, pick the contacts
again. A successful circle creates one `Circle` history entry; cancellation
creates none. Radius preferences survive model Undo/Redo.

The geometry solver uses analytic first and second derivatives, a normalized
contact system, and a damped Newton iteration. It verifies radius, distance,
plane and tangency constraints before accepting a solution. Analytic lines
can extend beyond their endpoints; other open curves retain their domains.
A failed solve leaves the previous input available for correction.

[Validation](../circle-tangent-validation.json) records native inputs and
full NURBS, source, selection and history comparisons. Native viewport rays
resolve the input locations used for these comparisons independently of
the resulting geometry. Pixel aperture parity, every possible multi-solution
branch, nonplanar guide combinations, and performance parity remain
unqualified. Surface-edge routing has local viewport coverage.

References: McNeel’s [Circle command](https://docs.mcneel.com/rhino/8/help/en-us/commands/circle.htm),
public [Circle.TryFitCircleTTT API](https://developer.rhino3d.com/api/rhinocommon/rhino.geometry.circle/tryfitcirclettt),
and [RhinoViewport.SetToPlanView](https://mcneel.github.io/rhinocommon-api-docs/api/RhinoCommon/html/M_Rhino_Display_RhinoViewport_SetToPlanView.htm).
