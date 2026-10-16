# RailRevolve

`RailRevolve axis-start axis-end [RailName=name|RailId=id] [ScaleHeight=Yes|No] [SplitAtTangents=Yes|No]`

Select the profile curve first and the rail second. Run `RailRevolve`, pick the
axis start and end, or press Enter after the start to use the active CPlane's Z
direction. You can also start the command before selecting the curves. Curve
picking retains a preselected profile and filters other geometry; Escape
cancels. Both options can be changed during selection and axis picking.

For example, in a new document:

```text
Line 2,0,2 1,0,5
Circle 0,0,2 2
SelAll
RailRevolve 0,0,0 0,0,1
```

For scripts, a unique case-insensitive `RailName` or an object UUID in `RailId`
can identify a visible, unlocked rail outside the selection. Select only the
profile in that case. Names currently cannot contain whitespace. Construction
preserves both sources and their selection and creates one Undo/Redo step.

The rail's start fixes the initial radial scale and rotation. Rational and
spatial profiles, closed rails, unclamped bases and arbitrary directed axes are
supported. Place a closed rail's seam at its contact with the profile.
`ScaleHeight=No` (default) translates profile heights with the rail's axial
variation. `ScaleHeight=Yes` scales heights about the axis origin; its location
therefore matters. At reference heights within `2^-26` of zero, scaling falls
back to translation, matching the observed inclusive Rhino threshold.

`SplitAtTangents=Yes` (default) uses native continuity classification to split
kinks and tangent joins into connected B-rep faces. `No` retains a single NURBS
surface with its internal joins. Options are remembered for subsequent calls.

This implementation requires `native-smlib`. It reuses usd-brep's rational
`SmBSplineSurface::CreateSwungSurface` tensor, with an axis-local coordinate map
for spatial profile rotation and axial following. Kernel inputs are private
copies; no Rhino proprietary code is used. Common-sign rational weights are
normalized before multiplication. Native curve import supports degree at most
17 and has further rational-weight limits. `ScaleHeight=Yes` requires endpoint-
clamped active bases: the public SDK gives different results for unclamped
rational quadratic inputs, so this combination is rejected pending a match.
Default axial translation supports native active-interval clamping. A rail starting within the document's
absolute tolerance of the axis is rejected. The output net is limited to one
million controls, with the existing native transfer and crease-splitting limits
also enforced. Invalid, overflowing or unsupported constructions fail before
adding geometry.

The [qualification record](../rail-revolve-qualification.json) retains fresh
licensed public SDK observations, full rational control nets, and validation
logs. Across 12 fixtures, a continuous rational control-net bound is below
`1e-6` model units over every complete surface. The largest bound is
`9.84e-7` at an origin near `3e9`. The comparison matches tensor bases under
chart transposition/reversal and bounds both control displacement and weight
roundoff. All 300 closest-point queries agree within `2e-6` model units and
areas within `1e-5` square model units. These finite fixture qualifications do
not certify arbitrary inputs.

Reference construction uses `NurbsSurface.CreateRailRevolvedSurface`;
these checks do not establish Rhino native command getter, split-topology,
metadata, associative history or performance parity.

References: [Rhino command](https://docs.mcneel.com/rhino/8/help/en-us/commands/railrevolve.htm),
[public construction API](https://developer.rhino3d.com/api/rhinocommon/rhino.geometry.nurbssurface/createrailrevolvedsurface).
