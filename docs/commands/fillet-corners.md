# FilletCorners

[Command reference](README.md) · [Rhino command](https://docs.mcneel.com/rhino/8/help/en-us/commands/fillet.htm)

Select one or more open or closed polylines, NURBS curves, or supported polycurves, then enter
`FilletCorners 0.5` or `FilletCorners Radius=0.5`. Turns between straight spans
and supported joints involving circular arcs or planar NURBS curves receive exact
circular arcs tangent to both sides.
The radius must exceed the document absolute tolerance. Each source segment
must have room for the setbacks at both ends. Adjacent arcs may meet at a tangent point.
A radius that makes them overlap rejects the entire
selection without editing it.

The result is a polycurve with native line and arc leaves and any preserved
curved leaves. Original object IDs,
attributes, groups, and selection are retained. One command changes all
selected curves in one Undo step. Straight polycurves may contain line,
polyline, and linear NURBS knot-span leaves; their junctions must be exact.
Open and closed polycurves may also contain circular arcs and curved NURBS leaves. These
leaves retain their exact geometry when their internal knots and adjoining
leaf junctions are smooth; corners within straight runs receive the fillets.
Coplanar kinks between a native circular arc and a line, polyline, or linear
NURBS leaf are also rounded when the radius fits both sides. Straight leaves
are split into their exact line spans when needed; trimmed arcs remain native.
Coplanar kinks between two circular arcs also receive a native tangent arc.
Coplanar kinks between a curved NURBS leaf and a line or circular arc receive
a native tangent arc while the NURBS remains a trimmed NURBS leaf.
Coplanar kinks between two curved NURBS leaves also receive a native tangent
arc, with both source leaves trimmed at their tangent points.
Sharp internal knots of a curved NURBS leaf are split into exact NURBS pieces
and rounded in the same way, including when the selected object is a standalone
NURBS curve.
For closed polycurves, a sharp seam between straight leaves also receives a
fillet, and the result starts at that fillet's incoming tangent point.
The same seam convention applies to a sharp curved NURBS junction.
Stationary cusps, viewport radius picking, and interactive preview remain to be
implemented.

The [geometry fixture](../../tools/rhino_oracle/fixtures/curve_fillet_corners.json)
and [saved Rhino response](../../tools/rhino_oracle/observations/curve_fillet_corners.json)
compare open, closed, tangent-meeting, and spatial polylines, three straight
polycurves, twenty-three polycurves with smooth or filleted curved leaves, and
one standalone NURBS curve against RhinoCommon's public
`CreateFilletCornersCurve` method. Thirty cases use 65
equal arc-length stations; the smooth quadratic NURBS case uses 17 fixed
closest-point probes to avoid differences in the two engines' arc-length
inversion. Closure and total length also agree. The largest sampled coordinate
difference is `5.1e-8`, in a curved NURBS fillet. The nine curved NURBS kink
cases have a `1e-7` comparison tolerance; earlier cases retain their tighter
saved oracle tolerances.
Rhino places a closed fillet result's seam at the incoming tangent
point of the source seam corner; the native result follows that convention. Run:

```sh
tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/curve_fillet_corners.json \
  --absolute-epsilon 1e-7 --relative-epsilon 1e-11 --timeout 240
```
