# Near object snapping

[Snap controls](object-snap-controls.md) · [Provenance and hashes](near-snaps-provenance.json)

Near captures curve points in screen space, preserving their model-space height.
It supports lines, polylines, circles, arcs, ellipses, NURBS, polycurve leaves,
natural surface boundaries and spatial B-rep edges. [Mesh wire Near](mesh-snaps.md)
is separately opt-in and uses its own calibrated depth weighting; mesh corner-edge
priorities remain incomplete. Curve Near's screen-Euclidean calculation is unchanged.
Enable it in **Snap modes**, right-click to isolate/restore it, or Shift-click
for one point. At a point prompt, `Near`/`Nearest` selects a one-shot override.
Near is off by default: the default five landmark modes remain Point/End/Mid/Cen/Quad.

The [16 requests](../tools/rhino_oracle/fixtures/near_snaps.json) and
[complete observations](../tools/rhino_oracle/observations/near_snaps.json) record
real SplitEdge picks and Undo/Redo in an owned private Xvfb running Rhino
8.32.26160.13001. All 16 now have native snap-derived command/history replay.

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
unchanged. Native targets computed from the source, camera and click drive
complete ordered geometry, attributes, selection and history comparisons at
absolute `1e-9`, relative `1e-10`. The replay checks the captured source object and
the three competing landmark kinds; it does not use observed positions as input.

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

## Implementation and limits

`object_snap/near` supplies targets through the existing shared cache and capture
query, so ordinary and edge-constrained prompts use the same behavior. Mid and
Center cache slots remain lazy during Near-only queries. Shared geometry snapshots
retain constant-time unchanged-source checks; edits, Undo, deletion and conversion
invalidate the relevant data. Hidden objects/layers are excluded; locked objects
remain eligible.

On one source, a captured Point/End/Mid/Quad landmark suppresses Near, and Near
suppresses hover Center. Different objects still compete by capture distance.
The four retained priority witnesses are single-source-curve cases, not a proof
of all Rhino inter-object priority rules. Adding Near to Mid also means Mid is no
longer the only mode, so the special Mid-only whole-segment hover does not apply.

The shared [straight-line projection](projected-line-snaps.md) module recovers
the model fraction from the one-dimensional projectivity.
An adaptive interior station avoids losing a large camera-depth ratio at the
midpoint; reversing the line avoids forming `1 - tiny_fraction`. Parallel
axis-aligned queries directly interpolate the affine fraction and retain local
cursor precision. Independent line tests cover both endpoint orders and depth
ratios through `1e100`. A line with one projectable endpoint resolves its clipped
interval by model-coordinate bisection before the direct solution. Numerically
unresolved projections retain the bounded visible-locus fallback below.

Curved targets use analytic derivatives or the kernel's
[fractional first derivatives](curve-fractional-derivatives.md). The projector
must be affine/projective in its visible half-space and reject clipped points.
Projecting a finite model tangent line determines its oriented screen tangent;
this is not a finite difference of the curve. Parallel axis metrics project the
derivative directly. A span search takes 64 intervals and bisects nonpositive-to-nonnegative
distance-derivative brackets for up to 96 steps, stopping at floating-point
stagnation. Endpoints and samples remain candidates. Roots take precedence over
unrefined samples with indistinguishable rounded distances. Common-sign degree-one
NURBS spans use their actual line locus instead. Sided sampling does not bridge
knot jumps, and mixed weights disable control-hull culling. Stationary endpoints
do not prevent refinement into their adjacent interval.

This is **not a certified global closest-point solver**. Highly oscillatory spans,
stationary/cusp configurations, narrow visible slivers and unresolved projections
can evade the sampling/bracketing scheme. Extreme projective/coordinate roundoff
is not universally bounded by the retained fixture epsilon. Full Mesh Near parity,
occlusion, CPlane-projected snaps, broader priority behavior and scene spatial indexing remain
unfinished. Work scales with scene/span complexity, not a guaranteed frame budget.

Native tests cover the supported geometry families, nonzero-distance conic
targets, extreme knot domains, mixed-weight branches, clipping, discontinuities,
large-origin relative queries and cache lifecycle. All four viewport types test
ordinary and edge-constrained prompts. Real egui press/release tests preserve
off-CPlane height without selecting geometry, and actual menu clicks exercise
the Near one-shot override without losing partially typed command input.

## Validation and timing checkpoint

The initial native checkpoint (`ca54e32`) passed 3,119 ordinary release-workspace
tests (29 opt-in tests ignored), all 298 Python tests, and all seven opt-in offscreen
GPU tests on NVIDIA GB10/Vulkan. The ordinary suite includes all 16 calibrated
Near history replays. Formatting, whitespace, strict Clippy and strict rustdoc
checks also passed. The subsequent [straight-line audit](projected-line-snaps.md)
adds clipped-line and asymmetric-endpoint regressions.

The opt-in `near_scene_timing` diagnostic measures 100 warm cached Near-only
queries after 10 warm-up queries, with an identity XY projector and spatially
separated objects. One object is near the cursor; the others exercise bounds
rejection and scene traversal. A local release run measured:

| Scene objects | Lines, microseconds/query | Quadratic NURBS, microseconds/query |
| ---: | ---: | ---: |
| 1 | 0.096 | 12.121 |
| 100 | 2.613 | 20.223 |
| 1,000 | 29.149 | 105.146 |

These are diagnostic samples, not a statistical benchmark, frame-rate claim or
Rhino speed comparison. Overlapping curves, many spans, exact-arithmetic fallback
and perspective/clipped projections can cost substantially more; scene traversal
still grows with object count.

## Reproduction

The generator does not read either engine's output:

```sh
python3 -m tools.rhino_oracle.references.near_snaps
python3 -m unittest tools.rhino_oracle.test_near_snaps tools.rhino_oracle.test_split_edge_snaps
cargo test --release -p viboceros-drafting object_snap::near
cargo test --release -p viboceros-oracle calibrated_near
cargo test --release -p viboceros near
cargo test --release -p viboceros-drafting near_scene_timing -- --ignored --nocapture
```

For a live run, use the generator's `--artifact /owned/box.3dm` option with a
verified box artifact matching the source recipe, then the owned
`tools/rhino_oracle/run_headless.sh rhino REQUEST --timeout 600` runner.
The Rust adapter recognizes Near but rejects raw uncalibrated aims as resolved
targets. The calibrated test above computes the actual snaps first and hands
their model coordinates to the command adapter as resolved point inputs.
Generic raw replay cannot infer missing camera data from the aim hypothesis.

Initial calibration checkpoint (`cdb3c9a`): all 298 Python tests passed, including
13 focused Near reference/probe tests, along with formatting and whitespace
checks. No native Rust code changed at that checkpoint.
