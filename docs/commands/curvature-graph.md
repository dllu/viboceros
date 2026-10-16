# CurvatureGraph and CurvatureGraphOff

[Command index](README.md)

`CurvatureGraph` displays persistent curvature combs on selected curves,
surfaces and polysurfaces. Without preselection, select objects and press Enter.
The panel and graphs remain visible while drawing or editing other objects.
`CurvatureGraphOff`, or closing the panel, removes the display. These commands
do not create geometry or change model undo/redo. Accepted source edits regenerate
its graph; deleted sources disappear and hidden sources are not painted.

```text
CurvatureGraph Scale=100 Density=1
CurvatureGraph Scale=102 Color=180,60,220
CurvatureGraph Count=3 U=Yes V=No
CurvatureGraph Add
CurvatureGraph Remove
CurvatureGraphOff
```

[ExtractCurvatureGraph](extract-curvature-graph.md) creates model polylines from
selected curve outlines, leaving the sources and analysis display intact.

Use the panel to change scale, density, surface count, U/V visibility and the
three hair colors. Scale and color edits reuse cached source differentials and
do not interrupt an active drawing prompt. Add/Remove use ordinary object
selection. Canceling those getters retains the previous display.

Hair tips are `point - curvature_vector * 2^((Scale-100)/2)`, pointing away
from the local center of curvature. The default scale 100 is 1:1; increasing it
by two doubles hair length. This rule is checked against actual Rhino graph
extraction at scales 50, 98, 100, 102 and 200.

Each nonzero NURBS knot span has `8 * 2^Density` sampling intervals, including
separate one-sided endpoint evaluations. Density defaults to 1, with range
0–10. Separate strips show curvature jumps without drawing a false connecting
line. Zero-speed samples break the strip and are counted in the panel; flat
strips have no visible comb. This is sampled visualization, not a proof of
bounded curvature between the samples.

Surface `Count=2..64` (default 3) places that many evenly spaced stations,
including both boundaries, in each enabled direction. U hairs follow curves
varying in U; V hairs follow curves varying in V. Existing exact isocurve and
trim extraction keeps combs out of face holes. Native builds use usd-brep for
isocurve and NURBS trim production. Source geometry remains unedited.

Sampling has a 200,000-station limit, and nonrepresentable display arithmetic
rejects. Invalid options or failed source additions retain the previous accepted
settings and sources. Scale is limited to 0–300. Color channels are 0–255.

[Rhino's reference](https://docs.mcneel.com/rhino/8/help/en-us/commands/curvature.htm)
also describes graph display on SubDs, which Viboceros does not yet represent.
Rhino's tessellation, density placement, surface count/trim behavior, default
colors and saved analysis metadata are not fully reproduced. Transient transform
or grip previews keep the previous comb until the source edit is accepted. The retained
[native comparison](../../tools/rhino_oracle/observations/curvature_graph_comparison_20261016.json)
checks 153 point/curvature pairs and circle graph envelopes; it does not qualify
complete graph geometry or native surface display parity. The source fixture's
case named `cubic` is a rational quadratic, as its explicit degree records.
See the [validation record](../curvature-graph-validation.json) for source hashes,
test commands and retained native captures.
