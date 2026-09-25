# Rhino compatibility oracle

[Project overview](../README.md)

[Diagnostic replay and Python API](oracle-replay.md) retain per-operation native
errors while comparing every successful record against saved Rhino observations.
On Linux, the CLI automatically runs live `rhino` and `compare` probes under
the dedicated `run_headless.sh` Xvfb display. Replay and native-only modes do
not launch Rhino. Set `VIBOCEROS_RHINO_VISIBLE=1` only for an intentional
interactive desktop run.

The [Match fixture](../tools/rhino_oracle/fixtures/curve_match_geometry.json)
contains 33 live Rhino `CreateMatchCurve` cases for single-span line, polynomial,
and rational curves across end orientation, continuity, and preserved opposite
end options. Replay against its [saved observation](../tools/rhino_oracle/observations/curve_match_geometry.json)
with `python3 -m tools.rhino_oracle replay tools/rhino_oracle/fixtures/curve_match_geometry.json --observations tools/rhino_oracle/observations/curve_match_geometry.json`.
All 33 outputs agree within the default 1e-10 absolute tolerance. The
[average-tangency fixture](../tools/rhino_oracle/fixtures/curve_match_average_tangency.json)
adds eight live cases, including rational handles and reversed selected ends;
all eight match their [Rhino observations](../tools/rhino_oracle/observations/curve_match_average_tangency.json)
within the same tolerance. The
[average-curvature fixture](../tools/rhino_oracle/fixtures/curve_match_average_curvature.json)
adds six matching [Rhino observations](../tools/rhino_oracle/observations/curve_match_average_curvature.json)
for opposite-end preservation of None or Position. The
[average-position fixture](../tools/rhino_oracle/fixtures/curve_match_average_position.json)
adds ten matching [Rhino observations](../tools/rhino_oracle/observations/curve_match_average_position.json)
for trimmed and reversed ends, including rational controls. One
[boundary diagnostic](../tools/rhino_oracle/fixtures/curve_match_average_position_boundary.json)
retains a difference: Rhino trims a straight cubic to a roughly 1e-6-unit
segment when its closest point is the far endpoint; native closest-point search
returns the endpoint exactly. The
[average multi-span curvature fixture](../tools/rhino_oracle/fixtures/curve_match_average_multispan_curvature.json)
adds 13 matching [Rhino observations](../tools/rhino_oracle/observations/curve_match_average_multispan_curvature.json)
for multi-span sources and references, rational weights, uneven knots, reversed
ends, and preserved far tangents. Its
[boundary fixture](../tools/rhino_oracle/fixtures/curve_match_average_multispan_curvature_boundary.json)
retains two [Rhino observations](../tools/rhino_oracle/observations/curve_match_average_multispan_curvature_boundary.json)
that still need knot edits to preserve far curvature. The
[multi-span tangency fixture](../tools/rhino_oracle/fixtures/curve_match_multispan_tangency.json)
adds ten matching [Rhino observations](../tools/rhino_oracle/observations/curve_match_multispan_tangency.json)
for cubic and quadratic two-span sources, opposite-end preservation, and
reversed picks. The
[multi-span position fixture](../tools/rhino_oracle/fixtures/curve_match_multispan_position.json)
adds seven matching [Rhino observations](../tools/rhino_oracle/observations/curve_match_multispan_position.json).
A more precise NURBS closest-point stopping rule resolves the interior trim
parameter at the default model tolerance. The
[multi-span curvature fixture](../tools/rhino_oracle/fixtures/curve_match_multispan_curvature.json)
adds three matching [Rhino observations](../tools/rhino_oracle/observations/curve_match_multispan_curvature.json)
for preserved opposite-end options None, Position, and Tangency. Its
[extended fixture](../tools/rhino_oracle/fixtures/curve_match_multispan_curvature_extended.json)
adds 15 matching [observations](../tools/rhino_oracle/observations/curve_match_multispan_curvature_extended.json)
for uneven knots, rational weights, and reversed selected ends. The
[uneven rational boundary case](../tools/rhino_oracle/fixtures/curve_match_multispan_curvature_boundary.json)
retains one mismatch in the tangential second-control coordinate when the
second control and endpoint weights are equal; its
[Rhino observation](../tools/rhino_oracle/observations/curve_match_multispan_curvature_boundary.json)
keeps the difference reproducible. The
[additional Rhino-only probe](../tools/rhino_oracle/fixtures/curve_match_rhino_only.json)
remains as historical evidence for the now-supported two-span curvature case.

The `viewport_arrangement_probe` operation records model viewport bounds,
titles, cameras, projection, floating state, and active view after a bounded
sequence of `NewViewport`, `CloseViewport`, `3View`, `4View`, and viewport split
commands. Run its [fixture](../tools/rhino_oracle/fixtures/viewport_arrangement.json)
with `tools/rhino_oracle/run_headless.sh rhino tools/rhino_oracle/fixtures/viewport_arrangement.json --timeout 300`.
It uses public [RhinoView properties](https://mcneel.github.io/rhinocommon-api-docs/api/RhinoCommon/html/Properties_T_Rhino_Display_RhinoView.htm).
The [default-view](viewport-arrangement-rhino-reference.json) and
[Front-active](viewport-arrangement-front-rhino-reference.json) Rhino 8.32
responses on 2026-09-25 confirm a centered, overlapping Top view and activation
of the previously active view when it closes. A
[repeated-NewViewport run](viewport-arrangement-repeated-rhino-reference.json)
confirms that successive views stack at the same centered rectangle and close
back through the prior active views. A
[Shaded-source run](viewport-arrangement-shaded-rhino-reference.json) confirms
that the new Top view starts in Wireframe even when the source is Shaded.
The Wine session required a normal
launcher `stop` before these probes could start.
The [4View projection run](viewport-arrangement-projection-rhino-reference.json)
checks both first- and third-angle layouts, their camera directions and active
Perspective view, and a Front-active reset. The worker sends Rhino's command-line
option as `Projection=<angle>` followed by Enter.
The [repeated 4View run](viewport-arrangement-fourview-repeat-rhino-reference.json)
checks a split four-view arrangement, a changed Top camera, a maximized Top
view, and a shifted Perspective camera. In these cases the first plain `4View`
call restored standard views and activated Perspective. Shaded Perspective
survived either projection arrangement, while Right-to-Left and Bottom-to-Top
replacements started in Wireframe.
The [grid-spacing run](viewport-arrangement-fourview-grid-rhino-reference.json)
sets distinctive grid and snap spacing through each viewport's public
construction-plane API before plain and explicit `4View` calls. It confirms
that unchanged Top keeps its spacing, first-angle Left takes Right's spacing,
and returning to third-angle resets Right's spacing. Each operation first
establishes a standard four-view layout with unit grid and snap spacing.
Changing Top to Bottom or Front to Back before plain `4View` gives the restored
Top or Front and Right the changed viewport's spacing. A changed Right-to-Left
view restores Right's spacing without changing the other grids.

The [orientation audit](orientation-audit.md) separates public document insertion
and replacement from actual `Flip` command behavior, retaining full definitions
and selection states. This diagnostic is not an identical-source native replay.
The separate [solid-orientation query](solid-orientation.md) compares 49 shared
3dm inputs without document insertion: all geometry records and 47 orientations
now match, with two coincident-shell orientations unresolved. Its
[planar follow-up](planar-solid-orientation.md) retains 68 further cases and eight
fresh-session repeats, exposing ten translation-sensitive Rhino classifications
despite identical geometric definitions after exact inverse translation.
The [Join scale audit](join-orientation.md) adds full-definition command records
at unit and volume-overflow scales, separating fixed native execution errors
from Rhino's large-coordinate orientation and normalization discrepancies.
The [shared-source document admission probe](document-brep-admission.md) separates
insertion and replacement using identical caller-owned inputs, with full
definitions, identity, attributes, groups, selection, and immutability checks.
It now resolves 40 normalization differences (56/64 full matches); a separate
16-case actual file-import capture has 14 full matches. Coincident-shell
orientation remains unresolved in both audits, with all numeric fields equal.

The [conic-center audit](conic-center-audit.md) compares 32 full-coordinate public
API records and 62 calibrated point-prompt records. It keeps API recognition,
interactive snap policy, and their remaining discrepancies separate.

The [unconstrained point-snap probe](point-snaps.md) records full 3D targets,
snap kinds and source ownership through public `GetPoint` APIs. Its 101 retained
line/mesh cases resolve the previous Mesh Near coordinate discrepancies while
preserving three corner-edge selection differences. Source/camera-only rational
references and native replay remain separate from the observed picked points.
The [mesh-competition follow-up and point replay API](mesh-snap-order.md) expose
broader wire-selection differences with 84 retained captures, including repeated
vertex-order checks and public picking diagnostics. `projected_object_snap`
accepts calibrated native inputs; `python3 -m tools.rhino_oracle.point_snap_replay`
compares target/kind/source and returns exit status 1 for real parity differences.

The `join_command` probe has 181 raw mesh records covering both
`JoinDisjointMeshes` choices, precision thresholds, ordered selection, winding,
attributes, groups, and identity replacement. It checks identical binary64
vertices and face indices before and after inserting each Rhino source;
no mesh output normalization is used. A further 140 paired Join/JoinCopy
workflow records cover curves and meshes, selection, no-ops, and source retention.
Curve records include native type, domains, sampled positions, NURBS definitions,
and raw polycurve local domains plus parent intervals; no affine-domain
normalization is applied during recording. Another 284 cases cover closed-chain
seams and early completion. The probe snapshots the named command's public
EndCommand event, requires exactly one completion, and always detaches its
handler. Callback errors fail the probe even if the host swallows exceptions.
The native adapter stops individual selection at the same completion boundary.
See [joining evidence and limits](commands/join.md) and the
[event transcripts and measurement correction](join-cycles.md).
Another [544 encoding and seam records](join-encodings.md) check rational weights,
higher-degree controls, polycurve leaves, and their local/parent parameter maps.
An additional [48 endpoint-search records](join-endpoint-search.md) check nearby
connections in the presence of distant unrelated sources, with absolute-only
comparison and exact preservation of the distant source records.
The [one-pass seeded-join audit](seeded-join.md) replays all 1,197 Join records and
refreshes the 140 workflow cases against an owned private-Xvfb Rhino session.

Border duplication has 86 matching live command records and offline replay
checks, including edge-table permutations. See [border validation](borders.md)
for the resolved polysurface seam discrepancies and identical-source 3DM protocol.

The [Cap command audit](commands/cap.md#oracle-evidence) adds 140 matching
shared-input B-rep records, including four resolved kinked-boundary segmentation
gaps. Two additional raw discrepancies cover negative rational weights and a
large knot origin; the latter also retains analytic area/volume witnesses.
Its schema compares exact spatial edge definitions and oriented incidence,
not newly generated cap UV parameterizations. The probe verifies that Rhino
document insertion preserves the shared source before invoking the command.
The [compound follow-up](cap-compound-orientation.md) adds 100 actual command
records: 96 match after correcting 28 total-volume orientation errors, while
four coincident-shell topology discrepancies remain explicit.

The [B-rep edge assembly audit](brep-edge-joining.md) compares explicitly paired
native boundaries with public automatic RhinoCommon `JoinBreps` on identical
shared 3DM inputs. It records full surfaces and UV trim definitions in addition
to spatial topology, tolerances, and mass properties. Raw orientation and
gap-rebuilding policy differences are retained separately. This geometry-only
probe does not establish interactive Join behavior or native automatic matching.

The separate [surface Join command audit](commands/join.md#surfaces-and-polysurfaces)
adds 92 actual Join/JoinCopy records with shared per-source 3DM artifacts.
Its original 26-case discrepancy archive is retained, but four partial-overlap
and sixteen gap cases now fully match; four duplicate-wall cases differ only in zero-volume
face senses. The [follow-up boundary audit](join-boundary-matching.md) adds 160
records, now 148 fully matching and 12 explicit zero-volume orientation differences.
The [gap-rebuilding audit](join-gap-rebuilding.md) adds 108 cases and resolves
20 earlier differences. The [selection-distance audit](join-selection-distance.md)
adds 150 cases and resolves two more earlier differences. The
[corner-only follow-up](join-corner-clustering.md) adds 84 cases, separating
endpoint movement from edge mating. Across all five audits,
471 of 594 cases fully match; the remaining 123 explicitly cover zero-volume
orientation, cutoff endpoints, order-dependent clusters, and area integration.
The command probe includes spatial and UV geometry, component
tolerances, oriented topology, object identity/selection, attributes, and groups.
Copies with a repeated source retain every disconnected piece. Neither failed
comparisons nor domain differences are normalized out of the saved observations.
Subsequent curved, projective and [short-overlap audits](join-short-overlaps.md)
bring the cumulative surface-command record to 804 cases: 633 full matches,
32 native errors and 139 other differences. The short-overlap work also corrects
native source construction to use the fixture tolerance, independently of a
command's document-tolerance override, so both engines operate on the same
short-feature inputs.
The [partial-boundary certificate audit](join-trim-certificates.md) resolves
eighteen of those differences and adds eighty pre-split curved records, retaining
all newly exposed seam-cleanup, outer-knot and area differences. That audit's total
is 651 full matches, 32 native errors and 201 other differences in 884 cases.
The [tensor-isocurve audit](join-isocurve-certificates.md) adds 33 comparable
records, with area/outer-knot differences retained: 651 full matches, 32 native
errors and 234 other differences across 917 cases. Another 31 translated cases
have native-only evidence, explicitly excluded from comparison counts. Its shared
`surface_face` sources can use `trim_bounds: [[u0,u1],[v0,v1]]` while retaining the
complete underlying surface. This shared-input recipe uses four unit-domain UV
trim curves by default, preserving the exact inputs of the retained archives.
`trim_domains: [[t0,t1], ...]` explicitly supplies the four scalar intervals in
South/East/North/West order, before edge subdivision. This is independent of UV
control coordinates and spatial edge domains. The kernel's natural-face
constructor now follows surface-axis intervals instead; the harness builds its
declared input curves, without rewriting expected output domains or observations.

The [redundant-edge cleanup audit](join-edge-cleanup.md) adds 32 angular-tolerance
records using the same sixteen shared source artifacts at four document angles.
The cumulative total is 679 full matches, 32 native errors and 238 other
differences in 949 cases. Twelve previous cases become matches and none regress;
small-angle representation-dependent Rhino topology remains an explicit difference.

The `align` object-layout probe compares actual bounding-box, curve and line/plane alignment commands,
including retained IDs, source samples/domains, groups, layer assignment and
pre/postselection cleanup. Its shared `object_layout` module supplies fixture
ownership and recording for Align and Distribute, not the alignment algorithm.
[Alignment evidence](commands/align.md#oracle-evidence) includes bounding-box,
line/plane and [best-fit plane](plane-fit.md) commands. It retains raw curved-bound,
mesh-storage and nonunique-normal discrepancies alongside successful comparisons.
All Rhino command probes run on an owned private Xvfb display.
For `ToCurve`, `curve` is an unselected curve's index in `sources` and `selected`
must explicitly omit it. Target IDs are bound only after owned source creation;
arbitrary command text is never accepted as a target. The native adapter uses
the corresponding document UUID. The 39-case curve fixture includes all seven
native target families and retains a separate incomplete-prompt diagnostic.

The `split_edge_command` operation uses the shared owned edge-edit fixture and
history recorder, with a separate strict point-input grammar. Its
[21-case record](split-edge-provenance.json) exercises actual mouse-selected
SplitEdge commands, including full-batch duplicate failure and unchanged
endpoint-only replacements with real Undo/Redo. Native replay compares all
ordered geometry/attribute/history fields without component renumbering; see
[SplitEdge](commands/split-edge.md) for interaction and search limitations.
An additional [19 distance-constraint records](split-edge-distances.md) use
bounded interleaved typed distance/point inputs and real mouse clicks. They retain
curved Rhino inversion residuals and compare those cases at an explicitly separate
bound, without weakening the original observations or omitting numeric fields.
The later [17 snap observations](split-edge-snaps.md) retain offset feature
clicks and unsnapped controls. Fifteen model-space snap locations replay complete
geometry/history; two uncalibrated screen-only controls are explicitly unsupported.
Nonuniform curve and edge witnesses distinguish arc-length Mid from parameter Mid.
The [composite/surface snap audit](composite-feature-snaps.md) adds eleven matching
End/Mid captures, one retained uncaptured-center discrepancy and five Center
hover diagnostics. Those historical uncalibrated discrepancies remain explicit;
they are not relabeled as passing model-target or pixel replays.
The subsequent [22 calibrated Center-hover observations](center-hover-snaps.md)
record the camera at the actual point prompt. Fourteen independently computed
native captures drive complete geometry/history replay; eight empty-center misses
compare admission only. Strict optional `record_viewport`, `persistent_snaps` and
separate `pick.aim` fields keep camera input and declared model targets distinct.
Two rejected point-target hypotheses remain unchanged in the requests.
The [four two-pick one-shot records](object-snap-controls.md) add eight calibrated
captures and complete history comparisons, including restoration of persistent
modes after the first pick. Native menu/prompt lifecycle tests are separate from
these camera-calibrated core capture replays.
The [37 Mid-hover records](mid-hover-snaps.md) add 26 calibrated captures with
complete geometry/history replay and 11 mixed-mode admission-only misses. One
unsnapped control fails to split and has no Undo/Redo; that limitation is retained.
These cases cover segment/boundary hover, opposite-seam conic targets and
curve-distance ranking across competing objects and polyline segments.
The [44 polygon Center records](polygon-center-snaps.md) add 34 complete calibrated
capture/history matches and ten admission-only misses. Cases distinguish corner
averages from area/bounds centers, retain collinear/repeated curve corners,
exclude internal subdivisions of straight surface edges, and include nonplanar
curve captures alongside warped-surface misses.
The [44 circular NURBS records](circular-center-snaps.md) add 34 complete capture
replays and four admission-only misses. Four unsupported elliptical captures and
two negative-weight recognition differences remain explicit. Both outside-arc
controls fail to split; the other 42 records include actual Undo/Redo.

The shared `serde_json` dependency explicitly enables round-trip float parsing.
Standalone oracle/document builds must not depend on app or test dependencies
to enable numerical fidelity through Cargo feature unification. A package-only
regression checks saved binary32 coordinates and 10,000 binary64 round trips.

The `surface_wires` probe compares natural surface-to-B-rep topology and wire
geometry against Rhino `CreateFromSurface`/`GetWireframe`. The
[paired surface-wire record](surface-wire-frames.md) contains 12 operations;
ten pass and two translated density-3 cases remain discrepancies. Its bounded
correspondence diagnostic requires unique nearest six-sample matches to form
a bijection, then compares unchanged coordinates and topology fields.

The `trimmed_surface_isocurves` probe accepts a `TrimmedBrepFixture` and 1–64
native `[u,v]` parameter pairs. It returns samples in face, query, U/V, segment,
point order. Both engines normalize each extracted curve's domain *after*
extraction, so differing output parameter origins do not hide geometric errors.
The [paired isocurve fixtures and recorded comparison](brep-isocurves.md)
cover local and translated paraboloid disks/annuli: local cases pass, while
large-offset cases remain explicit Rhino discrepancies.

The `mesh_face_normals` probe returns one normal per stored triangle or quad.
Its [fixture](../tools/rhino_oracle/fixtures/mesh_face_normals.json) and
[Rhino 8.32 record](../tools/rhino_oracle/observations/mesh_face_normals.json)
cover mixed face types, warped quads, cyclic/reversed winding, and a translated,
uniformly scaled copy. The native regression checks independent analytic unit
directions to `4 * f64::EPSILON`, then checks exact equality with Rhino after
conversion to its `Vector3f` face-normal storage representation. Native geometry
and probe results retain double precision; the default `1e-10` comparison
threshold is tighter than this Rhino collection's storage precision.
Extreme-scale native tests are separate from these ordinary-scale Rhino records.

The [edge-split fixture](../tools/rhino_oracle/fixtures/mesh_split_edge.json)
has a [27-case Rhino 8.32 record](../tools/rhino_oracle/observations/mesh_split_edge.json).
A native replay checks exact acceptance, face indices, vertex coordinates, and
ordering for all cases, including endpoint/outside parameters, surviving fans,
orientation conflicts, and mixed triangle/quad non-manifold incidences.
For the three-face mixed incidence, full welding produces eight vertices;
partial or no endpoint welding produces 24, with eight replacement triangles
in all three cases. These dyadic-coordinate fixtures need no comparison epsilon.

The [edge-collapse fixture](../tools/rhino_oracle/fixtures/mesh_collapse_edge.json)
has a [15-case Rhino 8.32 record](../tools/rhino_oracle/observations/mesh_collapse_edge.json).
The shared mesh-edit replay checks exact acceptance, coordinates, face indices,
and ordering. Cases include empty results, triangle/quad reduction, seams,
non-manifold edges, unused vertices, and disconnected coincident endpoint fans.
The added coincident-peer case confirms that peers outside the selected edge
move to its midpoint without merging their separate raw vertices. These ordinary
coordinate records do not establish parity for subnormal midpoint rounding.

The [edge-weld fixture](../tools/rhino_oracle/fixtures/mesh_weld_edge.json) has a
[nine-case Rhino record](../tools/rhino_oracle/observations/mesh_weld_edge.json).
It exercises the actual `WeldEdge` command with selected edge subobjects,
including reversed non-manifold face order, partial seams, empty/naked selections,
unused vertices, and disjoint seams. Replay compares acceptance, removed-vertex
counts, ordered face coordinates, and vertex-to-face sharing groups exactly.
Unlike split/collapse records, this representation does not compare raw index
numbering or identify which coincident source index survives.

The [vertex-weld fixture](../tools/rhino_oracle/fixtures/mesh_weld_vertex.json) has
a [ten-case Rhino 8.32 record](../tools/rhino_oracle/observations/mesh_weld_vertex.json).
It exercises the actual `WeldVertices` command using selected topology vertices:
both seam endpoints, reversed selection order, empty/naked/already-welded
selections, a closed fan, vertex-only contact, non-manifold incidence, and two
incident seams. It uses the same face-coordinate and sharing-group representation
as edge welding, with exact replay and the same raw-index comparison limitation.

The [vertex-unweld fixture](../tools/rhino_oracle/fixtures/mesh_unweld_vertex.json)
also has a [ten-case Rhino 8.32 record](../tools/rhino_oracle/observations/mesh_unweld_vertex.json).
It covers empty/naked/already-unwelded selections, a closed fan, duplicate and
reversed selections, cube corner/all-vertex edits, a non-manifold fan, and a
fully separated triangle. Replay checks acceptance, added-vertex counts, ordered
face coordinates, and sharing groups exactly, with the same raw-index limitation.

The [angle-unweld fixture](../tools/rhino_oracle/fixtures/mesh_unweld.json) has a
[six-case Rhino 8.32 record](../tools/rhino_oracle/observations/mesh_unweld.json).
Replay checks exact raw vertex coordinates, face indices, ordering, and added
vertex counts for zero/positive flat thresholds, equal/above right-angle
thresholds, already-unwelded faces, and cube creases. Separate non-manifold probes
exposed a [face-order-dependent mismatch](mesh-unweld-nonmanifold.md), now corrected
by radial face traversal and singleton-group ordering. All 69 non-manifold cases
are enabled parity regressions, including 24 four-face permutations and six
source-vertex reorderings; the earlier two ignored tests are enabled again.
The Rhino-only `mesh_radial_topology` probe records public edge ordering and
incidence separately; its twelve cases match the native radial sorter and isolated
the original discrepancy to subsequent grouping/rebuilding.

The [edge-unweld fixture](../tools/rhino_oracle/fixtures/mesh_unweld_edge.json) has
a [19-case Rhino 8.32 record](../tools/rhino_oracle/observations/mesh_unweld_edge.json).
This probe invokes `Mesh.UnweldEdge`, not the interactive command. Besides radial
fans and closed meshes, it covers partial sharing at one or both endpoints of a
non-manifold edge. Rhino separates an endpoint only when all incident edge faces
use one raw vertex there, preserving other existing sharing groups. Disconnected
coincident contacts and unused coincident vertices do not prevent separation.
Reversed face-order and swapped-endpoint probes confirm that partial sharing is
preserved even when the first two incident faces use different raw indices.
These cases exposed and corrected native over-separation. Replay compares
acceptance, added-vertex counts, face coordinates, and sharing groups exactly;
raw-index numbering and stored normal parity are not established by this record.

The [short-curve selection diagnostic](short-curve-selection-measurement.json)
embeds four requests and responses (40 line lengths). `short_curve_selection`
creates owned line objects, runs the actual `SelShortCrv` command, and records
selected source indices, restoring the original selection and deleting only
its temporary lines. This is a Rhino-only probe; command tests replay its
measurements. The measured relative `1e-6` allowance is more specific than the
[help page's “less than” description](https://docs.mcneel.com/rhino/8/help/en-us/commands/selection_commands.htm#SelShortCrv).

The [circle follow-up](short-curve-circle-measurement.json) records 15 analytic
circles and 15 rational NURBS circles. `curve_kind` explicitly chooses the source
representation; optional `inspect` records `GetLength`, `IsShort(limit)`, and
`IsShort(limit × 1.000001)` without changing the command under test.
All analytic and NURBS cases now replay successfully. The original mismatch was:
for nominal length `0.999999`, Rhino reports length `0.9999990022994623`, yet
both shortness queries return false and the command leaves it unselected.
The previous native length-based predicate selected it. A separate
[adaptive shortness predicate](curve-shortness.md) now reproduces the observed
selection without changing NURBS geometry or accurate length measurement.
Rhino documents [IsShort](https://mcneel.github.io/rhinocommon-api-docs/api/RhinoCommon/html/M_Rhino_Geometry_Curve_IsShort.htm)
as a faster alternative to calculating length.

The [representation follow-up](short-curve-representation-measurement.json)
adds 66 classifications: midpoint knot refinement at levels 0, 1, 2, and 4;
degree elevation from 2 through 5; and a polynomial quadratic arch with exact
nominal length derived from its parabola. These preserve the underlying locus
while changing the spans or degree. One refinement changes the circle's
near-limit selection, whereas degree elevation alone does not. A fixed
three-point Gauss–Legendre estimate explains the coarse-circle rejection but
fails on the arch, where it underestimates length and would accept cases that
Rhino rejects. It is therefore not implemented as a replacement predicate.
The records also include control counts and polygon lengths, separately from
Rhino's integrated lengths and shortness results. Adaptive three-point
integration with early rejection, rather than a single fixed estimate,
now passes all 66 classifications. Broader curve families, extreme shortness
thresholds, and polycurve-specific boundary behavior still need oracle coverage.

The versioned Python oracle API runs identical JSON geometry and document-state
batches in a native release build of Viboceros and Rhino 8, recursively checks
results, and reports per-operation timings.

`control_point_prompt_rhino_only.json` is a diagnostic exception: it exercises
Rhino's Curve point prompt and records resulting control points, rather than
running the geometry constructor. Run it with `run_headless.sh rhino
tools/rhino_oracle/fixtures/control_point_prompt_rhino_only.json --timeout 300`;
the native oracle does not implement this operation. Its [recorded response](control-point-prompt-measurement.json)
checks adjacent-point rejection; see [typed point input](point-input.md).
The [threshold follow-up](control-point-threshold-measurement.json) embeds three
additional requests and their responses (30 cases), establishing a fixed,
coordinate-wise `2^-32` comparison for the tested near-origin controls.
The app test `recorded_rhino_curve_threshold_sequences_match_interactive_completion`
replays all 30 stored requests with their document tolerances and checks accepted
controls, output degree, closure, and control locations against Rhino's responses.
JSON uses round-trip float parsing so adjacent binary64 boundary values remain distinct.

`interpolation_point_prompt_rhino_only.json` is a separate eight-case diagnostic
using the same coordinate whitelist and private construction-plane wrapper for
InterpCrv (open cubic, chord knots). Explicit command rejections become
`command_succeeded: false` records; unexpected failures still fail the batch.
Its [recorded response](interpolation-point-prompt-measurement.json) exposed an
open-curve point-tolerance discrepancy that is now corrected. An app test replays
the six successful cases and compares control points within `1e-9`; two Rhino
solver rejections remain diagnostic. Run it with `run_headless.sh rhino`, not
the native comparison runner.

`interpolation_closure_prompt_rhino_only.json` checks ordinary three-point
closure using `_Close` and `_Sharp _Close`, in that order, in a fresh private
Rhino session. The [recorded response](interpolation-closure-prompt-measurement.json)
replays through app completion and matches both sets of control points within
`1e-9`. This is a baseline, not a near-seam tolerance measurement or a test of
arbitrary persistent option state. In this Rhino build, `_Sharp` alone toggles
the option and leaves the point prompt active. Exploratory `_Sharp=_No _Close`
and `_Sharp=_Yes _Close` both produced the sharp curve; those spellings are not
used by the retained fixture. Closed prompt probes are restricted to InterpCrv.

The [closure tolerance follow-up](interpolation-closure-tolerance-measurement.json)
embeds two requests and their responses: a `0.001` interior or seam-adjacent
offset, smooth or sharp closure, at absolute tolerances `0.01` and `1e-9`.
Both batches use the same operation order in fresh private Rhino sessions.
All eight results replay through app completion with control-point error at
most `1e-9`. Command interpolation now retains these distinct points instead
of rejecting or merging them using model tolerance. This does not measure
the exact duplicate/near-zero seam threshold or automatic closing behavior.
The separate [auto-close audit](interpolation-auto-close-measurement.json)
embeds four requests and responses (18 cases) that establish an inclusive
Euclidean `1.490116119385e-8` prompt-closing threshold near the world origin.
The `PointOnly` diagnostic ending sends no Enter or closure option: its exact
seam case confirms completion from the point itself. Use that ending only to
probe suspected automatic completion; non-closing input may wait until the
client timeout. App regressions replay every measured geometry and distinguish
automatic completion from the cases that still require Enter.

The [translated follow-up](interpolation-translated-auto-close-measurement.json)
records `periodic` separately from `closed` for 13 cases. At X=`±1e6`, curves
with endpoint gaps of `2e-8` are closed under Rhino's coordinate-relative
topology test but are not periodic; a two-point return also produces a closed,
non-periodic result. App replay must not infer periodicity from closed state.
New InterpCrv prompt measurements include both properties; older responses
still check their recorded control geometry without inventing periodicity data.
A [two-point no-Enter probe](interpolation-two-point-auto-close-measurement.json)
confirms automatic completion with non-periodic output. Consequently neither
`closed` nor `periodic` alone establishes when the point prompt finished; the
`PointOnly` ending directly tests completion. The app now auto-closes two-point
returns with a non-periodic cubic instead of requiring a third collected point.

The [degree-one follow-up](interpolation-degree-one-auto-close-measurement.json)
records six Enter-completed sequences, all replayed through preview/completion
with control errors at most `1e-9`. Separate `PointOnly` probes for two and three
collected points followed by `w1e-8,0,0` both waited until the 100-second client
timeout. The owned private windows were inspected while live and still showed
the next-point prompt after accepting that input; these are prompt observations,
not successful geometry responses. Separate [exact-return probes](interpolation-degree-one-exact-close-measurement.json)
do complete without Enter for both point counts. Degree-one completion at a
nearby endpoint therefore differs from completion at exact equality, even when
its eventual endpoint is reconciled to the start.
A further no-Enter batch timed out during its first `1e-10` offset; its subsequent
`2^-32` case was not reached and must not be treated as measured.

The [degree-one reconciliation boundary audit](interpolation-degree-one-seam-boundary-measurement.json)
contains nine successful Enter-completed cases: equality and the next float
around `1.490116119385e-8`, a longer diagonal, two model tolerances, and three
translated endpoints at X=`1e6`. It confirms the current distance-based
reconciliation rule within control-point error `1e-9`. Replay also compares
the cached preview to completion for every still-active open prompt; this
does not establish a tighter automatic-completion threshold.

Standard geometry/command batches apply the request's absolute, relative, and angular tolerances to
Rhino's active document and restores its previous settings on success or failure.
This matters for command macros, which read document settings rather than an API
tolerance argument. See [Rhino's document tolerance API](https://developer.rhino3d.com/api/rhinocommon/rhino.rhinodoc/modelabsolutetolerance).
Older command comparisons made before this synchronization need revalidation.

`mesh_split_picking.json` is a **Rhino-only diagnostic**, not yet a native compare
operation. It uses the same owned-window idle-click mechanism with three disjoint
meshes, then invokes the actual `SplitDisjointMesh` command. Sixteen cases cover
ordinary and overlapping groups, ordered bridge memberships, hidden/locked
objects, hidden/locked layers, and connected meshes mixed with splittable peers.
Run it with:

```sh
tools/rhino_oracle/run_headless.sh rhino tools/rhino_oracle/fixtures/mesh_split_picking.json --timeout 240
```

The checked-in `tools/rhino_oracle/observations/mesh_split_picking.json` records
Rhino 8.32.26160.13001 on 2026-09-12: source identity, selection, object mode,
group memberships, layer modes, vertices and face counts, including untouched peers.
A native command regression compares those 16 recorded output sets exactly;
see the [verified fields and remaining limits](commands/meshes.md). The oracle CLI
still has no native `mesh_split_picking` operation; this is an offline observation
comparison in `cargo test -p viboceros-command split_disjoint_mesh_matches_live`.
Deleted sources are removed from cleanup tracking; surviving original and newly
created mesh IDs are explicitly tracked and cleaned up in the private document.

`mesh_explode_picking.json` is another Rhino-only diagnostic, using the same 16
mesh setups but invoking `Explode`. Run it with the preceding `rhino` command,
substituting this fixture name. Its checked-in observations record retained
restricted decomposed sources as **unselected**, unlike SplitDisjointMesh.
Connected sources remain selected in both commands. Python tests compare their
output records, allowing only the command success field name and decomposed
original-source selection difference. One mixed locked-connected case per
command also records selection and identity retention after actual Undo/Redo.
Those history selections are checked separately, including the unchanged peer.
The shared native test adapter in `mesh_decomposition_tests.rs` compares both
commands against their respective recorded output sets; run
`cargo test -p viboceros-command mesh_decomposition_tests`.
See the [verified native Explode behavior and limits](commands/editing.md).

`group_picking.json` is a dedicated, untimed three-line fixture: an idle-event
worker returns control to Rhino's normal UI loop, then the host clicks projected
line locations in the newly owned window and acknowledges each click atomically.
No Enter or selection command substitutes for a mouse pick. Its 52 exact checks
cover ordered group selection, hidden/locked peers and layers, and subsequent
Move commands. Run it as a separate batch with `run_headless.sh` (one iteration);
it cannot be mixed with synchronous geometry operations. See [groups](groups.md).
The idle worker guards against event-loop re-entry and finalizes only once.
Cleanup attempts every owned resource even after a failure, publishes cleanup
errors, and leaves process termination to the owned-window client fallback if
Rhino cannot exit. Fault-injection tests cover callback detachment, setup,
disposable attributes/layers, cleanup, logging, and exit failures.

`selection_recall.json` checks 84 SelPrev sequences with explicit/remembered
options, changed groups, and deleted objects. `selection_recall_picking.json`
uses the dedicated idle worker for 52 recalls after real group picking. These
136 exact comparisons distinguish recall from ordinary group expansion and
verify visibility/lock filtering. See [selection recall](selection-recall.md).
`last_selection.json` adds 32 exact SelLast sequences after idle Move, checking
group/mode filtering, remembered options, and empty-layer creation. The separate
16-case `last_selection_history.json` and 54-case `deletion_recall.json` now
match complete traces, including immediate Undo/Redo selection. These use the
same owned-window idle worker. `undo_selection.json` adds 10 isolated command
traces for Move, Delete, Explode, DeleteFaces, and ExtractMeshFaces; it checks
whole-object selection, source existence, object/output counts, and mesh face
counts after commands. See [history selection](history-selection.md).
Repeated picks at identical coordinates do not wait for a new mouse-motion event;
input failures report the case/window and progress without acknowledging a click.

`point_input.json` compares 19 [typed-coordinate sequences](point-input.md)
against Rhino's actual Polyline prompt in world, Front, Right, shifted, and
oblique construction planes. It checks unrounded resulting vertices, not a
second copy of the parser. The probe restores the plane and selection and deletes
only its own output objects; command execution is untimed.
`point_input_diagnostics.json` retains a large-scale exact-quadrant discrepancy,
not a passing Rhino reference at `1e-12`.

`plane_primitives.json` checks 57 Circle/Polygon/Rectangle/MeshPlane/Box/MeshBox
command results on construction planes. Complete boundary geometry is checked
independently of Box/MeshBox layout; ten raw layout probes remain in
`plane_primitives_representation.json`. See [comparison details](construction-planes.md).
The [two-point circle fixture](../tools/rhino_oracle/fixtures/circle_two_point.json)
checks six actual `_Circle _2Point` results against
[Rhino 8 observations](circle-two-point-rhino-reference.json), including
reversed diameter endpoints, oblique diameter, and tilted CPlane.
The [Arc Center angle fixture](../tools/rhino_oracle/fixtures/arc_center_angle.json)
compares six actual `_Arc _Center` commands with
[Rhino 8 sampled curves](arc-center-angle-rhino-reference.json), including a
major arc, capped sweep, negative sweep, and rotated construction plane. Native
samples and domains agree within `1e-10`.
The [bare Arc fixture](../tools/rhino_oracle/fixtures/arc_default.json) confirms
Rhino's center-first default through three typed angles and two actual endpoint
clicks. The [StartPoint ThroughPoint fixture](../tools/rhino_oracle/fixtures/arc_start_through_point.json)
checks the explicit three-point path, including a major and tilted arc.
[Default](arc-default-rhino-reference.json) and
[through-point](arc-start-through-point-rhino-reference.json) sampled curves and
domains match native results within `1e-10` when endpoint sweep is explicit.
The [Arc Center Length fixture](../tools/rhino_oracle/fixtures/arc_center_length.json)
compares five signed-length `_Arc _Center` commands against
[live Rhino 8 results](arc-center-length-rhino-reference.json), including a
rotated construction plane and both signs of length beyond one circumference.
Native curve samples and domains agree within `1e-10`.
The [Arc Center endpoint fixture](../tools/rhino_oracle/fixtures/arc_center_endpoint.json)
automates eight actual Rhino viewport clicks at Point osnaps with a public
`_Arc _Center` prompt and compares their complete sampled curves and domains
to explicitly directed native arcs within `1e-10`, including a construction
plane with its normal reversed. The
[single south pick](../tools/rhino_oracle/fixtures/arc_center_endpoint_south_alone.json)
produces a clockwise 90° arc, while the same pick after another operation in
the batch produces a counterclockwise 270° arc. Their
[batch](arc-center-endpoint-rhino-reference.json) and
[single-pick](arc-center-endpoint-south-alone-rhino-reference.json) records
show that interaction history affects the sweep; cursor travel appears to be
the deciding factor.
The [Arc Center Midpoint fixture](../tools/rhino_oracle/fixtures/arc_midpoint.json)
compares 17 typed angle/length commands and actual endpoint clicks against
[Rhino 8 sampled curves](arc-midpoint-rhino-reference.json). It covers
positive and negative sweeps, the 360° boundary, off-radius endpoint picks,
and a reversed construction-plane normal. Native curves and domains agree
within `1e-10` for explicitly directed endpoint picks. A separate same-radial
endpoint probe was rejected by Rhino and is rejected by the native command.
The [Arc StartPoint Direction fixture](../tools/rhino_oracle/fixtures/arc_start_direction.json)
compares five typed Rhino direction constructions with
[live sampled curves](arc-start-direction-rhino-reference.json): both turn
directions, a major arc, an oriented construction plane, and an off-plane
endpoint. Native samples and domains agree within `1e-10`.
The [Arc StartPoint Center fixture](../tools/rhino_oracle/fixtures/arc_start_center.json)
compares ten actual Rhino commands and viewport endpoint picks with
[live sampled curves](arc-start-center-rhino-reference.json), including signed
angles and lengths, major and quarter arcs, and reversed construction-plane
normal. Native curves and domains agree within `1e-10` when endpoint sweeps
are made explicit.
The [three-point circle fixture](../tools/rhino_oracle/fixtures/circle_three_point.json)
checks four actual `_Circle _3Point` results against
[Rhino 8 observations](circle-three-point-rhino-reference.json), including
reversed point order and off-CPlane geometry. Complete sampled curve records
and domains agree within `1e-10`; command execution is untimed.
The [three-point Radius fixture](../tools/rhino_oracle/fixtures/circle_three_point_radius.json)
checks four numeric-radius orientations against
[Rhino 8 records](circle-three-point-radius-rhino-reference.json). The
[picked Radius fixture](../tools/rhino_oracle/fixtures/circle_three_point_radius_picks.json)
checks on-plane and off-plane radius locations against
[live results](circle-three-point-radius-picks-rhino-reference.json).
The [circle size fixture](../tools/rhino_oracle/fixtures/circle_size_options.json)
checks Diameter, Circumference, and Area against
[Rhino 8 numeric results](circle-size-options-rhino-reference.json). The
[picked size fixture](../tools/rhino_oracle/fixtures/circle_size_picks.json)
checks Rhino's distinct point behavior for the same three options against
[live results](circle-size-picks-rhino-reference.json).
The [Vertical fixture](../tools/rhino_oracle/fixtures/circle_vertical.json)
checks world, off-plane, and rotated-CPlane direction picks against
[Rhino records](circle-vertical-rhino-reference.json). A separate
[numeric-radius fixture](../tools/rhino_oracle/fixtures/circle_vertical_numeric.json)
uses a direction point beyond the fixed radius and matches its
[Rhino result](circle-vertical-numeric-rhino-reference.json).
The [Orientation fixture](../tools/rhino_oracle/fixtures/circle_orientation.json)
checks five normal directions and construction planes against
[Rhino records](circle-orientation-rhino-reference.json). The
[radius-point fixture](../tools/rhino_oracle/fixtures/circle_orientation_picks.json)
checks on-plane and projected off-plane picks against
[live results](circle-orientation-picks-rhino-reference.json). The
[picked-size fixture](../tools/rhino_oracle/fixtures/circle_orientation_size_picks.json)
checks off-plane Diameter, Circumference, and Area input against
[live results](circle-orientation-size-picks-rhino-reference.json).

`plane_transforms.json` checks 140 actual transform commands using four affinely
independent point witnesses per operation, including copied/original identity
and selection. Four parallel-reference Shear cases remain explicit diagnostics
in `plane_transform_diagnostics.json`; see [plane transforms](plane-transforms.md).

`interface_commands.json` checks 208 untimed display/snapping transitions across
eight initial states against Rhino's actual commands. The native probe shares
the GUI's parser and state reducer. The worker restores application settings
and all viewport modes; see [interface controls](commands/interface.md).
`drafting_aids.json` adds 22 live Ortho, Planar, CPlane Z, and Ortho angle
transitions. Its [retained Rhino response](../tools/rhino_oracle/observations/drafting_aids.json)
replays against the native reducer with a maximum angular difference of
`7.1e-15` degrees. RhinoCommon reports the angle in radians; the probe records
degrees. Cursor positions still need a separate live comparison.

`construction_planes.json` checks 129 actual plane edits/history transitions.
`construction_plane_input.json` checks 24 transparent CPlane commands inside
Polyline prompts, including local and relative coordinate continuation.
See [construction-plane editing](cplane.md) for accuracy and remaining options.

`plane_arrays.json` checks 64 actual rectangular/linear/polar array commands,
including CPlane axes, tight curve extents, signed Fill lengths, zero-spacing
cells, selection, and groups. Initial scripted 3D Fill-height discrepancies
remain in `plane_array_diagnostics.json`; see [plane arrays](plane-arrays.md).
`curve_bounds.json` checks 16 timed tight-curve-box queries. A negative-gauge
Rhino discrepancy remains in `curve_bounds_diagnostics.json`, separate from
passing references; see [bounds policy and measurements](curve-bounds.md).
`surface_bounds.json` adds 25 passing timed surface-box queries.
`surface_bounds_diagnostics.json` retains four inaccurate Rhino boxes, with
untimed 41×41 `sample_grid` witnesses recorded separately as `sample_bounds`.
`surface_array_bounds.json` compares 32 actual surface/mixed-source arrays;
24 pass and eight retain placement discrepancies at `1e-8`.
See [surface bounds](surface-bounds.md) for coverage and limitations.
`parameter_curve_bounds.json` compares 20 exact surface images against
independently supplied spatial reference curves; a signed-rational Rhino box
discrepancy remains in `parameter_curve_bounds_diagnostics.json`.
`trim_boundary_bounds.json` adds six exact B-rep boundary comparisons.
These untimed probes retain direct surface-image samples and do not imply
complete trimmed-face bounds; see [trim-boundary bounds](trim-boundary-bounds.md).
`trimmed_brep_bounds.json` separately compares 11 complete B-rep boxes, with
five retained box/trim-evaluation diagnostics in
`trimmed_brep_bounds_diagnostics.json`; see [trimmed-face bounds](trimmed-face-bounds.md).
`trimmed_brep_array_bounds.json` exercises 32 actual arrays of single- and
multi-face trimmed B-reps, recording every face domain and trim-image samples.
`bounding_box.json` checks 57 actual BoundingBox commands, including output
corners/topology, groups, reports, and World/CPlane orientation.
`bounding_box_diagnostics.json` retains 26 curved-bound, thin-geometry, and
atomicity differences. See [bounding boxes](commands/bounding-box.md), including
why raw Rhino script success flags are not used for report-only commands.
`distribute.json` adds 188 actual distribution commands with signed spacing,
World/CPlane directions, bound tie ordering, retained source identities, and
top-group membership rules. Six analytically checked curved-bound placement
differences remain in `distribute_diagnostics.json`; see
[Distribute](commands/distribute.md). Its batch preselection guard avoids
leaving Rhino in an interactive object-selection prompt.

`group_memberships.json` contains 56 comparisons of ordered object memberships and the reverse
group-member table after every fixture step. It exercises membership edits,
deletion, nested/partial `Ungroup` and `UngroupAll`, copied-group allocation,
decomposition, and older-group distribution. IDs are compared as retained-source
flags; domains, sampled geometry, and selection are checked separately.
Only automatic `Group`-plus-digits names are canonicalized, by numeric order:
the reused Rhino document reserves those names across cleaned-up operations.
Explicit names and incorrectly styled copy names remain literal. This does not
establish absolute deleted-name allocation history; see [groups](groups.md).
The former ConvertToBeziers diagnostic now passes and is part of the regular
group fixture; ordinary Delete also retains empty definitions. The separate
`bezier_conversion.json` compares complete output NURBS control nets, source
identity, domains, sampled geometry, output creation order, current/source layers,
colors, names, groups, and selection after explicit deletion choices; see
[Bézier conversion](commands/beziers.md). A separate point-cloud-cycle probe uses
the same preselection workflow as the native command; the previous in-command
SelID workflow did not.

`single_span_conversion.json` adds 78 actual surface conversions covering
directional strips, Both patches, no-ops, trims, unclamped/periodic structure,
mixed sources, and U/V toggles. `conversion_sessions.json` adds eight self-seeded
sequences (39 steps) checking independent remembered deletion/direction choices,
partial option changes, no-op acceptance, and undo. They share the full conversion
recorder and owned-object cleanup. See [single spans](commands/single-spans.md)
for the explicit omitted-outer-knot codec and [option memory](command-options.md)
for lifetime and bootstrap limitations. These command probes are untimed.

`nurbs_conversion.json` compares 78 actual `ToNURBS` conversions, including meshes
and already-NURBS no-ops. It additionally records curve representation kind and
full source curve/surface definitions so identity-preserving replacement cannot look like a
no-op. B-reps include topology and full underlying surface nets. Six
`nurbs_conversion_sessions.json` sequences add 39 steps; unlike single-span
conversion, `ToNURBS` no-ops do not accept new choices. See
[ToNURBS](commands/to-nurbs.md) for parameters, chronological renewal, and limits.

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
[MeshToNURB](commands/mesh-to-nurb.md) for coverage and selection-path limits.

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

With Rhino installed through the configured Wine/FEX launcher, run the core fixture:

```sh
python3 -m tools.rhino_oracle compare \
  tools/rhino_oracle/fixtures/core.json \
  --absolute-epsilon 2e-12 --relative-epsilon 1e-12
```

For trimmed surfaces with non-affine parameterization:

```sh
tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/surface_split_nonaffine_trimmed.json \
  --absolute-epsilon 1e-9 --relative-epsilon 1e-10
```

That fixture sets `sample_trim_geometry=true`: each non-isoparametric trim is
compared at 65 equal-arc-length stations in UV, including both endpoints, with
both UV coordinates and surface-evaluated 3D positions recorded. This compares
independently fitted curves whose knot counts and parameter speeds differ.
Topology, edge domains, underlying surfaces, attributes, groups, and selection
are still compared. The default retains complete trim control/knot comparisons.
Sampling is bounded evidence of geometric agreement, not a continuous error proof.

The `trimmed_mass_properties.json` fixture checks [nonplanar trimmed-face area
and signed volume](mass-properties.md). Its `trimmed_surface_mass_properties`
operation supplies exact spatial and UV loop curves, an underlying surface, and
an optional cap surface sharing those boundaries. Rhino's public B-rep topology
API builds the same input geometry as the native probe, avoiding changes to the
input from a separate trim-fitting operation. Numerical API calls are timed;
the native probe also checks `Area` and `Volume` command results and document state.
The [35 trim-domain frame cases](mass-properties.md#trim-domain-regression-audit)
vary trim parameters without changing UV coordinates or spatial edges. Their
records include complete trim definitions, extracted outside the timed section,
so implicit input normalization cannot masquerade as large-domain coverage.

The `curve_area.json` fixture checks eight [enclosed curve-area cases](curve-area.md)
against analytic references and Rhino's public API. The retained strict comparison
has two conic discrepancies; it is not an all-passing parity claim.

The `non_manifold_selection.json` fixture checks the actual
[`SelNonManifold` command](non-manifold-selection.md) on mesh and B-rep objects,
with separate unselected controls and additive-selection cases.

The `polycurve.json` fixture exercises [exact piecewise curves](polycurves.md).
It preserves and compares segment definitions and domains, then tests reversal,
trimming, splitting, length-based reparameterization, derivatives, and division.
The `curve_division_contract.json` fixture checks open/closed division endpoint
rules separately, including the `include_ends=false` case.
`polycurve_document.json` exercises actual extraction and explode commands,
duplicate comparison, and 3DM round-trip segment definitions; the native side also
checks undo. See [polycurve documentation](polycurves.md) for compatibility boundaries.

`edge_surface.json` and `edge_surface_command.json` compare [Coons edge surfaces](edge-surfaces.md)
through both the geometry API and document command, including full NURBS
coefficients, direct samples and singular-side topology. The zero-weight-control
case uses exact comparison-degree elevation while retaining original-surface
samples. `edge_surface_3dm_interchange.json` separately checks both readers of the
same native-written files, without changing the serialized basis.

`point_grid.json` checks [point-grid surface construction](point-grid-surfaces.md),
including native domains, full coefficients, active samples, and actual
construction-site samples (including closed-direction continuation).
`point_grid_command.json` checks every output face and retained point-cloud
state. Higher degrees and same-file 3DM checks have separate fixtures.
`point_grid_high_degree_diagnostics.json` deliberately retains ill-conditioned
closed-grid residuals and a folded-shell orientation mismatch; it is not a
passing full-record reference. See the point-grid page for limits and measured
differences.

`polycurve_native.json` checks native analytic evaluation and segment classes
through reversal, trim/split, and transforms. `polycurve_analytic_editing.json`
checks analytic endpoint policy inside composites. These fixtures include both
native derivative samples and NURBS definitions: rational conversion alone cannot
verify angular parameterization. Exact nonuniform transforms explicitly prepare
Rhino's polycurve with `MakeDeformable`; direct API shear has different behavior.
`polycurve_native_document.json` compares extraction, recursive Explode, duplicate
checks, and 3DM round trips on native mixed composites in both directions.

`curve_native_parameters.json` covers the shared [native parameter contract](curve-parameters.md)
for all seven curve families. It compares domains, first/second derivatives,
tangents, equal-length division parameters, reversal, transforms, and NURBS definitions.
`curve_native_editing.json` extends those records with native trims, cyclic subcurves,
splits, seam relocation, periodic curves, and unequal rational weights. See
[domain-editing validation](curve-domain-editing.md) for the separate geometry and
Rhino length-inversion comparison limits.

`nurbs_rational_jets.json` adds 23 degree-one and higher rational derivative,
weight-scale, and seam comparisons. `nurbs_translated_jets.json` is a separate
diagnostic exposing Rhino's large-coordinate cancellation, not a passing reference
at the ordinary epsilon. Its `curve_native` operations set `differential_only=true`
to omit length/division calls, isolating derivative evaluation from Rhino's failed
translated length inversion. See [rational numerical validation](nurbs-numerics.md)
for analytic checks, observed errors, and remaining kernel limits.

`curve_sided_evaluation.json` compares 45 native left/right limit cases via
`sided_parameters`, including composite child knots and stationary endpoints.
See [one-sided evaluation](curve-sided-evaluation.md) for the exact-limit contract,
Rhino probe construction, and native-only full-order knot tests.

`curve_frames.json` and `curve_frames_multispan.json` compare 22 sets of
[rotation-minimizing frames](curve-frames.md), keeping points/tangents separate
from the relative-rotation accuracy checks. `curve_frames_diagnostics.json`
isolates seven known Rhino availability, large-coordinate, and corner-query
differences; it is not a passing reference. `curve_array.json` now records
unrounded endpoints. `curve_array_corner_diagnostics.json` is a separate,
untimed actual-command probe exposing Rhino's query-dependent corner twist.

`sweep1.json`, `sweep1_command.json`, `sweep1_multisection.json`, and
`sweep1_weights.json` check 49 [one-rail sweep cases](sweep1.md)
through Rhino's public refitted API and actual unrefitted/refitted commands.
Each compares 135 unrounded closest-point results, including 54 off-surface
queries. Command probes compare output geometry, not full document state, and
are untimed. The actual-command macro uses Rhino's script options
`Style`, `ShapeBlending`, and `RefitRail`, not dialog property labels. It rejects
new `Unknown command` history even if `RunScript` reports success.
`sweep1_curved_blend.json`, `sweep1_diagnostics.json`,
`sweep1_basis_diagnostics.json`, and `sweep1_weights_diagnostics.json` retain
13 discrepancies; these are not passing references at `1e-6`.
Some are construction differences, others closest-point
differences, including two independently verified nonminimal Rhino answers.
Weight probes distinguish raw relative scales, Euclidean versus homogeneous
placement, and normalization after placement. Signed-control sweeps additionally
check a sufficient positive-denominator bound; no comparison epsilon was widened.

All `loft.json` and `loft_command.json` cases now enable `sample_geometry`:
289 unrounded normalized-UV grid points supplement each output face's full
coefficients. `loft_end_weights.json` adds two normalization checks;
`loft_end_weights_diagnostics.json` retains three public Rhino endpoint-drift
cases, not passing references at absolute `1e-9`, relative `1e-12`. Native
analytic checks preserve the original ruled profiles. See [Loft](loft.md) and
[endpoint-weight numerics](nurbs-numerics.md#endpoint-weight-normalization).

`surface_jets.json` checks 20 [rational surface differential cases](surface-evaluation.md),
including second partials, parameter-domain changes, continuation, and exact
quadrant limits. `surface_translated_jets.json` separately diagnoses large-coordinate
cancellation and is not a passing reference at the ordinary epsilon.

`surface_curvature.json` checks 27 [surface-curvature API cases](curvature.md),
including one-sided knot limits and singular-pole availability. Spatial shape
operators avoid arbitrary principal-direction signs and umbilic axes.
`surface_curvature_umbilic.json` isolates Rhino's roughly `7.45e-9` repeated
eigenvalue split on a sphere and uses its own `1e-8` absolute comparison limit.
`curvature_command.json` checks 17 actual command cases, including permanent
markers, source geometry and attributes, and retained selection. These command
records verify that a measurement was reported; the API records separately
check unrounded numerical results.

`curve_surface_morph.json` separates [direct point-map and curve-fit validation](curve-morphing.md).
Its eight direct maps match at absolute `2e-12`; fitted outputs use `1e-5` because
Rhino's observed fit error exceeds the requested `1e-9`. Native fits are separately
asserted against their direct maps at the requested tolerance. `surface_orient.json`
now compares unrounded curve samples and document state, not rounded control nets.
`surface_surface_morph.json` adds five [surface-fitting cases](surface-morphing.md)
with 1,089 offset-grid direct-map and fitted samples per case; native fit errors
are independently checked at each case's requested tolerance.
`brep_surface_morph.json` adds four [trimmed B-rep morph cases](brep-morphing.md),
checking shared topology, direct maps, and fitted geometry separately. Edge
samples use closest-point correspondence because Rhino changes their parameter
speeds; native fits additionally retain original-parameter accuracy checks.
`brep_mesh_boundaries.json` checks [open and closed shared-boundary meshing](brep-meshing.md)
on five independently refined box-face subsets. It compares boundary geometry
and incidence properties, not identical mesh element counts, and separately
validates native `Mesh` command results.

`curve_3dm_interchange.json` checks Rhino's reading of actual Viboceros-written
files, including decomposed full-order knots and all visibility/locking states.
Unlike independent-construction probes, `compare` supplies shared private artifact
paths and cleans them after both readers finish. See [curve interchange](curve-3dm-interchange.md).
`brep_3dm_interchange.json` uses the same shared-artifact workflow for eight
[nonlinearly morphed B-reps](brep-3dm-interchange.md), comparing complete NURBS
definitions, topology, tolerances, geometry samples and post-import mesh topology.
`rational_3dm_range.json` checks four extreme-coordinate/weight cases using zero
absolute epsilon and relative `1e-12`; see [numeric range validation](rational-3dm-range.md).

`curve_parameter_map.json` adds both native/rational parameter maps to those records.
`curve_native_cutting.json` tests cutting-object Split and Trim on all curve families,
including wrapped outputs, seam hits, and projected cuts. `curve_native_extrusion.json`
checks profile-domain preservation. See [native cutting validation](curve-cutting.md)
for the 75 cases, numeric limits, and the separate ill-conditioned legacy Trim
tangent comparison. Curve-cut command records sort by native domain, not rounded
world-space endpoint coordinates.

`curve_join_close.json` compares 39 mixed joining and closure cases, including
full NURBS definitions, retained intervals, representation, and length. It tests
the batch `JoinCurves` API separately from actual `Join`/`CloseCrv` commands.
Join command records also compare source identity, names, and overlapping groups;
document results are sorted by their unique source names. The native command
path executes the real command registry. See [curve editing](curve-editing.md).

To keep Wine/Rhino completely off the active desktop, use the isolated Xvfb
runner (requires `Xvfb`, `xvfb-run`, and `i3`):

```sh
tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/parabola_three_point.json \
  --absolute-epsilon 2e-12 --relative-epsilon 1e-12

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/helix.json \
  --absolute-epsilon 2e-5 --relative-epsilon 1e-12

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/spiral.json \
  --absolute-epsilon 3e-5 --relative-epsilon 1e-12

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/swept_spiral.json \
  --absolute-epsilon 2e-7 --relative-epsilon 1e-12

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/catenary.json \
  --absolute-epsilon 2e-8 --relative-epsilon 1e-12

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/curve_through.json \
  --absolute-epsilon 2e-12 --relative-epsilon 1e-12

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/curve_tween.json \
  --absolute-epsilon 5e-8 --relative-epsilon 1e-12

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/curve_tween_short_samples.json \
  --absolute-epsilon 2e-12 --relative-epsilon 1e-12

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/curve_fit.json \
  --absolute-epsilon 2e-12 --relative-epsilon 1e-12

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/curve_rebuild.json

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/curve_make_uniform.json

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/surface_make_uniform.json

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/make_uniform_commands.json

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/curve_insert_knot.json

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/surface_insert_knot.json

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/insert_control_point.json

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/curve_change_seam.json

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/surface_change_seam.json

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/reparameterize.json

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/reparameterize_automatic.json \
  --relative-epsilon 1e-8

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/surface_extend.json

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/surface_extend_length.json

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/surface_extend_line.json

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/curve_extend_line.json \
  --absolute-epsilon 5e-14 --relative-epsilon 1e-12

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/curve_extend_command.json \
  --absolute-epsilon 2e-8 --relative-epsilon 1e-11

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/curve_extend_arc.json \
  --absolute-epsilon 2e-11 --relative-epsilon 2e-12

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/curve_extend_arc_command.json \
  --absolute-epsilon 2e-8 --relative-epsilon 1e-11

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/curve_extend_boundary_command.json

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/surface_shrink.json \
  --absolute-epsilon 3e-8 --relative-epsilon 1e-10

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/curve_subcurve.json

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/curve_split.json

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/curve_split_command.json

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/surface_split_isocurve_command.json

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/surface_split_cutting_command.json

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/curve_intersect_command.json \
  --absolute-epsilon 1e-9 --relative-epsilon 1e-11

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/curve_surface_intersect_command.json \
  --absolute-epsilon 1e-9 --relative-epsilon 1e-11

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/curve_brep_intersect_command.json \
  --absolute-epsilon 1e-9 --relative-epsilon 1e-11

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/surface_surface_intersect_command.json \
  --absolute-epsilon 1e-9 --relative-epsilon 1e-11

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/surface_brep_intersect_command.json \
  --absolute-epsilon 1e-9 --relative-epsilon 1e-11

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/brep_brep_intersect_command.json \
  --absolute-epsilon 1e-9 --relative-epsilon 1e-11

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/curve_trim_command.json \
  --absolute-epsilon 5e-6 --relative-epsilon 1e-11

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/surface_direction_edit.json

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/remove_knot.json \
  --absolute-epsilon 2e-11 --relative-epsilon 2e-12

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/remove_control_point.json \
  --absolute-epsilon 1e-10 --relative-epsilon 1e-10

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/remove_multi_knot.json

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/make_non_periodic.json

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/curve_make_periodic.json \
  --absolute-epsilon 2e-11 --relative-epsilon 2e-12

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/curve_make_periodic_degrees.json \
  --absolute-epsilon 2e-11 --relative-epsilon 2e-12

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/surface_make_periodic.json \
  --absolute-epsilon 2e-11 --relative-epsilon 2e-12

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/curve_change_degree.json \
  --absolute-epsilon 2e-11 --relative-epsilon 2e-12

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/surface_change_degree.json \
  --absolute-epsilon 2e-11 --relative-epsilon 2e-12
```

The same workflow is importable for instrumentation and tests:

```python
from tools.rhino_oracle import OracleClient, load_request

report = OracleClient().compare(load_request("tools/rhino_oracle/fixtures/core.json"))
assert report.passed
```

Set `VIBOCEROS_RHINO_LAUNCHER` to use another launcher. McNeel's documented
`/runscript` startup interface is tried first; this project's Wine path also
has an owned-window fallback that requires `wmctrl` and `xdotool` and never
targets a pre-existing Rhino process. Set `VIBOCEROS_RHINO_UI_FALLBACK=0` to
disable it. The `viboceros` and `rhino` modes run either side independently.
On Linux the driver reports a Rhino process that exits before publishing a
response after a two-second exit grace period. If no owned Rhino process ever
appears, it reports startup failure after 30 seconds; an active process keeps
the full requested probe timeout.

### SetView CPlane camera probe

The bounded `view_camera_probe` operation uses public RhinoCommon viewport
properties to record camera direction, up vector, location, target, projection,
and construction plane after all six `SetView CPlane` directions in parallel and
perspective viewports. It restores the original view projection, target, name,
CPlane, and model aid settings. Run the Rhino side with:

```sh
tools/rhino_oracle/run_headless.sh rhino tools/rhino_oracle/fixtures/view_camera_cplane.json --timeout 300
```

Save the JSON response and compare its camera and CPlane values with the current
Viboceros camera rule:

```sh
python3 -m tools.rhino_oracle.view_camera_probe tools/rhino_oracle/fixtures/view_camera_cplane.json rhino-camera-response.json
```

The comparator checks the six orientations, CPlane axes and origin, camera
target, projection, and perspective distance. Parallel camera location is
diagnostic because Viboceros has no finite parallel camera location. The camera
probe has no native oracle operation yet, so `compare` is unavailable for this
fixture. The current Wine launcher crashed during .NET startup before the
worker ran; no live camera observation was saved or used to claim numeric
agreement. The independent Rust viewport tests cover all six directions in
both projections while a live comparison remains pending.

## Timing interpretation

The comparison report's `rhino_to_viboceros_ratio` is a ratio of raw harness
elapsed times, **not a kernel speedup**. Its `timing_note` is included in both
the Python report and the CLI's JSON output to keep that qualification with
exported comparisons. Correctness checks do not depend on timing ratios.

The generic measurement helpers run one warm-up before timing repeated calls.
Process startup and transport JSON I/O are outside those loops, but the work
inside a call varies by probe and engine. Rhino's `mesh_unweld_edge`, for
example, still extracts result geometry into Python containers and disposes
the edited duplicate on every timed iteration. Excluding JSON encoding does
**not** necessarily exclude result extraction.

For angle-based `mesh_unweld`, both current implementations time creation of
the edited result and cleanup of the previous result (including the warm-up
result on the first iteration). Rhino duplicates its source for each in-place
edit; native Unweld returns an owned mesh. Both extract only the final result
outside the loop, then release that result. Source mesh construction is also
outside both loops. Worker lifecycle tests check event order and cleanup when
editing, timer reads, or result extraction fail. Historical angle-Unweld
recordings predate this timing-boundary change and included per-iteration
result extraction and disposal; their geometry values remain valid.
A fresh private-session run on Rhino 8.32.26160.13001 with the revised worker
matched all six `mesh_unweld.json` recorded geometry values exactly (100 timed
iterations per case). This verifies unchanged output, not equivalent kernel
performance.

Rhino measurements also include its Python/RhinoCommon bridge and, on this
development host, FEX/Wine overhead. Existing recorded responses retain their
original timings; geometry replay tests do not compare those elapsed times.
Before making a kernel-performance claim, audit the specific operation's
timing boundaries, match copying/extraction/cleanup work, use release-mode
native builds and sufficient iterations, and report the host/emulation setup.
