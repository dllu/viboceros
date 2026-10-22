# UV maps and surface pullback captures

[Historical checkpoint](../../validation-checkpoint.md)

The preceding standalone Python run reproduces nine original untrimmed CreateUVCrv
surface/input definitions. The Rust command replay additionally reconstructs
the two trimmed sources from their original UV curves. A create/apply roundtrip
recovers the original spatial curve within `1e-6`; application tests exercise
surface-first picking, optional point selection, preselection, cancellation and
World-XY output under a rotated construction plane.

The preceding 11-recipe native capture ran on private Xvfb with settings scheme
`VibocerosOracleCreateUVFinal20261006`, creating 17 UV curves and four points.
The [raw records](../../../tools/rhino_oracle/observations/create_uv_curves_command.json)
retain full inputs, trim curves, isocurve-length diagnostics, output stations,
properties, groups, selection and independent history states. The native coarse
rectangle size differs from accurate integration by about `3.00e-4` for the
radius-two cylinder. The off-surface curve's maximum paired discrepancy is about
`0.00686`; its endpoint also differs from the local closest-parameter projection.
These differences remain explicit in [provenance](../../create-uv-curves-provenance.json)
and [scope documentation](../../commands/create-uv-curves.md). Replay uses `1e-6` for
ordinary cases, `1e-3` for native primitive sizing differences and `0.01` for that
off-surface projection case. This does not establish full native sizing or
projection parity, or a continuous global nearest-locus certificate.

The preceding standalone Python command run reproduces all 13 original surface/input
fixtures, producing 17 curves and nine points. Undo and Redo replay outputs and
retain copied group definitions, including empty definitions for point-only
input. The two degenerate mapping rectangles produce no geometry or Undo entry;
`Undo` then reports nothing to undo and clears picking. Additional document tests
prove that retaining accepted group definitions never retains groups after a
failed transaction's rollback.

The preceding 13-recipe Rhino 8.32.26160.13001 capture ran on private Xvfb with settings
scheme `VibocerosOracleApplyUVPurity20261006`. Every recipe starts at idle in an
empty owned document with independently cleared Undo history. The
[raw records](../../../tools/rhino_oracle/observations/apply_uv_curves_command.json)
retain full source and target definitions before/after the command, command-end
events, 561 output curve stations, nine output points, properties, selection,
active group definitions and Undo/Redo states. Source and target geometry and
attributes remain unchanged. Rust replay compares the complete set of output
stations at `1e-6`, checks properties/group structure and exercises the actual
document adapter. Application tests verify source/target picking, preselection,
group filtering, aliases, cancellation and World-XY mapping under a rotated
construction plane. See [provenance](../../apply-uv-curves-provenance.json).

The preceding standalone Python pushup run certified images of all 13 original UV sources,
retaining their original domains. Eight diagnostic images have zero certified
error; the five curved primitive images have complete bounds at most `3.501e-16`,
below their requested limit `1e-6`. The Rust replay checks 1,677 returned-curve
stations against native surface-image witnesses and independently recomputes
every assembled result's continuous certificate.

The preceding 13-recipe Rhino 8.32.26160.13001 capture ran on private Xvfb with settings
scheme `VibocerosOraclePushup20261006`. Its
[raw records](../../../tools/rhino_oracle/observations/surface_pushup_certified.json)
retain 1,677 source/image stations, all source-purity agreements, five successful
native `Surface.Pushup` operations, and the sphere-seam parameter-speed
discrepancy. The other eight sources retain diagnostic reference curves. See
[provenance](../../certified-surface-pushup-provenance.json) for source hashes and
contract boundaries. That kernel API now backs the `ApplyCrv` / `ApplyCurves`
document workflow; `Pushup` is the SDK operation name.

The preceding standalone Python run certified all ten nonlinear singular-endpoint
source definitions without endpoint constraints, retaining each original
spatial domain. The polynomial recipes report bounds below `9e-16`; the
square-root boundary recipes refine to 103 controls with complete bounds about
`9.87e-7`, below their input limit `1e-6`.

The preceding ten-recipe Rhino 8.32.26160.13001 capture ran on private Xvfb with settings
scheme `VibocerosOracleInterpolatedPullbackFinal_20261006`. Its
[raw records](../../../tools/rhino_oracle/observations/surface_pullback_interpolation.json)
retain 1,290 source/image stations and ten source-purity agreements. Eight native
pullbacks succeed geometrically with different parameter speeds; both singular
cubic directions return no native pullback. The retained local before-change
audit failed on all ten recipes. See
[provenance](../../derivative-free-pullback-provenance.json) for source hashes and
contract boundaries.

The preceding standalone Python pullback run certified all 24 native sources without
endpoint constraints, returning two UV controls and each original spatial
parameter domain. All reported continuous bounds were below `1e-12`, with a
maximum of `8.86e-16`. The run used 16 iterations per source, retaining the
successful bound without recomputing its certificate.

The preceding 24-recipe Rhino capture ran on private Xvfb with settings
scheme `VibocerosOracleLinearPullbackFinal_20261006`. Its
[raw records](../../../tools/rhino_oracle/observations/surface_pullback_linear.json)
retain full source definitions, source purity, 129 stations per recipe, 20
successful native pullbacks, four native failures, and a parameter-speed
discrepancy for both directions of the swapped two-pole surface. The local
certificates and native geometric fitting have different contracts; see
[provenance](../../automatic-surface-pullback-provenance.json) and
[timing details](../../automatic-surface-pullback-performance.json).

The earlier eight-recipe capture also ran on private Xvfb with settings scheme
`VibocerosOraclePullbackEndpointsVerify_20261006`, using the eight
[`surface_pullback_endpoints` recipes](../../../tools/rhino_oracle/fixtures/surface_pullback_endpoints.json).
Its complete output was identical to the
[retained capture](../../../tools/rhino_oracle/observations/surface_pullback_endpoints.json),
including the planar parameter-speed differences and independent endpoint
witnesses. Both files had SHA-256
`71593c8aa90b81679ed1a196a5a25fd9738ecd9febdc71cd237934a3e0e436a0`.
The owned Rhino, Python worker, and Xvfb processes exited after the capture.
