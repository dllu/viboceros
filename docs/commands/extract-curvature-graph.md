# ExtractCurvatureGraph

[Command index](README.md)

Turn on [CurvatureGraph](curvature-graph.md) for curves, then enter
`ExtractCurvatureGraph` to create model polylines from their displayed graph
outlines. Hairs are excluded. Without eligible preselection, pick graphed curves
and press Enter. Ungraphed curves, surfaces and polysurfaces are excluded.
Empty Enter or Escape cancels the picker.

```text
CurvatureGraph Scale=100 Density=1
ExtractCurvatureGraph
Undo
Redo
```

Outputs use default attributes on the current layer. They remain unselected and
do not inherit source names, colors, groups or user text. Preselected sources
retain selection, including other preselected objects; command-first selection
is cleared after extraction. Sources and the persistent graph display remain.
All output construction precedes insertion, and one Undo removes the extraction.
Flat inputs create no outline and preserve the existing undo/redo history.

Smooth neighboring knot spans join when their source positions, tangents and
tip positions agree within model tolerances. Kinks, curvature jumps and omitted
singular samples remain separate. Closed curves may join across their original
seam; discontinuous components stay open. Consecutive exact duplicate vertices
are removed, while short representable segments retain their geometry.

The command extracts Viboceros's current sampled display. Rhino's graph
tessellation differs, so matching native output counts and endpoints does not
establish matching polyline geometry. Native command, attribute, selection and
geometry comparisons are retained in the [comparison](../../tools/rhino_oracle/observations/extract_curvature_graph_comparison_20261016.json).
The bidirectional vertex-to-polyline distance diagnostic exposes that remaining
sampling gap; it is not a continuous Hausdorff certificate.

Native nested-script `_Undo` reports “Nothing to undo.” Those observations do
not qualify native history parity. Local tests independently verify one-step
undo/redo and read-only no-result behavior. See the [validation record](../extract-curvature-graph-validation.json).

[Rhino's reference](https://docs.mcneel.com/rhino/8/help/en-us/commands/extractcurvaturegraph.htm)
describes extracting the outline of curves with curvature graph enabled and
splitting at discontinuities.
