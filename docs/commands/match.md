# Match curve ends

[Command reference](README.md) · [Endpoint continuity queries](gcon.md) · [Rhino Match](https://docs.mcneel.com/rhino/8/help/en-us/commands/match.htm)

Run `Match` and pick the first curve near the end to change. Pick the
reference curve near its matching end. The viewport previews the result
while you change options. Press Enter or click **Match** to accept in one
Undo step. `SelID` selects by object ID, and Esc cancels the preview.

At the reference prompt, `MatchTo=PickPoint` uses the location clicked on the
target curve. `SurfaceEdge` asks for a boundary edge and then a point on it.
Pick this point in the viewport or type coordinates. `PerpendicularToEdge`
orients the matched tangent across the edge in the surface tangent plane and
preserves the far-end curvature. Interior and edge targets remain unchanged.
Tangency and curvature on surface edges require a single adjacent face.

For end-to-end matching, `Join=Join` creates a polycurve in the edited
curve's place. `Join=Merge` is available with `Mode=Curvature` and uses a
single NURBS curve with a reduced seam-knot multiplicity. At the options
prompt, `Mode` opens the same choices as `Continuity`. Option changes carry
forward to the next interactive invocation.

The scripted command API also accepts two preselected open curves. It uses
the nearest pair of ends by default. `Pick1=x,y,z` and `Pick2=x,y,z` choose
other ends.

```text
Match Continuity=Tangency PreserveOtherEnd=Position
Match Continuity=Curvature AverageCurves=Yes PreserveOtherEnd=Position
Match Pick1=0,0,0 Pick2=4,0,0 Continuity=Curvature PreserveOtherEnd=Curvature
```

`Continuity=Position|Tangency|Curvature` selects the condition at the matched end.
`PreserveOtherEnd=None|Position|Tangency|Curvature` selects the condition retained
at the opposite end. Single-span and multi-span curves support position, tangent,
and curvature matching; position matching can trim the source endpoint.

`AverageCurves=Yes` changes both curves and keeps the edit in one Undo step.
Average curvature matching can preserve the far position, tangent, or curvature
of both multi-span curves, refining either curve when needed.

Viboceros uses usd-brep trimming, degree elevation, knot
refinement and affine Greville solving for structural preparation. The shared
endpoint and preservation policies remain described below; see
[native qualification](../smlib-curve-match.md).

When the source has too few controls to preserve the requested opposite end,
Match adds controls before editing. For five-control cubic and four- or
five-control quadratic curvature matches preserving far curvature, it uses a
uniform curve fitted at Greville parameters; other short sources use knot
insertion.

See the [oracle guide](../oracle.md) for retained matching fixtures, numeric
comparisons, and remaining differences, including an average-position boundary
case where Rhino retains a short parameter tail.
