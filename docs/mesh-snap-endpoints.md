# Mesh Near endpoint capture

[Square apertures](snap-capture-box.md) · [Mesh competition](mesh-snap-order.md) ·
[Provenance](mesh-snap-endpoints-provenance.json)

Mesh Near now distinguishes three cases for a fully visible wire. When both
original endpoints are inside the square aperture, it selects the screen-nearest
endpoint, retaining the first endpoint on exact distance ties. With exactly one
endpoint inside, it uses ordinary screen-nearest curve proximity. With neither
inside, it retains the calibrated endpoint-depth weighting described in
[point-snap calibration](point-snaps.md). Ordinary curve Near is unchanged.

This fixes the eight short-wire targets exposed by the aperture corpus. Its
native replay improves from 115/128 to **123/128**, still with five unresolved
competing-wire selections. A synthesized clipping-plane point does not qualify
as an original mesh endpoint for the new branch; clipped-wire parity remains
unverified.

## Evidence and input controls

Three newly owned private-Xvfb Rhino 8.32.26160.13001 sessions retain all 272
GetPoint observations, including 196 captures and 76 non-admissions:

- [80 order/aperture controls](../tools/rhino_oracle/fixtures/mesh_snap_endpoints.json):
  the same short quad in Top/Perspective, eight cyclic/winding variants and
  apertures 12/16/24/32, plus separate line controls.
  [Complete observations](../tools/rhino_oracle/observations/mesh_snap_endpoints.json).
- [120 timing controls](../tools/rhino_oracle/fixtures/mesh_snap_endpoint_settling.json):
  40 cases repeated with immediate input, 250 ms settling and 750 ms settling.
  [Complete observations](../tools/rhino_oracle/observations/mesh_snap_endpoint_settling.json).
- [72 edge-on triangles](../tools/rhino_oracle/fixtures/mesh_snap_edge_on.json):
  valid 3D triangles whose projections collapse onto a short segment and one
  endpoint, with six vertex permutations, three depth offsets and two apertures.
  [Complete observations](../tools/rhino_oracle/observations/mesh_snap_edge_on.json).

The timing controls rule out immediate-click timing as the explanation for these
cases: all 120 reproduce the original point, kind, owner, component, camera,
public picking diagnostics and unchanged sources exactly. The optional bounded
`input_settle_ms` field accepts integers 0–1000. Positive values make one
one-pixel detour, return to the calibrated pixel, then defer the click without
blocking the host polling loop. The owned window/target must remain the same.
Abort markers suppress pending clicks; failed input is never acknowledged.
Host `input_motion` metadata records the requested and observed minimum interval
separately from Rhino's result. Replay validates it but never supplies it to the
native geometric calculation. Existing requests retain immediate input.

The public
[PickFrustumTest line overload](https://mcneel.github.io/rhinocommon-api-docs/api/RhinoCommon/html/M_Rhino_Input_Custom_PickContext_PickFrustumTest_2.htm)
provides a parameter independently of GetPoint's returned target. Across the
80 order controls, 72 triangle controls and the earlier 12 picking diagnostics,
all **520 per-line admission/target checks** agree with the corrected independent
rational reference and native implementation: 300 hits and 220 misses. The
36 screen-distance ties preserve endpoint order despite different depths.
All captured mesh GetPoint targets also agree with this formula on their
reported component in the new corpora. Whole-mesh picking remains distinct;
its screen-nearest interior target is not a replacement for mesh Near.

No proprietary implementation was inspected. The model is inferred from public
API outputs; the public signatures do not specify this endpoint algorithm.
The existing 630-case mathematical CSV was regenerated from the corrected
source-only reference, changing 96 expected targets. Its previous digest is
retained in the original provenance; raw Rhino observations were not rewritten.

## Remaining selection differences

Per-wire correctness does not settle which wire wins inside a mesh. Reordering
the same quad can select a farther endpoint. Global minimum screen distance,
nearest endpoint, and ordinary whole-mesh picking do not reproduce all cases.
Native full-target replay is 59/80 for order controls, 93/120 for timing controls,
and 58/72 for edge-on triangles. All admission decisions and kinds/owners agree;
the remaining target differences still produce parity-failure exit status 1.
The earlier corpora remain at 98/101, 35/84 and 65/72 matches respectively.
No wire-selection heuristic was added to fit these outputs.

```sh
python3 -m tools.rhino_oracle.references.mesh_snap_endpoints
python3 -m tools.rhino_oracle.references.mesh_snap_endpoints --settled
python3 -m tools.rhino_oracle.references.mesh_snap_endpoints --edge-on
python3 -m tools.rhino_oracle.point_snap_replay tools/rhino_oracle/fixtures/mesh_snap_endpoints.json tools/rhino_oracle/observations/mesh_snap_endpoints.json
python3 -m unittest tools.rhino_oracle.test_mesh_snap_endpoints tools.rhino_oracle.test_point_snap_input
cargo test --release -p viboceros-drafting projected_line
cargo test --release -p viboceros-oracle projected_object_snap
```

Fresh captures use `tools/rhino_oracle/run_headless.sh rhino REQUEST --timeout 600`.
Native inputs contain only source geometry, calibrated camera/click and snap
policy. All target comparisons keep absolute `1e-9` and zero relative epsilon.
These probes do not establish general clipped-wire/occlusion behavior, command
history, arbitrary-scene selection parity or a Rhino performance advantage.

Validation checkpoint: 3,158 release workspace tests pass (29 ignored), along
with all 339 Python tests, warning-denied Clippy/rustdoc, formatting and whitespace
checks. All seven real replay CLI runs retain their documented parity-failure
status and match counts. Original retained observations are unchanged; the three
new request/response pairs also match the raw captures exactly, including float
bits and signed zero.
