# Circle AroundCurve

Choose `AroundCurve` at Circle's first prompt, or start `Circle AroundCurve`
(alias `C AroundCurve`). Select a curve or surface/B-rep edge, pick the center
on that guide, then pick a size point or enter a size.

One preselected curve is accepted automatically. You can also use
`Curve=<object-id>` or `Edge=<object-id>,<edge-index>` at the source prompt.
The source stays unchanged. The center is the closest guide point, including
endpoint clamping, and the circle's plane is perpendicular to its tangent.
Size points are projected into that plane before measuring the size.

| Size getter | Numeric input | Picked input |
| --- | --- | --- |
| Radius | Radius. | Radius and seam direction. |
| Diameter | Half the value becomes the radius. | Radius and seam direction, as in Rhino's standard Circle getter. |
| Circumference | Value divided by `2*pi` becomes the radius. | Projected distance from the center supplies the circumference; the seam follows the guide frame. |
| Area | Square root of value divided by `pi` becomes the radius. | Projected distance from the center supplies the area; the seam follows the guide frame. |

Radius, diameter and circumference values accept the shared calculator and
length units. Area values use squared document units; calculator expressions
work, but explicit physical area-unit suffixes are not supported. Length
suffixes are rejected at the Area prompt.

Enter at Radius or Diameter uses the remembered size. Enter at source, center,
Circumference or Area cancels. Escape and `Cancel` always cancel. Invalid sizes
and zero projected radius picks leave the getter available for correction.
Previews create no objects and use the same circle definition as the commit.
Point filters, parameter picks, snapping and world/CPlane/relative input pass
through the common point pipeline. Relative input uses the accepted constrained
center and active CPlane vector axes.

Successful size constructions remember the radius and Radius/Diameter choice
per command registry, across document Undo/Redo. Plain 2Point and 3Point
constructions retain the old radius; 3Point's Radius option updates it.
Pending local size-mode changes save only after success. Native preference
changes on cancellation are not qualified.

A guide that changes, disappears, becomes hidden/locked, or outlives a document
tolerance change must be picked again. This local freshness policy applies
before and after the center, clears pending filters/constraints and retains
the size mode. It is not a claim about Rhino's source-freshness behavior.

The completed circle forms one `Circle` Undo/Redo operation. Postselected whole
guides are released when the command returns; preselected guides remain selected.
Picking an edge leaves its parent unselected. Cancellation creates no output
or history step.

Complete command examples:

```text
Circle AroundCurve Curve=<id> 0,0,5 3,0,9
Circle AroundCurve Curve=<id> 0,0,5 Diameter 6
Circle AroundCurve Edge=<id>,0 5,0,0 Circumference 18.84955592153876
```

## Verification and limits

[Validation](../circle-around-validation.json) compares 43 successful Rhino 8.32
workflows through command and frontend paths on both backends at absolute
`1e-6`. Checks include every NURBS control, weight, knot, domain, length,
unchanged guide geometry, returned selection and actual Undo/Redo. Guides
include lines, arcs, circles/ellipses, NURBS and polylines, with five acknowledged
owned surface/B-rep edge clicks. Inputs cover projected points, numeric size
modes, relative coordinates, endpoints/kinks, remembered sizes and a corrected
zero-radius retry. Five native cancellations also replay without geometry or
history. An unsupported Orientation attempt remains a separate diagnostic.

Rhino 8.32 does not offer Orientation in AroundCurve's size getter. This path
creates exact circles by default. Choose [Deformable](circle-deformable.md) before
AroundCurve for a NURBS approximation. [Tangent](circle-tangent.md) builds
circles from picked tangent contacts and free points.
The native per-getter ProjectOsnap switch is not wired to this getter.
Exhaustive physical snapping, perspective/aperture behavior, extreme ranges
and performance parity remain unqualified.

Reference: McNeel's public
[Circle command documentation](https://docs.mcneel.com/rhino/8/help/en-us/commands/circle.htm).
