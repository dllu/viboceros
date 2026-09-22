# Unconstrained snap calibration

[Mesh snapping](mesh-snaps.md) · [Curve Near](near-snaps.md) · [Provenance and hashes](point-snaps-provenance.json)

The owned `point_snap` Rhino probe records `GetPoint.Point()`, `OsnapEventType`
and `PointOnObject()`, alongside the actual prompt-time camera and integer click.
Unlike SplitEdge, this measures an unconstrained **3D target**, its snap kind,
and source ownership. These are documented public
[GetPoint properties](https://mcneel.github.io/rhinocommon-api-docs/api/RhinoCommon/html/Properties_T_Rhino_Input_Custom_GetPoint.htm)
and [methods](https://mcneel.github.io/rhinocommon-api-docs/api/RhinoCommon/html/Methods_T_Rhino_Input_Custom_GetPoint.htm),
also verified in the installed Rhino 8 API documentation. No proprietary code
was inspected or decompiled.

The probe requires an empty owned document, accepts only bounded line/mesh input,
and creates/deletes only its own sources. Timers wait for the actual point prompt,
request one scoped click, and fail closed on invalid calibration or incomplete
input. Camera, target, viewport name, snap settings and source resources are
restored/disposed, including tested failure paths. Cleanup failure prevents a
reported successful observation. The host only interacts with its newly owned
Rhino window in a private Xvfb. UI timeouts retain the existing owned-process
cleanup policy; they do not restart a live session.

## Retained evidence

The [101 inputs](../tools/rhino_oracle/fixtures/point_snaps.json) and
[complete observations](../tools/rhino_oracle/observations/point_snaps.json) comprise
24 discovery cases, 24 fresh transformed cases, 41 endpoint-region checks, and
12 fresh aperture checks. They cover Perspective/Top, line/quad/triangulated
sources, Near/Mid/disabled controls, reversed vertices, tilted and axis-swapped
geometry, scales `1/64` and `64`, translation, and apertures 8/10/12/16 pixels.
All are from Rhino 8.32.26160.13001.

Ninety captures agree in full 3D within absolute `1e-9`, including their actual
snap kind and source. Eight cases verify non-admission; their unsnapped CPlane
points are not a native placement-parity claim. All 101 retain unchanged source
geometry and mesh-switch restoration. Four Mid captures distinguish direct mesh
Mid from whole-segment curve Mid without inferring labels from point positions.

Three genuine differences remain in this corpus: `threshold-tilted--10-0`,
`threshold-tilted--9-0` and `aperture-tilted--8-0-r16`. Rhino selects an adjacent
mesh edge's endpoint, whereas native snapping selects a closer projected point
on the hovered edge. The aperture witness records `MeshTopologyEdge` index 1;
the other two were additionally checked by a separately hashed component
diagnostic. Their model-space discrepancies exceed `0.5` units. These are not
roundoff, toleranced matches, or silently skipped assertions: tests check the
independent native target, observed endpoint, source/kind and distance ordering.
The [competition follow-up](mesh-snap-order.md) retains broader wire-selection
counterexamples, repeated vertex-order effects, and a reusable native replay
API. General Rhino mesh wire ranking remains unresolved, not only at corners.

## Measured per-wire calculation

The [square-aperture follow-up](snap-capture-box.md) exposed eight short-wire
counterexamples; the [endpoint follow-up](mesh-snap-endpoints.md) resolves those
with a both-endpoints-inside branch. Competing-wire selection remains unresolved.

Curve Near minimizes ordinary squared screen distance. Mesh Near chooses the
screen-nearest endpoint when both original endpoints are inside the **square**
aperture, retaining the first endpoint on exact distance ties. With exactly one
endpoint inside, it uses ordinary squared screen distance; an
endpoint outside the aperture's circle can still trigger this branch.
Otherwise, the measured mesh target uses a different endpoint-depth weighting.

For the line's homogeneous projection `H(t) = (X(t), Y(t), W(t))`, with cursor
`c`, the reference minimizes

```text
| (X(t), Y(t)) - c * W(t) |² / W(1-t)²,  0 <= t <= 1.
```

Ordinary screen-nearest snapping instead divides by `W(t)²`. This is an
independent mathematical model inferred from outputs, not a claim about Rhino's
internal source code. It was initially contradicted by one of the 24 transformed
cases; the endpoint checks exposed the missing aperture branch. Subsequent
fresh-radius cases confirm that branch and preserve the separate corner-ranking
counterexample.

Let `pa,pb` be cursor-relative screen endpoints and `wa,wb` their homogeneous
depths. The reference takes the nearest point on the virtual segment
`qa = pa*wa²`, `qb = pb*wb²`, then converts its fraction `s` to model fraction
`t = s*wb / ((1-s)*wa + s*wb)`. All intermediate reference arithmetic is exact
`Fraction`; no observed target coordinates enter the calculation.

Native snapping recovers relative depths from a conditioned interior projection
station, since its metric exposes only a camera projection callback. It reverses
and halves stations for asymmetric depths, normalizes before products/dots, and
interpolates from the nearer endpoint fraction. Affine metrics retain the direct
path. A 630-case independent rational corpus covers three camera axes, both
endpoint orders, four aperture sizes, and depth ratios through `1e100`.
The mesh hierarchy is separately checked against exhaustive per-wire queries.

Existing visible-segment clipping and unresolved-projection fallback remain.
The live corpus establishes fully visible wires, not universal clipped-wire,
occlusion, endpoint-priority, extreme-coordinate or arbitrary-scene parity.
No Rhino speed comparison follows from these probes.

This fixes the five previous [SplitEdge Mesh Near](mesh-snaps.md) discrepancies:
all seven retained mesh captures now replay complete ordered geometry,
attributes, selection and actual Undo/Redo with unchanged `1e-9` absolute /
`1e-10` relative epsilon. The historical screen-nearest reference and all raw
coordinates are retained. The new GetPoint probe itself makes no model-history
claim.

Validation checkpoint at `1fd0b8e`: the release workspace passes 3,139 tests (29 ignored);
all seven explicitly enabled GPU raster checks and all 314 Python oracle tests
pass. Workspace Clippy with warnings denied, rustdoc with warnings denied,
formatting and whitespace checks also pass. These checks preserve the three
documented corner-edge differences; they do not establish complete Rhino parity.

## Reproduction

```sh
python3 -m tools.rhino_oracle.references.point_snaps --all
python3 -m tools.rhino_oracle.references.point_snaps --all --observations tools/rhino_oracle/observations/point_snaps.json
python3 -m unittest tools.rhino_oracle.test_point_snaps tools.rhino_oracle.test_mesh_snaps
cargo test --release -p viboceros-drafting object_snap
cargo test --release -p viboceros-oracle unconstrained_snaps
cargo test --release -p viboceros-oracle calibrated_mesh_snaps
```

Run generated requests with the owned `tools/rhino_oracle/run_headless.sh rhino
REQUEST --timeout 600` runner. This is a Rhino diagnostic operation; native
`projected_object_snap` replay uses the recorded camera/click, never an
uncalibrated aim or an observed target as input. The [replay CLI](mesh-snap-order.md#reusable-native-replay)
compares the whole corpus and exits with a parity failure for the three known
differences. Separate generator flags reproduce each stage:
no flag for discovery, `--held-out`, `--threshold`, or `--aperture`.
