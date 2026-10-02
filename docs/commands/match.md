# Match curve ends

[Command reference](README.md) · [Endpoint continuity queries](gcon.md)

Select two open curves, then use `Match` to edit the first curve at the nearest
pair of ends. `Pick1=x,y,z` and `Pick2=x,y,z` choose other ends.

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

When the source has too few controls to preserve the requested opposite end,
Match adds controls before editing. For five-control cubic and four- or
five-control quadratic curvature matches preserving far curvature, it uses a
uniform curve fitted at Greville parameters; other short sources use knot
insertion.

See the [oracle guide](../oracle.md) for retained matching fixtures, numeric
comparisons, and remaining differences, including an average-position boundary
case where Rhino retains a short parameter tail.
