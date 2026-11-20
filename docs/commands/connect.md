# Connect

[Command reference](README.md) · [Rhino command](https://docs.mcneel.com/rhino/8/help/en-us/commands/connect.htm)

Run `Connect` and pick two open curves near the ends you want to keep. Moving
over a second curve previews the connection. The second pick applies it in
one Undo step. Separate results retain their object identities. `Join=Yes`
keeps the first curve's identity and replaces the two curves with a polycurve.
The command leaves the results unselected.

`Join`, `ExtendArcsBy` and `ExtendOtherCurvesBy` are available at either
curve prompt. Edits survive cancellation. Use `SelID` followed by an object
ID to select a curve by its ID. `SubCrv` defines a temporary input
using the last portion point as the pick. It preserves the original curve.
When the picked ends cannot connect, the command tries the other ends.

Scripted input also accepts two preselected curves with `Pick1=x,y,z` and
`Pick2=x,y,z` to specify the ends to edit. The scripted default uses the
nearest pair of endpoints. Scripted outputs inherit source attributes and groups.

Straight terminal segments may meet by extension. Nonmeeting NURBS and
polyline ends can extend with straight tangent segments; this is also available
as `ExtendOtherCurvesBy=Line`. `ExtendOtherCurvesBy=Smooth` continues a NURBS
terminal span to the supporting line of the other curve, or continues two NURBS
terminal spans to their nearest forward intersection. `Join=Yes` joins the two
results. Curved terminal segments are also supported when their selected
endpoints already meet.

Viboceros uses
[usd-brep producers](../smlib-curve-connections.md) for smooth NURBS
continuation and retained intervals.

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

Rhino parity for smooth extension of rational NURBS is unresolved. In an
[eight-case Rhino probe](../../tools/rhino_oracle/fixtures/connect_rational_smooth_rhino_only.json),
Rhino either rejected the operation or extended only the line while leaving
the rational source unchanged; the [raw results](../../tools/rhino_oracle/observations/connect_rational_smooth_rhino_only.json)
are retained. Viboceros currently follows the rational curve's mathematical
continuation, so these cases can produce different document geometry.

```text
Connect ExtendOtherCurvesBy=Smooth Join=Yes
```
