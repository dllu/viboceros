# Fillet

[Command reference](README.md) · [Rhino command](https://docs.mcneel.com/rhino/8/help/en-us/commands/fillet.htm)

Select exactly two open curves, then enter `Fillet 0.5` or
`Fillet Radius=0.5`. The command trims the selected ends, inserts a tangent
circular arc, and joins the three pieces into one polycurve. The result is
selected and the two sources can be restored with one Undo. It inherits the
first source's attributes and groups.
With radius zero, the curves meet at a sharp corner without an inserted arc.

The nearest pair of source endpoints determines which ends are filleted. Use
`Pick1=x,y,z` and `Pick2=x,y,z` to choose different ends. The picks identify
the end; they do not need to lie exactly on the curves. Line ends may be
extended to their supporting-line intersection. Already meeting line, arc,
and NURBS terminal leaves are supported, including terminal leaves of
polycurves. Existing earlier leaves remain native and unchanged.

The `Join=No`, `Trim=No`, dynamic preview, and extension of nonmeeting curved
leaves are still to be implemented. A pick exactly at an arc endpoint can be
ambiguous in RhinoCommon's public pair-filleting method; pick a nearby point
on the arc when comparing outputs.

The [nine-case fixture](../../tools/rhino_oracle/fixtures/curve_fillet_pair.json)
and [saved Rhino response](../../tools/rhino_oracle/observations/curve_fillet_pair.json)
compare meeting and extended lines, reversed selection, arc-to-line,
NURBS-to-line, NURBS-to-NURBS, and zero-radius line joins. Each uses 65 equal arc-length
stations. The largest coordinate difference is below `3.2e-8`; the NURBS
cases use a `1e-7` tolerance. Run:

```sh
tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/curve_fillet_pair.json \
  --absolute-epsilon 1e-7 --relative-epsilon 1e-11 --timeout 240
```
