# Near snap calibration (implementation pending)

[Snap controls](object-snap-controls.md) · [Provenance and hashes](near-snaps-provenance.json)

The [16 requests](../tools/rhino_oracle/fixtures/near_snaps.json) and
[complete observations](../tools/rhino_oracle/observations/near_snaps.json) record
real SplitEdge picks and Undo/Redo in an owned private Xvfb running Rhino
8.32.26160.13001. Near is **not yet implemented** in Viboceros's snapping UI;
this is a reference corpus, not a native feature or command-replay claim.

Six families test one-shot Near (overriding five persistent modes) and persistent
Near-only: planar line, elevated line, tilted line, polyline, circle and rational
quadratic ellipse quarter. Four mixed-mode cases probe Near against End, Mid,
Center and Quad on one curve. These observed End/Mid/Quad targets beat closer
Near positions within the aperture; Near beats Center while hovering on the circle.
This does not establish a universal priority order across objects or geometries.

Each point prompt records the actual world-to-screen matrix, camera, world aim,
WorldToClient result, integer click and viewport size. Matrix projection agrees
with WorldToClient within `1e-7` pixels. Input `pick.point` is only an aim
hypothesis: pixel rounding and intentional offsets mean it is not the snapped
target and must not be used as expected snap output.

## Independent reference checks

[`references/projected_near.py`](../tools/rhino_oracle/references/projected_near.py)
uses only source geometry and the recorded camera/click. For lines, the nearest
screen fraction `s` converts back to model fraction
`t = s*w0 / ((1-s)*w1 + s*w0)`, where `w0,w1` are endpoint homogeneous depths.
Using `s` directly to interpolate world endpoints is incorrect in perspective.
Polyline references compare the actual segments independently.

Circle and single quadratic Bezier references use analytic derivatives and
bracket stationary points of squared screen distance, avoiding the positional
precision loss from minimizing rounded distance values alone. This bounded
reference search is not a certified general closest-point solver.

Thirteen Near targets and three competing discrete features predict the observed
split coordinate within absolute `1e-9`. Line/polyline and circle discrepancies
are below `1e-12`; the ellipse-quarter discrepancies
are about `9.491e-10`, close to that absolute threshold. No results are rounded
or adjusted to match. One-shot and persistent pairs agree within `1e-12`.

SplitEdge projects the target onto the box's x-axis edge. Consequently these
observations establish **only the target's constrained x coordinate**, not its
unconstrained 3D position. Independent analytic tests additionally check full
3D line/circle reference results, but are not Rhino measurements. Complete
geometry, attributes, selection and history are retained; all 16 actual Undo/Redo
cycles restore their respective before/after states and leave snap-source objects
unchanged. Native snap-driven history replay is still pending.

The preliminary run failed because the polyline fixture used `points` instead of
`vertices`; the circle also lacked `x_axis`. The successful run followed both
schema corrections. The failed diagnostic is separately identified in provenance,
not presented as a successful calibration.

An intermediate successful run revealed an ambiguous End control: integer pixel
rounding made Near choose the endpoint too. The End aim was moved along the line,
and the Quad aim moved farther along the circle for a clearer coordinate witness.
Only the subsequent fresh complete 16-case run is retained. Regression checks
require all three discrete targets to differ from Near and be farther from the
cursor, while both targets remain within the 12-pixel aperture.

## Reproduction and next steps

The generator does not read either engine's output:

```sh
python3 -m tools.rhino_oracle.references.near_snaps
python3 -m unittest tools.rhino_oracle.test_near_snaps tools.rhino_oracle.test_split_edge_snaps
```

For a live run, use the generator's `--artifact /owned/box.3dm` option with a
verified box artifact matching the source recipe, then the owned
`tools/rhino_oracle/run_headless.sh rhino REQUEST --timeout 600` runner.
The portable fixture is currently Rhino-only: the Rust adapter does not yet
accept Near, and generic native replay cannot derive targets from the aims.

Next work is the native projective closest-point query, cache/UI mode integration,
and snap-derived ordered command/history replay. Camera-plane crossings,
discontinuous and mixed-sign rational curves, meshes, broad priority behavior and
performance still require separate implementation and evidence. No global
closest-point, full Rhino parity or cross-engine performance claim follows here.

The kernel now has [fractional first derivatives](curve-fractional-derivatives.md)
for that query, including sided and extreme-domain NURBS sampling. This is a
numerical prerequisite, not yet native Near capture.

Initial calibration checkpoint (`cdb3c9a`): all 298 Python tests passed, including
13 focused Near reference/probe tests, along with formatting and whitespace
checks. No native Rust code changed at that checkpoint.
