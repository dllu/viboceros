# Connect

[Command reference](README.md) · [Rhino command](https://docs.mcneel.com/rhino/8/help/en-us/commands/connect.htm)

Select two open curves and run `Connect`. The selected ends are trimmed or
extended to meet at one point. Two updated curves are selected, and one Undo
restores the originals. `Join=Yes` creates a single selected polycurve.

The nearest pair of endpoints is used by default. `Pick1=x,y,z` and
`Pick2=x,y,z` choose other ends. Separate outputs inherit their respective
sources' attributes and groups; a joined output inherits the first source's.

Straight terminal segments may meet by extension. Nonmeeting NURBS and
polyline ends can extend with straight tangent segments; this is also available
as `ExtendOtherCurvesBy=Line`. `ExtendOtherCurvesBy=Smooth` continues a NURBS
terminal span to the supporting line of the other curve, or continues two NURBS
terminal spans to their nearest forward intersection. `Join=Yes` joins the two
results. Curved terminal segments are also supported when their selected
endpoints already meet.

For coplanar arc/line pairs, the default `ExtendArcsBy=Arc` trims or extends
the selected arc end on its supporting circle to a valid line intersection.
Coplanar arc pairs can trim or extend their selected ends to a
supporting-circle intersection. `ExtendArcsBy=Line` adds a straight tangent
extension instead. Other mixed curved-pair extensions remain pending.

The [four-case Connect fixture](../../tools/rhino_oracle/fixtures/connect_nurbs_line.json)
and [saved Rhino command output](../../tools/rhino_oracle/observations/connect_nurbs_line.json)
compare straight and smooth NURBS-to-line extensions, including reversed
selection, and a smooth NURBS-to-NURBS extension. Endpoints and NURBS control
points agree within `1e-10`.

```text
Connect ExtendOtherCurvesBy=Smooth Join=Yes
```
