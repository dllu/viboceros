# Document and command workflows

[Oracle overview](../oracle.md) · [Evidence index](README.md)

`mesh_split_picking.json` uses the owned-window idle-click mechanism with three
disjoint meshes, then invokes the actual `SplitDisjointMesh` command. Sixteen
cases cover ordinary and overlapping groups, ordered bridge memberships,
hidden/locked objects, hidden/locked layers, and connected meshes mixed with
splittable peers. The native oracle builds the same documents and runs the
registered command. All 16 cases replay against the
[saved observations](../../tools/rhino_oracle/observations/mesh_split_picking.json).
Run a read-only replay with:

```sh
python3 -m tools.rhino_oracle replay tools/rhino_oracle/fixtures/mesh_split_picking.json --observations tools/rhino_oracle/observations/mesh_split_picking.json
```

The checked-in `tools/rhino_oracle/observations/mesh_split_picking.json` records
Rhino 8.32.26160.13001 on 2026-09-12: source identity, selection, object mode,
group memberships, layer modes, vertices and face counts, including untouched peers.
A native command regression also compares those 16 recorded output sets exactly;
see the [verified fields and remaining limits](../commands/meshes.md).
Deleted sources are removed from cleanup tracking; surviving original and newly
created mesh IDs are explicitly tracked and cleaned up in the private document.

`mesh_explode_picking.json` uses the same 16 mesh setups but invokes `Explode`.
All 16 cases also replay against its
[saved observations](../../tools/rhino_oracle/observations/mesh_explode_picking.json).
Its observations record retained restricted decomposed sources as
**unselected**, unlike SplitDisjointMesh.
Connected sources remain selected in both commands. Python tests compare their
output records, allowing only the command success field name and decomposed
original-source selection difference. One mixed locked-connected case per
command also records selection and identity retention after actual Undo/Redo.
Those history selections are checked separately, including the unchanged peer.
The shared native test adapter in `mesh_decomposition_tests.rs` compares both
commands against their respective recorded output sets; run
`cargo test -p viboceros-command mesh_decomposition_tests`.
See the [verified native Explode behavior and limits](../commands/editing.md).

`group_picking.json` is a dedicated, untimed three-line fixture: an idle-event
worker returns control to Rhino's normal UI loop, then the host clicks projected
line locations in the newly owned window and acknowledges each click atomically.
No Enter or selection command substitutes for a mouse pick. Its 52 exact checks
cover ordered group selection, hidden/locked peers and layers, and subsequent
Move commands. Run it as a separate batch with `run_headless.sh` (one iteration);
it cannot be mixed with synchronous geometry operations. See [groups](../groups.md).
The idle worker guards against event-loop re-entry and finalizes only once.
Cleanup attempts every owned resource even after a failure, publishes cleanup
errors, and leaves process termination to the owned-window client fallback if
Rhino cannot exit. Fault-injection tests cover callback detachment, setup,
disposable attributes/layers, cleanup, logging, and exit failures.

`selection_recall.json` checks 84 SelPrev sequences with explicit/remembered
options, changed groups, and deleted objects. `selection_recall_picking.json`
uses the dedicated idle worker for 52 recalls after real group picking. These
136 exact comparisons distinguish recall from ordinary group expansion and
verify visibility/lock filtering. See [selection recall](../selection-recall.md).
`last_selection.json` adds 32 exact SelLast sequences after idle Move, checking
group/mode filtering, remembered options, and empty-layer creation. The separate
16-case `last_selection_history.json` and 54-case `deletion_recall.json` now
match complete traces, including immediate Undo/Redo selection. These use the
same owned-window idle worker. `undo_selection.json` adds 10 isolated command
traces for Move, Delete, Explode, DeleteFaces, and ExtractMeshFaces; it checks
whole-object selection, source existence, object/output counts, and mesh face
counts after commands. See [history selection](../history-selection.md).
Repeated picks at identical coordinates do not wait for a new mouse-motion event;
input failures report the case/window and progress without acknowledging a click.

`point_input.json` compares 19 [typed-coordinate sequences](../point-input.md)
against Rhino's actual Polyline prompt in world, Front, Right, shifted, and
oblique construction planes. It checks unrounded resulting vertices, not a
second copy of the parser. The probe restores the plane and selection and deletes
only its own output objects; command execution is untimed.
`point_input_diagnostics.json` retains a large-scale exact-quadrant discrepancy,
not a passing Rhino reference at `1e-12`.

`plane_primitives.json` checks 57 Circle/Polygon/Rectangle/MeshPlane/Box/MeshBox
command results on construction planes. Complete boundary geometry is checked
independently of Box/MeshBox layout; ten raw layout probes remain in
`plane_primitives_representation.json`. See [comparison details](../construction-planes.md).
The [two-point circle fixture](../../tools/rhino_oracle/fixtures/circle_two_point.json)
checks six actual `_Circle _2Point` results against
[Rhino 8 observations](../circle-two-point-rhino-reference.json), including
reversed diameter endpoints, oblique diameter, and tilted CPlane.
The [Arc Center angle fixture](../../tools/rhino_oracle/fixtures/arc_center_angle.json)
compares six actual `_Arc _Center` commands with
[Rhino 8 sampled curves](../arc-center-angle-rhino-reference.json), including a
major arc, capped sweep, negative sweep, and rotated construction plane. Native
samples and domains agree within `1e-10`.
The [bare Arc fixture](../../tools/rhino_oracle/fixtures/arc_default.json) confirms
Rhino's center-first default through three typed angles and two actual endpoint
clicks. The [StartPoint ThroughPoint fixture](../../tools/rhino_oracle/fixtures/arc_start_through_point.json)
checks the explicit three-point path, including a major and tilted arc.
[Default](../arc-default-rhino-reference.json) and
[through-point](../arc-start-through-point-rhino-reference.json) sampled curves and
domains match native results within `1e-10` when endpoint sweep is explicit.
The [Arc Center Length fixture](../../tools/rhino_oracle/fixtures/arc_center_length.json)
compares five signed-length `_Arc _Center` commands against
[live Rhino 8 results](../arc-center-length-rhino-reference.json), including a
rotated construction plane and both signs of length beyond one circumference.
Native curve samples and domains agree within `1e-10`.
The [Arc Center endpoint fixture](../../tools/rhino_oracle/fixtures/arc_center_endpoint.json)
automates eight actual Rhino viewport clicks at Point osnaps with a public
`_Arc _Center` prompt and compares their complete sampled curves and domains
to explicitly directed native arcs within `1e-10`, including a construction
plane with its normal reversed. The
[single south pick](../../tools/rhino_oracle/fixtures/arc_center_endpoint_south_alone.json)
produces a clockwise 90° arc, while the same pick after another operation in
the batch produces a counterclockwise 270° arc. Their
[batch](../arc-center-endpoint-rhino-reference.json) and
[single-pick](../arc-center-endpoint-south-alone-rhino-reference.json) records
show that interaction history affects the sweep; cursor travel appears to be
the deciding factor.
The [Arc Center Midpoint fixture](../../tools/rhino_oracle/fixtures/arc_midpoint.json)
compares 17 typed angle/length commands and actual endpoint clicks against
[Rhino 8 sampled curves](../arc-midpoint-rhino-reference.json). It covers
positive and negative sweeps, the 360° boundary, off-radius endpoint picks,
and a reversed construction-plane normal. Native curves and domains agree
within `1e-10` for explicitly directed endpoint picks. A separate same-radial
endpoint probe was rejected by Rhino and is rejected by the native command.
The [Arc StartPoint Direction fixture](../../tools/rhino_oracle/fixtures/arc_start_direction.json)
compares five typed Rhino direction constructions with
[live sampled curves](../arc-start-direction-rhino-reference.json): both turn
directions, a major arc, an oriented construction plane, and an off-plane
endpoint. Native samples and domains agree within `1e-10`.
The [Arc StartPoint Center fixture](../../tools/rhino_oracle/fixtures/arc_start_center.json)
compares ten actual Rhino commands and viewport endpoint picks with
[live sampled curves](../arc-start-center-rhino-reference.json), including signed
angles and lengths, major and quarter arcs, and reversed construction-plane
normal. Native curves and domains agree within `1e-10` when endpoint sweeps
are made explicit.
The [three-point circle fixture](../../tools/rhino_oracle/fixtures/circle_three_point.json)
checks four actual `_Circle _3Point` results against
[Rhino 8 observations](../circle-three-point-rhino-reference.json), including
reversed point order and off-CPlane geometry. Complete sampled curve records
and domains agree within `1e-10`; command execution is untimed.
The [three-point Radius fixture](../../tools/rhino_oracle/fixtures/circle_three_point_radius.json)
checks four numeric-radius orientations against
[Rhino 8 records](../circle-three-point-radius-rhino-reference.json). The
[picked Radius fixture](../../tools/rhino_oracle/fixtures/circle_three_point_radius_picks.json)
checks on-plane and off-plane radius locations against
[live results](../circle-three-point-radius-picks-rhino-reference.json).
The [circle size fixture](../../tools/rhino_oracle/fixtures/circle_size_options.json)
checks Diameter, Circumference, and Area against
[Rhino 8 numeric results](../circle-size-options-rhino-reference.json). The
[picked size fixture](../../tools/rhino_oracle/fixtures/circle_size_picks.json)
checks Rhino's distinct point behavior for the same three options against
[live results](../circle-size-picks-rhino-reference.json).
The [Vertical fixture](../../tools/rhino_oracle/fixtures/circle_vertical.json)
checks world, off-plane, and rotated-CPlane direction picks against
[Rhino records](../circle-vertical-rhino-reference.json). A separate
[numeric-radius fixture](../../tools/rhino_oracle/fixtures/circle_vertical_numeric.json)
uses a direction point beyond the fixed radius and matches its
[Rhino result](../circle-vertical-numeric-rhino-reference.json).
The [Orientation fixture](../../tools/rhino_oracle/fixtures/circle_orientation.json)
checks five normal directions and construction planes against
[Rhino records](../circle-orientation-rhino-reference.json). The
[radius-point fixture](../../tools/rhino_oracle/fixtures/circle_orientation_picks.json)
checks on-plane and projected off-plane picks against
[live results](../circle-orientation-picks-rhino-reference.json). The
[picked-size fixture](../../tools/rhino_oracle/fixtures/circle_orientation_size_picks.json)
checks off-plane Diameter, Circumference, and Area input against
[live results](../circle-orientation-size-picks-rhino-reference.json).

`plane_transforms.json` checks 140 actual transform commands using four affinely
independent point witnesses per operation, including copied/original identity
and selection. Four parallel-reference Shear cases remain explicit diagnostics
in `plane_transform_diagnostics.json`; see [plane transforms](../plane-transforms.md).

`interface_commands.json` checks 208 untimed display/snapping transitions across
eight initial states against Rhino's actual commands. The native probe shares
the GUI's parser and state reducer. The worker restores application settings
and all viewport modes; see [interface controls](../commands/interface.md).
`drafting_aids.json` adds 22 live Ortho, Planar, CPlane Z, and Ortho angle
transitions. Its [retained Rhino response](../../tools/rhino_oracle/observations/drafting_aids.json)
replays against the native reducer with a maximum angular difference of
`7.1e-15` degrees. RhinoCommon reports the angle in radians; the probe records
degrees. Cursor positions still need a separate live comparison.

`construction_planes.json` checks 129 actual plane edits/history transitions.
`construction_plane_input.json` checks 24 transparent CPlane commands inside
Polyline prompts, including local and relative coordinate continuation.
See [construction-plane editing](../cplane.md) for accuracy and remaining options.

`plane_arrays.json` checks 64 actual rectangular/linear/polar array commands,
including CPlane axes, tight curve extents, signed Fill lengths, zero-spacing
cells, selection, and groups. Initial scripted 3D Fill-height discrepancies
remain in `plane_array_diagnostics.json`; see [plane arrays](../plane-arrays.md).
`curve_bounds.json` checks 16 timed tight-curve-box queries. A negative-gauge
Rhino discrepancy remains in `curve_bounds_diagnostics.json`, separate from
passing references; see [bounds policy and measurements](../curve-bounds.md).
`surface_bounds.json` adds 25 passing timed surface-box queries.
`surface_bounds_diagnostics.json` retains four inaccurate Rhino boxes, with
untimed 41×41 `sample_grid` witnesses recorded separately as `sample_bounds`.
`surface_array_bounds.json` compares 32 actual surface/mixed-source arrays;
24 pass and eight retain placement discrepancies at `1e-8`.
See [surface bounds](../surface-bounds.md) for coverage and limitations.
`parameter_curve_bounds.json` compares 20 exact surface images against
independently supplied spatial reference curves; a signed-rational Rhino box
discrepancy remains in `parameter_curve_bounds_diagnostics.json`.
`trim_boundary_bounds.json` adds six exact B-rep boundary comparisons.
These untimed probes retain direct surface-image samples and do not imply
complete trimmed-face bounds; see [trim-boundary bounds](../trim-boundary-bounds.md).
`trimmed_brep_bounds.json` separately compares 11 complete B-rep boxes, with
five retained box/trim-evaluation diagnostics in
`trimmed_brep_bounds_diagnostics.json`; see [trimmed-face bounds](../trimmed-face-bounds.md).
`trimmed_brep_array_bounds.json` exercises 32 actual arrays of single- and
multi-face trimmed B-reps, recording every face domain and trim-image samples.
`bounding_box.json` checks 57 actual BoundingBox commands, including output
corners/topology, groups, reports, and World/CPlane orientation.
`bounding_box_mixed.json` compares six mixed valid/point selections with actual
Rhino partial output and failure reporting; all six match. The remaining 20 of
the 26 `bounding_box_diagnostics.json` cases retain curved-bound and thin-geometry
differences. See [bounding boxes](../commands/bounding-box.md), including
why raw Rhino script success flags are not used for report-only commands.
`distribute.json` adds 188 actual distribution commands with signed spacing,
World/CPlane directions, bound tie ordering, retained source identities, and
top-group membership rules. Six analytically checked curved-bound placement
differences remain in `distribute_diagnostics.json`; see
[Distribute](../commands/distribute.md). Its batch preselection guard avoids
leaving Rhino in an interactive object-selection prompt.

`group_memberships.json` contains 56 comparisons of ordered object memberships and the reverse
group-member table after every fixture step. It exercises membership edits,
deletion, nested/partial `Ungroup` and `UngroupAll`, copied-group allocation,
decomposition, and older-group distribution. IDs are compared as retained-source
flags; domains, sampled geometry, and selection are checked separately.
Only automatic `Group`-plus-digits names are canonicalized, by numeric order:
the reused Rhino document reserves those names across cleaned-up operations.
Explicit names and incorrectly styled copy names remain literal. This does not
establish absolute deleted-name allocation history; see [groups](../groups.md).
The former ConvertToBeziers diagnostic now passes and is part of the regular
group fixture; ordinary Delete also retains empty definitions. The separate
`bezier_conversion.json` compares complete output NURBS control nets, source
identity, domains, sampled geometry, output creation order, current/source layers,
colors, names, groups, and selection after explicit deletion choices; see
[Bézier conversion](../commands/beziers.md). A separate point-cloud-cycle probe uses
the same preselection workflow as the native command; the previous in-command
SelID workflow did not.

`single_span_conversion.json` adds 78 actual surface conversions covering
directional strips, Both patches, no-ops, trims, unclamped/periodic structure,
mixed sources, and U/V toggles. `conversion_sessions.json` adds eight self-seeded
sequences (39 steps) checking independent remembered deletion/direction choices,
partial option changes, no-op acceptance, and undo. They share the full conversion
recorder and owned-object cleanup. See [single spans](../commands/single-spans.md)
for the explicit omitted-outer-knot codec and [option memory](../command-options.md)
for lifetime and bootstrap limitations. These command probes are untimed.

`nurbs_conversion.json` compares 78 actual `ToNURBS` conversions, including meshes
and already-NURBS no-ops. It additionally records curve representation kind and
full source curve/surface definitions so identity-preserving replacement cannot look like a
no-op. B-reps include topology and full underlying surface nets. Six
`nurbs_conversion_sessions.json` sequences add 39 steps; unlike single-span
conversion, `ToNURBS` no-ops do not accept new choices. See
[ToNURBS](../commands/to-nurbs.md) for parameters, chronological renewal, and limits.

`mesh_nurbs_conversion.json` adds 50 actual preselected `MeshToNURB` comparisons,
distinct from the older `mesh_to_nurb.json` geometry-API probes. Three
`mesh_nurbs_conversion_sessions.json` sequences add 21 option-memory steps.
Requested Rhino options are first seeded on a separate owned temporary mesh
because preselection hides the command's option prompt. Results include full
B-rep topology and surface nets, source/output metadata, selection, group tables,
and creation order; no result selection is normalized.
`mesh_nurbs_postselection.json` adds 60 prompted-selection cases; three
`mesh_nurbs_postselection_sessions.json` sequences add 18 steps. `postselect`
uses the actual option/picking prompt with ordered `SelID` inputs; `cancel`
escapes it, including an empty picked list. Optional `initial_selection` contains
only non-mesh sources, so it cannot accidentally trigger preselected conversion.
Cancellation cannot request undo. Full source/output records capture cleared
selection, pick-dependent creation order, and choices retained after cancellation.
See
[MeshToNURB](../commands/mesh-to-nurb.md) for coverage and selection-path limits.

`nurbs_postselection.json` adds 87 cases and three
`nurbs_postselection_sessions.json` sequences add 31 steps. ToNURBS macros complete
ordered object selection before entering the separate conversion options and nested
MeshOptions prompt. `cancel` can cancel preselected or prompted confirmation;
`cancel_at_selection` additionally requires prompted cancellation and permits an
empty picked list. No-op input cannot reach confirmation, and cancelled commands
cannot seed option memory. A separate script builder is tested for precise phase
ordering. Native probes exercise explicit ordered copies/renewal and preserve
complete source/output records without normalizing selection or creation order.

`bezier_postselection.json` adds 61 cases and two
`bezier_postselection_sessions.json` sequences add 24 steps. The macro completes
ordered selection before answering the separate Yes/No deletion question; the
answer finishes the command without an extra Enter. Cancellation at selection
or at the deletion question accepts no choice. Initial selection must be
ineligible, and cancelled commands cannot seed a deterministic session. The
same complete conversion records check source retention/deletion, fresh output
attributes, empty groups, cleared picks, and pick-dependent creation order.

`single_span_postselection.json` adds 108 cases and three
`single_span_postselection_sessions.json` sequences add 34 steps. Macros finish
selection before entering the options stage and its Direction chooser. Cancelled
options are remembered, but selection-stage presets are not. This includes
toggle-only follow-ups and no-op input. A cancelled options stage can seed a
session; cancelled selection cannot. Disjoint grouped subset probes avoid
confounding group behavior with overlapping-surface hit ambiguity.

## BlockEdit dialog controls

An owned native viewport gesture is captured separately with:

```sh
WINEDLLOVERRIDES='sspicli,secur32,schannel=b' tools/rhino_oracle/run_headless.sh exec python3 -m tools.rhino_oracle.block_edit_double_click_capture --output /tmp/block-edit-double-click.json --timeout 300
```

This probe uses a fresh line instance and a real double-click at its projected
midpoint, checks exposed members and root visibility, then discards and compares
the original model IDs. It uses the existing owned-process/idle picker transport;
its envelope is a dedicated gesture experiment, not a new geometry operation.

`block_workflow` supports `edit_roundtrip` with optional `add_objects` model
handles, `remove_members` definition-member ordinals and a world `base_point`.
Saved releases allocate logical model handles; discarded releases allocate none.
`translation_api` distinguishes individual public-SDK transforms from a native
Move command. Definition-member order remains part of the comparison.

These captures run the worker from Rhino's idle event, inspect the owned dialog
through public WPF/Win32 interfaces, click its actual buttons and wait for the
native getter prompt before typing prescribed input into the owned command line.
The command reference documents the supported local UI and remaining scope.

The current Wine launcher needed built-in security libraries to resolve its
`AcquireCredentialsHandleW` licensing error. Reproduce the control capture with:

```sh
WINEDLLOVERRIDES='sspicli,secur32,schannel=b' tools/rhino_oracle/run_headless.sh rhino tools/rhino_oracle/fixtures/block_edit_controls.json --scheme VibocerosOracleBlockControlsReplay20261009 --timeout 300
```

See [control provenance](../block-edit-controls-provenance.json). This invocation
uses a private Xvfb display and settings scheme; it does not change the launcher
or the user's Wine configuration.

## Directional grip selection

`grip_navigation` replays Next/Prev U/V, their additive forms and SelU/SelV
against shared source models. The capture contains 25 workflows and 1,650
completed commands, including `Nothing` results for empty or mesh-only inputs.
Every step retains its before/after grip identities, locations and selection,
ordinary object selection and native result. Geometry equality is checked during
capture; a prepared Redo item must survive the complete command sequence.

```sh
python3 -m tools.rhino_oracle replay \
  tools/rhino_oracle/fixtures/grip_navigation.json \
  --observations tools/rhino_oracle/observations/grip_navigation.json \
  --absolute-epsilon 1e-8 --relative-epsilon 1e-12
```

Add `--native-smlib` to replay with the optional backend. To regenerate the
owned Rhino capture and its 3DM inputs:

```sh
tools/rhino_oracle/run_headless.sh exec \
  python3 -m tools.rhino_oracle.generate_grip_navigation
```

In another checkout, `generate_grip_navigation.request()` produces the same
recipes with that checkout's source paths. Supply it to `OracleClient.replay`
with the saved observation. The [validation receipt](../grip-navigation-validation.json)
records source and artifact hashes, comparison bounds and qualification limits.
`SelUV` has a separate interactive surface-row picker and is outside this corpus.
