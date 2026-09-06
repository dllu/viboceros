# ConvertToBeziers

```text
ConvertToBeziers DeleteInput=No
ConvertToBeziers DeleteInput=Yes
```

Converts selected curves and underlying NURBS surfaces into exact, clamped Bézier
pieces. Analytic curves, polylines, and polycurves first acquire an exact NURBS
representation; mixed-degree polycurves consequently use its common degree.
Each nonempty knot span becomes one curve, or each U/V knot rectangle one surface.
Even an unclamped single-span input is genuinely clamped into a fresh object.

Every output has a `[0,1]` domain, or `[0,1]²` UV domain. Surface conversion drops
trims and uses the full underlying surface, not just its visible trimmed region.
Pieces follow source document order, then increasing span order; surface patches
use U-span outer/V-span inner order, independent of selection action order.

Outputs are unnamed, ungrouped, unselected, and have fresh current-layer attributes
(including color-by-layer), not copies of source attributes. `DeleteInput=No`
leaves source identity, attributes, memberships, and selection unchanged.
`DeleteInput=Yes` deletes eligible sources but retains empty group definitions.
Only single-face B-reps are eligible: polysurfaces are ignored, even when a curve
is also selected. Selected points, point clouds, and meshes are ignored. With no
eligible selection, the command returns an error without modifying the document.

Viboceros currently defaults to `No`; Rhino remembers its last deletion choice.
Use explicit options for reproducible scripts. Sticky-option parity is not yet
implemented. All generated geometry is staged before insertion/deletion in one
undoable transaction; geometry failures roll back without partial output.

## Geometry and limits

The geometry crate's `NurbsCurve::try_bezier_spans` and
`NurbsSurface::try_bezier_patches` retain original span domains; unit-domain
reparameterization belongs to the command layer. Local homogeneous blossoming is
shared with tight-bound extraction. It avoids repeated whole-spline splitting,
supports periodic/unclamped ends and full-order knots, and projects only after
both tensor directions have been extracted. An intermediate zero weight is not
mistaken for a final control at infinity.

Local affine origins and normalized intermediate weights limit avoidable
cancellation/overflow; final weights retain the source gauge. Final zero-weight
or non-finite controls cannot be represented by `WeightedPoint3` and return an
explicit error. Decomposition is bounded to 1,048,576 aggregate output controls
and 33,554,432 estimated extraction work units per curve/surface; the command
also bounds aggregate output controls across sources. These are resource limits,
not accuracy tolerances. Performance parity with Rhino is not established.

## Verification

The 44 `bezier_conversion.json` comparisons record source identity, full output
NURBS definitions,
33 curve samples or 25 surface samples, native domains, creation order, layers,
stored colors/color sources, names, ordered memberships, selection, and complete
group tables. No attributes are silently normalized to match. New Rhino surface
outputs must have trivial trimming, checked with
[`Brep.IsSurface`](https://developer.rhino3d.com/api/rhinocommon/rhino.geometry.brep/issurface).
The recorded comparisons use absolute `1e-8` and relative `1e-12` epsilon;
maximum observed numeric error is `2.67e-15`. Default-prompt probes are diagnostic
only, since they depend on Rhino's session history.

Native tests additionally check first/second curve derivatives, tensor partials,
periodic seams, independent full-order weight gauges, extreme common scales,
large translations, intermediate zero weights, explicit resource failures, and
command undo/redo/rollback. Python tests exercise partial-failure cleanup of owned
objects, groups, layers, selection, and disposable construction geometry.
An isolated GUI test converted a curve and surface with both deletion choices,
exercised undo/redo, and checked three 3DM exports for exact geometry, unit domains,
current-layer attributes, and retained empty groups.

The [Rhino command documentation](https://docs.mcneel.com/rhino/8/help/en-us/commands/convert.htm)
describes the conversion; document attributes and ordering above were established
by running commands in the licensed Rhino 8 oracle, not by inspecting proprietary code.
