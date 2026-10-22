# SubCrv and face input captures

[Historical checkpoint](../../validation-checkpoint.md)

The preceding edge SubCrv capture ran eleven owned public commands on private Xvfb
under `VibocerosOracleSubcurveEdgesFinal20261007`, including box, planar surface
and circular cylinder edges. All succeed. [Complete native records](../../../tools/rhino_oracle/observations/subcurve_edge.json)
retain parent B-rep definitions, public edge preselection, macros, events,
metadata and independent Undo/Redo. Application replay checks 297 curve stations,
directed endpoints and four marker points at `1e-6`, copied attributes/groups,
source snapshot identity and complete history restoration. Copy=No retains the
parent and creates a new curve. Tests cover typed edge references, atomic
failures, stale sources, numbered ambiguity choices and cancellation. A real
CPU pointer-event test covers both ordinary curve clicks and surface edges;
a cache regression checks reuse across moving hover and invalidation by edge,
source and tolerance. The final application rerun includes that cache test.
A fresh production wgpu/egui inspection on private Xvfb confirms edge selection,
curve/marker previews, acceptance and Undo/Redo. The [saved screenshot and scope](../../subcurve-edge-input.md)
document that inspection. Native command-first mouse picking, numeric edge
getters and arbitrary trimmed-edge parity remain unverified.

The preceding SubCrv preview checks ten pending workflows against saved native
confirmed results: 330 stations and directed endpoints at `1e-6`. Repeated
preview calls preserve source snapshot identity, attributes, selection and
history, reuse cached geometry, and cover Copy, MarkEnds, locked-side rejection,
midpoint symmetry and cancellation. The CPU egui check covers four viewport
orientations in Wireframe, Shaded and Ghosted, including no hovered viewport and
inactive input. A fresh private-Xvfb application inspection through the production
wgpu/egui renderer confirms trim overlays, marker-only mode and removal after
Escape. The [saved image and scope](../../subcurve-preview.md) document that inspection;
this does not add a new opt-in GPU raster test or a native preview measurement.

A preceding 32-recipe closed SubCrv numeric workflow ran on private Xvfb under
`VibocerosOracleClosedDirectionComplete20261007`. The
[raw records](../../../tools/rhino_oracle/observations/subcurve_direction_grid.json)
retain 16 intermediate snapshots with public InGetPoint=true and
InGetObject=false, intact sources and no created geometry. The generic Select
curve prompt keeps a pending number; it does not start a new source selection.
Twenty-six commands succeed and six cancel an unconfirmed length. Application
replay checks original input tokens, pending source/start/length, released source
selection and absent history, then 924 output curve stations and endpoints at
`1e-6`, selection and independent Undo/Redo states. Opposite-side confirmation
can override the original lock; a skewed polyline distinguishes nearest candidate
endpoints from the shorter arc toward the pick. Two nested UV workflows preserve
parent-owned history. Explicit script regressions retain valid closed lengths
in both directions. See [scope and provenance](../../subcurve-direction-confirmation.md).

A preceding 18-recipe SubCrv direction workflow ran on private Xvfb under
`VibocerosOracleDirectionVerified20261007`. The
[raw records](../../../tools/rhino_oracle/observations/subcurve_direction.json)
retain actual motion, locked/unlocked getter prompts, every public
SendKeystrokes input, command events, source/output definitions and independent
Undo/Redo states. Ten commands succeed; four opposite-side open point inputs
end with Failure and four closed numeric inputs with Cancel. App replay checks
330 output curve stations and two markers at `1e-6`, directed endpoints, source
purity, selection and history. The backward seam-crossing polyline numeric case
succeeds. The four cancelled closed numbers remained in a point getter; the
follow-up above corrects the earlier interpretation as a source restart. Inline
open numeric input shares the policy, with source purity and parent-owned history. A CPU
viewport test confirms that hover reports a location without accepting a point
or editing history. See [scope and provenance](../../subcurve-direction.md).

A preceding 37-step SubCrv preference workflow ran on private Xvfb in settings
scheme `VibocerosOracleSubcurvePreferenceFinal20261007`. The
[raw records](../../../tools/rhino_oracle/observations/subcurve_preferences.json)
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
unverified. See [scope](../../subcurve-option-memory.md) and
[provenance](../../subcurve-option-memory-provenance.json).

The preceding 22-recipe FromMidpoint capture ran on private Xvfb with settings scheme
`VibocerosOracleSubcurveMidpointFinal20261007`. Nineteen commands succeed; zero
and half/full-perimeter coincident endpoints remain native cancellations.
The [raw records](../../../tools/rhino_oracle/observations/subcurve_midpoint.json)
retain original/output curves, stations, marker points, metadata, groups,
selection, events and Undo/Redo. Numeric inputs are half-lengths and finish
immediately; point inputs derive that radius along the source. Open sides clamp
independently, while closed endpoints wrap and keep their forward interval,
including the over-half-perimeter remainder. Command/app replays compare every
locus, directed endpoint and marker at `1e-6`, with metadata and history checks.
They cover curved input, Copy/replacement, endpoint centers, option changes and
MarkEnds. Midpoint policy inherits numerical integration and parameter-resolution
limits; direction locking, live preview, B-rep edges, restart option persistence and
reactive History remain outstanding. See [scope](../../subcurve-midpoint.md) and
[provenance](../../subcurve-midpoint-provenance.json).

The preceding 15-recipe SubCrv MarkEnds capture ran on private Xvfb with settings
scheme `VibocerosOracleMarkEnds20261007`. Thirteen commands produce 26 endpoint
markers; zero and missing confirmation remain native cancellations. The
[raw records](../../../tools/rhino_oracle/observations/subcurve_mark_ends.json) retain
full original curves, point coordinates, attributes, groups, selection, command
events and Undo/Redo. Command and app tests replay all marker coordinates at
`1e-6`, with source purity and default-attribute checks. Both Copy choices produce
ungrouped, unselected current-layer points; a full closed traversal retains two
coincident markers. The app can change Mode during input without edits, and bad
modes or geometry fail atomically. Undo/Redo restore marker IDs and empty
selection. Restart mode persistence and reactive History remain unimplemented; see
[scope](../../subcurve-mark-ends.md) and [provenance](../../subcurve-mark-ends-provenance.json).

The preceding 17-recipe standalone SubCrv capture ran on private Xvfb with settings
scheme `VibocerosOracleStandaloneSubcurveFinal20261007`. Fifteen commands succeed;
zero and missing confirmation remain native cancellations. The
[raw records](../../../tools/rhino_oracle/observations/standalone_subcurve.json) retain
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
outstanding. See [scope](../../commands/subcurve.md) and
[provenance](../../standalone-subcurve-provenance.json).

The preceding 29-recipe command capture ran on private Xvfb with settings scheme
`VibocerosOracleSubcurveConfirmCommands20261007`. Twenty-eight commands succeed;
the standalone object-ID reference attempt remains a native cancellation.
[Raw records](../../../tools/rhino_oracle/observations/subcurve_numeric_followup.json)
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
B-rep edges remain unimplemented. See [scope](../../subcurve-length-confirmation.md)
and [provenance](../../subcurve-length-confirmation-provenance.json).

The preceding 20-recipe public SDK capture ran on private Xvfb with settings scheme
`VibocerosOracleSignedLengthFinal20261007`, returning 16 curve pieces and four
unavailable cases. The source NURBS definitions cover lines, curved polynomials,
stationary endpoints, piecewise spans, polylines, circles, ellipses and an arc.
[Raw records](../../../tools/rhino_oracle/observations/signed_length_subcurves.json)
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
[scope](../../signed-length-subcurves.md) and [provenance](../../signed-length-subcurves-provenance.json).

The preceding 14-recipe native capture ran on private Xvfb with settings scheme
`VibocerosOracleUVSubcurveClear20261007`. All inline `SubCrv` recipes succeeded,
creating 24 curves and two points. They cover forward/reversed inputs, closed
seam crossings, repeated ranges, mixed grouped whole objects and `SelNone`.
[Raw records](../../../tools/rhino_oracle/observations/uv_subcurve_input_command.json)
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
policy remain incomplete; see [scope](../../uv-subcurve-input.md) and
[provenance](../../uv-subcurve-input-provenance.json).

The preceding 14-recipe native capture ran on private Xvfb with settings scheme
`VibocerosOracleUVFacesFinal20261006`. All 12 face-component-preselected
`ApplyCrv` and `CreateUVCrv` recipes succeeded; the two coordinate-input
attempts were rejected and remain in the
[raw records](../../../tools/rhino_oracle/observations/uv_face_reference_command.json).
Replay compares all 33 stations of every successful output curve at `1e-8`,
retaining the original six surface charts and face indices. Its reconstructed
B-rep contains independent faces and does not reproduce native shared-edge
topology. Separate tests cover every face of a connected kernel box, source
purity, Undo/Redo, ambiguous or invalid references, viewport hits, typed face
indices and component preselection. See [scope](../../uv-face-references.md) and
[provenance](../../uv-face-references-provenance.json).
