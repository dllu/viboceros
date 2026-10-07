# Validation checkpoint

[Architecture and status](architecture.md) · [Rhino oracle](oracle.md)

This is a reproducible regression checkpoint, not a compatibility certificate.
The October 7, 2026 audit tested code at `8fc9a7ba` with Rust 1.95.0 after
[SubCrv cursor direction locking](subcurve-direction.md).

## Commands and results

```sh
cargo test --workspace --release
python3 -m unittest discover -s tools/rhino_oracle -t .
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
git diff --check
```

All commands completed successfully. The ordinary Rust suite passed 5,168 tests:

| Package | Passed | Ignored |
| --- | ---: | ---: |
| App | 966 | 17 |
| Command | 1,157 | 2 |
| Document | 185 | 5 |
| Drafting | 159 | 6 |
| Geometry | 2,020 | 8 |
| I/O | 195 | 0 |
| Oracle | 486 | 0 |

The Python suite passed 908 tests. The 38 ordinarily ignored Rust tests were
not run in this checkpoint. The September 12 audit of `bd299074` separately
passed six opt-in GPU tests covering 182 renders on NVIDIA GB10 / Vulkan /
driver 610.43.02; that is historical evidence, not a new graphics check.
See [GPU tests](gpu-tests.md) for their pixel assertions and limits.

A fresh 18-recipe SubCrv direction workflow ran on private Xvfb under
`VibocerosOracleDirectionVerified20261007`. The
[raw records](../tools/rhino_oracle/observations/subcurve_direction.json)
retain actual motion, locked/unlocked getter prompts, every public
SendKeystrokes input, command events, source/output definitions and independent
Undo/Redo states. Ten commands succeed; four opposite-side open point inputs
end with Failure and four closed numeric inputs with Cancel. App replay checks
330 output curve stations and two markers at `1e-6`, directed endpoints, source
purity, selection and history. The backward seam-crossing polyline numeric case
succeeds; other closed numeric outcomes remain a policy inferred from five
recorded cases, without arbitrary closed-chart parity. Inline open numeric
input shares the policy, with source purity and parent-owned history. A CPU
viewport test confirms that hover reports a location without accepting a point
or editing history. See [scope and provenance](subcurve-direction.md).

A preceding 37-step SubCrv preference workflow ran on private Xvfb in settings
scheme `VibocerosOracleSubcurvePreferenceFinal20261007`. The
[raw records](../tools/rhino_oracle/observations/subcurve_preferences.json)
retain default queries, accepted/cancelled option changes, completed geometry,
global Remember toggles, command events and complete source/result snapshots.
Copy, Mode and FromMidpoint persist immediately, even after cancellation;
SubCrv Copy is not reset by the global RememberCopyOptions switch in this capture.
Hidden Mode prompt fields remain absent. Copy=No queries and completed MarkEnds
steps independently establish their saved value. Application replay checks all
37 steps, visible defaults, loci/marker points at `1e-6`, identity and selection.
Command tests verify omitted values, invalid syntax, registry isolation and
document-history independence. Preferences remain session-only; restart
persistence, direction/edge lifetimes and complete native failure timing are
unverified. See [scope](subcurve-option-memory.md) and
[provenance](subcurve-option-memory-provenance.json).

The preceding 22-recipe FromMidpoint capture ran on private Xvfb with settings scheme
`VibocerosOracleSubcurveMidpointFinal20261007`. Nineteen commands succeed; zero
and half/full-perimeter coincident endpoints remain native cancellations.
The [raw records](../tools/rhino_oracle/observations/subcurve_midpoint.json)
retain original/output curves, stations, marker points, metadata, groups,
selection, events and Undo/Redo. Numeric inputs are half-lengths and finish
immediately; point inputs derive that radius along the source. Open sides clamp
independently, while closed endpoints wrap and keep their forward interval,
including the over-half-perimeter remainder. Command/app replays compare every
locus, directed endpoint and marker at `1e-6`, with metadata and history checks.
They cover curved input, Copy/replacement, endpoint centers, option changes and
MarkEnds. Midpoint policy inherits numerical integration and parameter-resolution
limits; direction locking, live preview, B-rep edges, restart option persistence and
reactive History remain outstanding. See [scope](subcurve-midpoint.md) and
[provenance](subcurve-midpoint-provenance.json).

The preceding 15-recipe SubCrv MarkEnds capture ran on private Xvfb with settings
scheme `VibocerosOracleMarkEnds20261007`. Thirteen commands produce 26 endpoint
markers; zero and missing confirmation remain native cancellations. The
[raw records](../tools/rhino_oracle/observations/subcurve_mark_ends.json) retain
full original curves, point coordinates, attributes, groups, selection, command
events and Undo/Redo. Command and app tests replay all marker coordinates at
`1e-6`, with source purity and default-attribute checks. Both Copy choices produce
ungrouped, unselected current-layer points; a full closed traversal retains two
coincident markers. The app can change Mode during input without edits, and bad
modes or geometry fail atomically. Undo/Redo restore marker IDs and empty
selection. Restart mode persistence and reactive History remain unimplemented; see
[scope](subcurve-mark-ends.md) and [provenance](subcurve-mark-ends-provenance.json).

The preceding 17-recipe standalone SubCrv capture ran on private Xvfb with settings
scheme `VibocerosOracleStandaloneSubcurveFinal20261007`. Fifteen commands succeed;
zero and missing confirmation remain native cancellations. The
[raw records](../tools/rhino_oracle/observations/standalone_subcurve.json) retain
full source/output definitions, 33 stations per curve, names, layers, user text,
groups, selection, events and Undo/Redo. Command and application replays compare
every locus and directed endpoint at `1e-6`, with metadata and history checks.
The app additionally accepts starting without preselection and choosing a source
by viewport or ID. Copy results use the original layer/groups and are selected;
replacement retains identity and clears selection. Standalone open point picks
retain source orientation, and numeric full closed traversals remain geometry.
The inline getter retains its separate full-traversal omission policy.
Explicit `Parameter` edits remain directed mathematical intervals. Direction
locking, B-rep edges and complete Copy memory remain
outstanding. See [scope](commands/subcurve.md) and
[provenance](standalone-subcurve-provenance.json).

The preceding 29-recipe command capture ran on private Xvfb with settings scheme
`VibocerosOracleSubcurveConfirmCommands20261007`. Twenty-eight commands succeed;
the standalone object-ID reference attempt remains a native cancellation.
[Raw records](../tools/rhino_oracle/observations/subcurve_numeric_followup.json)
retain 50 curves and 1,650 stations, complete originals, getter history and
command-end events. Real viewport and typed-coordinate confirmation picks
establish the step missing from earlier numeric diagnostics. Application tests
replay all 27 UV-command cases at `1e-6` for loci and directed endpoints, with
source purity and Undo/Redo. They cover magnitude/replacement inputs, forward and
backward orientation, curved sources, closed seams, nonuniform segment lengths,
open-end clamping, empty Enter and zero input. Drafting tests preserve units,
calculator expressions and point/angle routing. A self-crossing kernel regression
checks the original-domain endpoint without assuming which tied branch a
closest-point query would select. Direction locking and
B-rep edges remain unimplemented. See [scope](subcurve-length-confirmation.md)
and [provenance](subcurve-length-confirmation-provenance.json).

The preceding 20-recipe public SDK capture ran on private Xvfb with settings scheme
`VibocerosOracleSignedLengthFinal20261007`, returning 16 curve pieces and four
unavailable cases. The source NURBS definitions cover lines, curved polynomials,
stationary endpoints, piecewise spans, polylines, circles, ellipses and an arc.
[Raw records](../tools/rhino_oracle/observations/signed_length_subcurves.json)
retain definitions, 528 stations and measured source purity. Runtime Rust replay
compares every locus and directed endpoint at `1e-6`; the preceding standalone Python
geometry run additionally compares paired stations, with maximum discrepancy
`1.05031e-7`. Independent kernel tests check analytic forms, signed seam crossings,
one complete traversal and a quarter-unit interval after a `1e16` prefix.
Both UV command adapters accept explicit `SubCrvLength` options, with atomic
failure and source/history checks. These use numerical integration rather than
a continuous length certificate. Historical scripted/cursor getter diagnostics
remain recorded: those numeric inputs did not supply the required confirmation
pick and did not yield temporary curves. Those historical producers were
not retained; the fresh canonical SDK producer is hashed. See
[scope](signed-length-subcurves.md) and [provenance](signed-length-subcurves-provenance.json).

The preceding 14-recipe native capture ran on private Xvfb with settings scheme
`VibocerosOracleUVSubcurveClear20261007`. All inline `SubCrv` recipes succeeded,
creating 24 curves and two points. They cover forward/reversed inputs, closed
seam crossings, repeated ranges, mixed grouped whole objects and `SelNone`.
[Raw records](../tools/rhino_oracle/observations/uv_subcurve_input_command.json)
retain complete originals, 792 output curve stations, default temporary-input
attributes, layers, groups, getter events and independent Undo/Redo states.
Command tests compare every captured locus and directed endpoints at `1e-6`,
with source purity and complete metadata/history checks. Application tests cover
typed/mouse picking, invalid endpoints, cancellation, clearing and interruption.
The preceding standalone Python run reproduces all original source/range definitions
through the same command adapters. Ordinary normalized stations agree at
`1e-6`; one closed CreateUVCrv curve has identical ordered linear controls but
different knot spacing, retaining a maximum paired difference of `0.50625`.
Inline length input, direction locking, B-rep edges and that parameter-speed
policy remain incomplete; see [scope](uv-subcurve-input.md) and
[provenance](uv-subcurve-input-provenance.json).

The preceding 14-recipe native capture ran on private Xvfb with settings scheme
`VibocerosOracleUVFacesFinal20261006`. All 12 face-component-preselected
`ApplyCrv` and `CreateUVCrv` recipes succeeded; the two coordinate-input
attempts were rejected and remain in the
[raw records](../tools/rhino_oracle/observations/uv_face_reference_command.json).
Replay compares all 33 stations of every successful output curve at `1e-8`,
retaining the original six surface charts and face indices. Its reconstructed
B-rep contains independent faces and does not reproduce native shared-edge
topology. Separate tests cover every face of a connected kernel box, source
purity, Undo/Redo, ambiguous or invalid references, viewport hits, typed face
indices and component preselection. See [scope](uv-face-references.md) and
[provenance](uv-face-references-provenance.json).

The preceding standalone Python run reproduces nine original untrimmed CreateUVCrv
surface/input definitions. The Rust command replay additionally reconstructs
the two trimmed sources from their original UV curves. A create/apply roundtrip
recovers the original spatial curve within `1e-6`; application tests exercise
surface-first picking, optional point selection, preselection, cancellation and
World-XY output under a rotated construction plane.

The preceding 11-recipe native capture ran on private Xvfb with settings scheme
`VibocerosOracleCreateUVFinal20261006`, creating 17 UV curves and four points.
The [raw records](../tools/rhino_oracle/observations/create_uv_curves_command.json)
retain full inputs, trim curves, isocurve-length diagnostics, output stations,
properties, groups, selection and independent history states. The native coarse
rectangle size differs from accurate integration by about `3.00e-4` for the
radius-two cylinder. The off-surface curve's maximum paired discrepancy is about
`0.00686`; its endpoint also differs from the local closest-parameter projection.
These differences remain explicit in [provenance](create-uv-curves-provenance.json)
and [scope documentation](commands/create-uv-curves.md). Replay uses `1e-6` for
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
[raw records](../tools/rhino_oracle/observations/apply_uv_curves_command.json)
retain full source and target definitions before/after the command, command-end
events, 561 output curve stations, nine output points, properties, selection,
active group definitions and Undo/Redo states. Source and target geometry and
attributes remain unchanged. Rust replay compares the complete set of output
stations at `1e-6`, checks properties/group structure and exercises the actual
document adapter. Application tests verify source/target picking, preselection,
group filtering, aliases, cancellation and World-XY mapping under a rotated
construction plane. See [provenance](apply-uv-curves-provenance.json).

The preceding standalone Python pushup run certified images of all 13 original UV sources,
retaining their original domains. Eight diagnostic images have zero certified
error; the five curved primitive images have complete bounds at most `3.501e-16`,
below their requested limit `1e-6`. The Rust replay checks 1,677 returned-curve
stations against native surface-image witnesses and independently recomputes
every assembled result's continuous certificate.

The preceding 13-recipe Rhino 8.32.26160.13001 capture ran on private Xvfb with settings
scheme `VibocerosOraclePushup20261006`. Its
[raw records](../tools/rhino_oracle/observations/surface_pushup_certified.json)
retain 1,677 source/image stations, all source-purity agreements, five successful
native `Surface.Pushup` operations, and the sphere-seam parameter-speed
discrepancy. The other eight sources retain diagnostic reference curves. See
[provenance](certified-surface-pushup-provenance.json) for source hashes and
contract boundaries. That kernel API now backs the `ApplyCrv` / `ApplyCurves`
document workflow; `Pushup` is the SDK operation name.

The preceding standalone Python run certified all ten nonlinear singular-endpoint
source definitions without endpoint constraints, retaining each original
spatial domain. The polynomial recipes report bounds below `9e-16`; the
square-root boundary recipes refine to 103 controls with complete bounds about
`9.87e-7`, below their input limit `1e-6`.

The preceding ten-recipe Rhino 8.32.26160.13001 capture ran on private Xvfb with settings
scheme `VibocerosOracleInterpolatedPullbackFinal_20261006`. Its
[raw records](../tools/rhino_oracle/observations/surface_pullback_interpolation.json)
retain 1,290 source/image stations and ten source-purity agreements. Eight native
pullbacks succeed geometrically with different parameter speeds; both singular
cubic directions return no native pullback. The retained local before-change
audit failed on all ten recipes. See
[provenance](derivative-free-pullback-provenance.json) for source hashes and
contract boundaries.

The preceding standalone Python pullback run certified all 24 native sources without
endpoint constraints, returning two UV controls and each original spatial
parameter domain. All reported continuous bounds were below `1e-12`, with a
maximum of `8.86e-16`. The run used 16 iterations per source, retaining the
successful bound without recomputing its certificate.

The preceding 24-recipe Rhino capture ran on private Xvfb with settings
scheme `VibocerosOracleLinearPullbackFinal_20261006`. Its
[raw records](../tools/rhino_oracle/observations/surface_pullback_linear.json)
retain full source definitions, source purity, 129 stations per recipe, 20
successful native pullbacks, four native failures, and a parameter-speed
discrepancy for both directions of the swapped two-pole surface. The local
certificates and native geometric fitting have different contracts; see
[provenance](automatic-surface-pullback-provenance.json) and
[timing details](automatic-surface-pullback-performance.json).

The earlier eight-recipe capture also ran on private Xvfb with settings scheme
`VibocerosOraclePullbackEndpointsVerify_20261006`, using the eight
[`surface_pullback_endpoints` recipes](../tools/rhino_oracle/fixtures/surface_pullback_endpoints.json).
Its complete output was identical to the
[retained capture](../tools/rhino_oracle/observations/surface_pullback_endpoints.json),
including the planar parameter-speed differences and independent endpoint
witnesses. Both files had SHA-256
`71593c8aa90b81679ed1a196a5a25fd9738ecd9febdc71cd237934a3e0e436a0`.
The owned Rhino, Python worker, and Xvfb processes exited after the capture.

## What this establishes

The current checked cases cover geometry, document transactions, commands,
interchange, CPU viewport behavior, and Python orchestration. The historical
GPU evidence above separately covers production GPU rendering.
The complete workspace test process was observed to finish with exit status 0.
The nonlinear pullback replay adds 1,290 source and image stations at `1e-11`,
with independent recomputation of the final assembled-curve certificates.
Earlier automatic pullback tests check 3,096
paired spatial and surface-image stations at `1e-11`, independently recomputing
the local continuous bounds. Earlier fixed-endpoint tests retain another 1,032
stations. Geometry regressions preserve adjacent-float and subnormal domains,
singular endpoints, signed weight gauges, fixed constraints, and exact rational
linear crossings in both parameter directions.

The newest native capture covers 18 cursor-direction recipes with four retained
failures and four cancellations. The preceding capture covers a 37-step
preference sequence with raw cancelled queries and option edits. The preceding
capture covers 22 FromMidpoint recipes with three retained
cancellations. The preceding capture covers 15 MarkEnds recipes with two retained
cancellations. The preceding standalone capture covers 17 SubCrv recipes. The
numeric getter/command capture covers 29 recipes,
including one retained cancellation. The signed-length SDK capture covers 20
independent geometry queries. The earlier command capture
covers 14 successful endpoint-input SubCrv recipes.
The preceding face capture retains two rejected coordinate-input attempts.
Other recorded-output
replays are not fresh cross-engine measurements, and tests that merely execute
fixtures with finite output do not establish numerical agreement with Rhino.
Native `Surface.Pullback` and `Surface.Pushup` follow geometric loci and do not
promise these local fitters' normalized-parameter correspondence or fixed UV
endpoint constraints.
Passing these cases does not establish all-command coverage, arbitrary-geometry
epsilon agreement, general curved Booleans, general STEP B-rep interchange,
other graphics backends, or performance parity. Those remain subject to the
implementation boundaries in
the [architecture](architecture.md), [file-format](file-formats.md), and
[oracle timing](oracle.md#timing-interpretation) documentation.
