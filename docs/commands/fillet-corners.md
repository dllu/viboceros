# FilletCorners

[Command reference](README.md) · [Rhino command](https://docs.mcneel.com/rhino/8/help/en-us/commands/fillet.htm)

Select one or more open or closed polylines, then enter `FilletCorners 0.5`
or `FilletCorners Radius=0.5`. Every nonstraight corner receives an exact
circular arc tangent to its two adjacent straight segments. The radius must
exceed the document absolute tolerance. Each source segment must have room
for the setbacks at both ends. Adjacent arcs may meet at a tangent point.
A radius that makes them overlap rejects the entire
selection without editing it.

The result is a polycurve with native line and arc leaves. Original object IDs,
attributes, groups, and selection are retained. One command changes all
selected polylines in one Undo step. The initial command supports polyline
objects; Rhino also supports kinked polycurves. Viewport radius picking and
interactive preview remain to be implemented.

The [geometry fixture](../../tools/rhino_oracle/fixtures/curve_fillet_corners.json)
and [saved Rhino response](../../tools/rhino_oracle/observations/curve_fillet_corners.json)
compare open, closed, tangent-meeting, and spatial polylines with RhinoCommon's public
`CreateFilletCornersCurve` method at 65 equal arc-length stations. Closure and
total length also agree. The largest sampled coordinate difference is
`5.4e-15`. Rhino places a closed fillet result's seam at the incoming tangent
point of the source seam corner; the native result follows that convention. Run:

```sh
tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/curve_fillet_corners.json \
  --absolute-epsilon 1e-8 --relative-epsilon 1e-11 --timeout 240
```
