# Fillet

[Command reference](README.md) · [Rhino command](https://docs.mcneel.com/rhino/8/help/en-us/commands/fillet.htm)

Enter `Fillet` and pick two curves, or two locations on one curve, near the intended contacts. Radius starts
at 1, Join at No, Trim at Yes, and Dynamic at No. Enter `Radius`, then a number
or two distance points to change the radius. Typed values accept document units
and arithmetic. Options remain available between the source picks.

Choose `Dynamic=Yes` while selecting the curves to preview the prospective
fillet. Change the radius with numbers or two points, including snapping and
typed 3D coordinates; these edits leave the getter active. Press Enter to accept.
The original sources remain highlighted while prospective pieces are drawn in
all viewports and display modes. Escape cancels without a geometry edit or loss
of pending Redo. Source/settings changes invalidate the getter.

For two distinct open sources, `Trim=Yes` changes them in place, preserving their IDs, attributes and
groups. `Join=Yes` additionally combines both sources and the arc into a
polycurve, retaining the first picked source's identity and metadata. A separate
fillet arc is new, unnamed, ungrouped geometry on the current layer. Results are
deselected, and one Undo restores the sources. With radius zero, there is no
inserted arc. Trimming an already meeting pair still records a replacement and
clears pending Redo, even when its geometry is identical.

`Trim=No` retains both complete sources and adds only the arc. Join is unavailable
until Trim is restored. Radius, Join, Trim and extension choices persist for the
application session, including after cancellation. Dynamic resets to No for each
invocation. See [getter evidence and limits](../fillet-getter.md).

For distinct sources, closed planar curves, including circles and smooth closed NURBS, remain complete.
Picks choose their local tangent contacts. Trim changes only the open partner,
retaining the side picked. Join includes the arc and any retained open piece,
leaving closed parents intact. An open first source keeps its identity and groups;
a closed first source supplies attributes to a new, ungrouped polycurve. With
Join=No, the new arc uses the current layer. A nonmeeting open partner can continue
on its supporting circle, endpoint tangent or smooth NURBS chart, using the same
extension options. See [closed-source evidence](../fillet-closed.md) and
[continuation behavior and limits](../fillet-continuation.md).

With radius zero, intersecting or extended open partners trim to the picked
intersection without an arc. `Trim=No` is a no-op. An already meeting open piece
can still record a replacement and clear Redo. See [zero-radius behavior for open
curves](../fillet-open-zero.md) and [closed-source coverage](../fillet-zero.md).

On one parent, Trim retains the two open ends or one cyclic closed remainder.
Join replaces that parent in place; without Join, a second open remainder is new,
ungrouped geometry with the parent's attributes. Polylines, polycurves and kinked
NURBS are covered, including a closed NURBS seam. See [same-parent behavior](../fillet-self.md).

Headless commands accept two selected curves, with nearest ends as the default,
or explicit ordered bindings:

```text
Fillet 0.5 Join=Yes Trim=Yes
Fillet Sources=id1,id2 Pick1=3.7,0,0 Pick2=4,0.3,0 Radius=0.5 Join=No
Fillet Sources=id,id Pick1=2.5,0,0 Pick2=4,1.5,0 Radius=0.5 Join=Yes
```

For open sources, picks identify a retained side; they do not need to lie exactly on a curve. Line ends may be
extended to their supporting-line intersection. `ExtendArcsBy=Arc|Line`
continues nonmeeting arcs on their supporting circle or adds a tangent line.
`ExtendOtherCurvesBy=Line|Smooth` adds a tangent line or smoothly continues a
nonmeeting NURBS. The defaults are `Arc` and `Line`. Already meeting line,
arc, and NURBS terminal leaves are supported, including terminal leaves of
polycurves. Earlier leaves remain native and unchanged.

Linux `native-smlib` builds use
[usd-brep producers](../smlib-curve-connections.md) to prepare smooth NURBS
connections and [extract NURBS contact intervals](../smlib-fillet-contacts.md)
during fillet construction.
Curved NURBS contacts now use the [native executive](../smlib-native-fillet-contacts.md)
in a normalized geometry frame, with independent radius and tangency checks.

When a larger fillet consumes an arc's tangent extension and touches the
original arc, the extension is removed and the native arc is retained. Other
fillets that consume an entire added segment can still be rejected. The distinct closed-source solver currently requires a radius
above the document's absolute tolerance for positive radii; zero uses a separate
intersection path. Corner contacts and tiny positive radii remain unqualified.
Smooth rational NURBS extension can differ
from Rhino. A pick exactly at an arc endpoint can be
ambiguous in RhinoCommon's public pair-filleting method; pick a nearby point
on the arc when comparing outputs.

The earlier public geometry-API [nine-case fixture](../../tools/rhino_oracle/fixtures/curve_fillet_pair.json)
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

The [separate-output fixture](../../tools/rhino_oracle/fixtures/curve_fillet_pair_parts.json)
and [saved response](../../tools/rhino_oracle/observations/curve_fillet_pair_parts.json)
cover `Join` and `Trim` combinations, including zero radius and reversed picks.

The [nonmeeting curve fixture](../../tools/rhino_oracle/fixtures/curve_fillet_nonmeeting.json)
and [saved Rhino response](../../tools/rhino_oracle/observations/curve_fillet_nonmeeting.json)
compare arc/line and NURBS/line selections in both orders, plus a larger
arc/line fillet that trims into the original arc. Across 65
equal-length stations per case, the largest coordinate difference in a live
comparison was below `2.3e-8`. The arc cases alone were below `3.3e-15`.
