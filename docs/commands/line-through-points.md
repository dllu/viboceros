# LineThroughPt

`LineThroughPt` fits one straight line through selected point objects, point
clouds, whole meshes, or control/edit grips. Select sources first, or start the
command and pick sources before pressing Enter. Enable curve/surface controls
or mesh vertex grips with `PointsOn`. `SelAll`, `SelPt` and `InvertPt` share the
existing point-selection flow. Escape cancels without creating geometry.

The fit minimizes the sum of squared orthogonal point distances. Every point
occurrence has equal weight, including duplicates and unreferenced mesh
vertices. A selected mesh or cloud contributes its complete point array;
selected grips contribute Euclidean positions independently of NURBS weights.
The segment spans the extrema of the point projections onto the fitted line.
Its domain measures the distance between its stored endpoints.

```text
Point 0,0,0
Point 2,0.2,0
Point 4,0,0
SelPt
LineThroughPt
```

Sources and attributes stay intact. The output uses default attributes on the
current layer and starts unselected. Preselection remains selected; objects
picked after starting the command are deselected on acceptance. Creation is
one Undo/Redo edit. Coincident inputs finish successfully without creating a
line or a history step. Fewer than two points rejects. Short representable fits
are retained, including the captured `1e-9` line at model tolerance `1e-6`.

The kernel shares the [plane fitter's](plane-through-points.md) exactly centered,
isotropically scaled thin SVD. Exact collinear inputs retain their exact
supporting line, including extreme-range coordinates and subnormal lengths.
Exact scalar products select projected extrema, and only the two selected
endpoints are rounded. The existing one-million-point fitting cap applies;
unresolved numerical ranges and non-finite output reject.

No second eigensolver is introduced. The available usd-brep
`N_LineFit3dPts` producer forms a covariance-derived normal matrix; this path
instead reuses the established centered thin SVD and exact projector. Repeated
largest singular values admit several optimal lines. General ambiguous fits,
directed endpoint parity and performance relative to Rhino remain unqualified.

The [retained combined fixture](../../tools/rhino_oracle/fixtures/point_fitting.json)
contains 25 new line commands and SDK fits plus 17 plane regressions. Native
captures cover meshes, point clouds, grips, duplicate weights, permutations,
noisy planar/spatial data, a remote origin, a square with multiple optimal
directions, degenerate selections and short curves. Tests admit both endpoints
with the exact point-distance predicate under an explicit forward or reversed
unit chart. Convexity bounds the complete paired segment images by `1e-6`.
The plane regressions retain their complete bilinear image checks.

All 21 retained line outputs pass that whole-image gate, with a largest paired
endpoint distance of `4.66e-10`. Ten line reports retain opposite endpoint
directions. The [comparison record](../../tools/rhino_oracle/observations/point_fitting_comparison_20261016.json)
keeps those raw differences alongside all 35 command-image certificates and
21 SDK-line image certificates.

The `line_through_points` Python/Rust oracle operation accepts `points`, `source`
(`points`, `cloud`, `mesh`, `mesh_grips`, `curve_grips`) and `postselect`. Its
response retains complete curve definitions, endpoints, source selection,
output attributes and SDK fit observations. Raw reports preserve direction
differences instead of reversing the recorded geometry.

See [McNeel's command reference](https://docs.mcneel.com/rhino/8mac/help/en-us/commands/line.htm),
the [native captures](../../tools/rhino_oracle/observations/point_fitting_rhino_20261016.json),
and the [validation record](../line-through-points-validation.json).
