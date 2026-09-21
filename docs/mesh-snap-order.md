# Mesh snap competition and calibrated replay

[Point-snap calibration](point-snaps.md) · [Mesh snaps](mesh-snaps.md) ·
[Provenance](mesh-snap-order-provenance.json)

Mesh Near's per-wire target and its choice of wire are separate questions.
The calibrated per-wire calculation remains supported by the new observations,
but minimum screen distance does **not** reproduce Rhino's wire selection.
This follow-up does not change native selection or claim to resolve it.

## Evidence

The [inputs](../tools/rhino_oracle/fixtures/mesh_snap_order.json) and
[complete observations](../tools/rhino_oracle/observations/mesh_snap_order.json)
retain 84 actual GetPoint results from newly owned private-Xvfb Rhino sessions:

- 48 competition cases: eight vertex-order/winding variants at two offsets,
  four offsets at each corner, and eight offsets around parallel wires in each
  of Top and Perspective views.
- 24 repetitions of the eight competing order variants, including reversed
  and rotated case sequences, with newly created source objects.
- 12 repeated cases with additional public picking diagnostics.

The probe now records the source's public `Mesh.TopologyEdges.EdgeLine(i)` list,
alongside the actual `PointOnObject()` component index. This avoids guessing
which wire an index denotes. The bounded probe validates finite wire coordinates
and still disposes/restores owned sources, view and settings on failures.
The [public topology API](https://developer.rhino3d.com/api/RhinoCommon/html/T_Rhino_Geometry_Collections_MeshTopologyEdgeList.htm)
was also checked in the installed Rhino 8 documentation.

All 84 observed targets agree within `1e-9` with the independent mathematical
calculation **on the observed wire**. That is a per-wire test, not a selection
parity claim. All 24 order repetitions reproduce the original point and component.

Concrete counterexamples:

- `order-0-0--10` selects an adjacent wire endpoint about 10.517 pixels away,
  despite another admitted wire offering a target about 0.030 pixels away.
- `order-0-1--10` changes only the cyclic raw vertex order and selects the
  near-screen target instead. The camera, click and geometric boundary are
  identical. All three fresh repetitions preserve this change.
- `parallel-top-10` selects the first long edge about 9.045 pixels away even
  though the parallel edge is about 7.545 pixels away.
- Perspective parallel-wire cases select the other long edge, including
  cases where it is farther from the cursor. Thus “always choose the first
  topology edge” also fails.

## Picking is not snapping

Optional `pick_diagnostics: true` uses public
[PickContext](https://developer.rhino3d.com/api/rhinocommon/rhino.input.custom.pickcontext)
line and wireframe-mesh queries, with the calibrated click's square aperture.
It records the pick transform, line parameters, depths/distances and whole-mesh
hit point/flag/index. These calls are read-only and run after the actual GetPoint
result; they do not supply the snap's expected target. The context is disposed
on success and all tested failure paths.

The public per-line picking values agree across reversed endpoint orientations,
yet actual mesh snapping can change with vertex order. Ordinary mesh picking
also selects a different wire from snapping in the corner and parallel-wire
counterexamples. Its returned perspective hit point can differ even when the
wire index agrees. Substituting ordinary object picking for snapping therefore
does not resolve these discrepancies. A general snap-selection rule still needs
independent evidence; no proprietary implementation was inspected.

## Reusable native replay

The native oracle operation `projected_object_snap` accepts 1–16 bounded line or
mesh sources, a `camera` containing a 4×4 `world_to_screen` matrix plus `location`
and `direction`, a two-coordinate `cursor`, `capture_radius` in `[1,64]`, `modes`
and `snap_to_meshes`. It calls the application's drafting cache and returns the
3D `point`, Rhino-style `kind` label and zero-based `source` index. Inputs and
source geometry are validated; invalid inputs are errors, not successful misses.
It is an untimed, single-iteration diagnostic, not a benchmark.

`tools.rhino_oracle.point_snap_replay.prepare(request, observations)` validates
the recorded camera/aim/click, exact input source geometry and restored settings,
then builds native inputs using **no observed target coordinates**.
`replay(...)` executes those inputs and compares all three coordinates, kind
and owner at componentwise absolute `1e-9`, with no relative epsilon.
Non-admission compares `None`/null source/null point; it intentionally makes no
claim about the unrelated unsnapped CPlane placement. Raw observations are never
rewritten. Error responses remain errors, and mismatches produce exit status 1.

```sh
python3 -m tools.rhino_oracle.references.mesh_snap_order --all
python3 -m tools.rhino_oracle.point_snap_replay tools/rhino_oracle/fixtures/point_snaps.json tools/rhino_oracle/observations/point_snaps.json
python3 -m tools.rhino_oracle.point_snap_replay tools/rhino_oracle/fixtures/mesh_snap_order.json tools/rhino_oracle/observations/mesh_snap_order.json
python3 -m unittest tools.rhino_oracle.test_point_snaps tools.rhino_oracle.test_point_snap_replay
cargo test --release -p viboceros-oracle projected_object_snap
```

The replay commands are expected to fail parity while the documented differences
remain: the original corpus reports 98/101 matches (90 captures and eight
non-admissions), while the competition-and-repeat batch reports 35/84 matches.
The latter comprises 29/48 discovery, 3/24 order-repeat and 3/12 picking-repeat
matches; the repeated differences are not additional unique counterexamples.
`--emit-native` prints the calibrated native request without running
either engine. Generator stages are selected with no flag, `--repeat`, or
`--picking`; the owned `run_headless.sh rhino REQUEST --timeout 600` runner can
capture them again. Neither replay nor these captures claim history, occlusion,
near/far viewport clipping, arbitrary-scene parity or a Rhino speed advantage.

Validation checkpoint: 3,142 release workspace tests pass (29 ignored), along
with all 322 Python oracle tests, warning-denied Clippy/rustdoc, formatting and
whitespace checks. Both real replay CLI runs return the expected parity-failure
status and counts above; passing harness tests do not turn those failures into
Rhino compatibility passes.
