# Curve, mesh and B-rep editing comparisons

[Oracle overview](../oracle.md) · [Evidence index](README.md)

## Curve matching

The [Match fixture](../../tools/rhino_oracle/fixtures/curve_match_geometry.json)
contains 33 live Rhino `CreateMatchCurve` cases for single-span line, polynomial,
and rational curves across end orientation, continuity, and preserved opposite
end options. Replay against its [saved observation](../../tools/rhino_oracle/observations/curve_match_geometry.json)
with `python3 -m tools.rhino_oracle replay tools/rhino_oracle/fixtures/curve_match_geometry.json --observations tools/rhino_oracle/observations/curve_match_geometry.json`.
All 33 outputs agree within the default 1e-10 absolute tolerance. The
[average-tangency fixture](../../tools/rhino_oracle/fixtures/curve_match_average_tangency.json)
adds eight live cases, including rational handles and reversed selected ends;
all eight match their [Rhino observations](../../tools/rhino_oracle/observations/curve_match_average_tangency.json)
within the same tolerance. The
[average-curvature fixture](../../tools/rhino_oracle/fixtures/curve_match_average_curvature.json)
adds six matching [Rhino observations](../../tools/rhino_oracle/observations/curve_match_average_curvature.json)
for opposite-end preservation of None or Position. The
[average-position fixture](../../tools/rhino_oracle/fixtures/curve_match_average_position.json)
adds ten matching [Rhino observations](../../tools/rhino_oracle/observations/curve_match_average_position.json)
for trimmed and reversed ends, including rational controls. One
[boundary diagnostic](../../tools/rhino_oracle/fixtures/curve_match_average_position_boundary.json)
retains a difference: Rhino's `CreateMatchCurve` sometimes trims a straight
cubic to a `2^-22` parameter tail when the midpoint projects onto the far
endpoint. Both public `ClosestPoint` and the native closest-point search return
that endpoint exactly. Additional private-Xvfb probes found the tail at lengths
1 through 90 in several cases, but not at lengths 12 and 100. The
[average multi-span curvature fixture](../../tools/rhino_oracle/fixtures/curve_match_average_multispan_curvature.json)
adds 13 matching [Rhino observations](../../tools/rhino_oracle/observations/curve_match_average_multispan_curvature.json)
for multi-span sources and references, rational weights, uneven knots, reversed
ends, and preserved far tangents. Its
[boundary fixture](../../tools/rhino_oracle/fixtures/curve_match_average_multispan_curvature_boundary.json)
and its two [Rhino observations](../../tools/rhino_oracle/observations/curve_match_average_multispan_curvature_boundary.json)
now match after uniform Greville preparation of short source curves. The
[multi-span tangency fixture](../../tools/rhino_oracle/fixtures/curve_match_multispan_tangency.json)
adds ten matching [Rhino observations](../../tools/rhino_oracle/observations/curve_match_multispan_tangency.json)
for cubic and quadratic two-span sources, opposite-end preservation, and
reversed picks. The
[multi-span position fixture](../../tools/rhino_oracle/fixtures/curve_match_multispan_position.json)
adds seven matching [Rhino observations](../../tools/rhino_oracle/observations/curve_match_multispan_position.json).
A more precise NURBS closest-point stopping rule resolves the interior trim
parameter at the default model tolerance. The
[multi-span curvature fixture](../../tools/rhino_oracle/fixtures/curve_match_multispan_curvature.json)
adds three matching [Rhino observations](../../tools/rhino_oracle/observations/curve_match_multispan_curvature.json)
for preserved opposite-end options None, Position, and Tangency. Its
[extended fixture](../../tools/rhino_oracle/fixtures/curve_match_multispan_curvature_extended.json)
adds 15 matching [observations](../../tools/rhino_oracle/observations/curve_match_multispan_curvature_extended.json)
for uneven knots, rational weights, and reversed selected ends. The
[uneven rational boundary case](../../tools/rhino_oracle/fixtures/curve_match_multispan_curvature_boundary.json)
now matches its [Rhino observation](../../tools/rhino_oracle/observations/curve_match_multispan_curvature_boundary.json).
The [12-case transition fixture](../../tools/rhino_oracle/fixtures/curve_match_multispan_curvature_b1_rule.json)
and [live observation](../../tools/rhino_oracle/observations/curve_match_multispan_curvature_b1_rule.json)
vary the first rational weight, first interior knot, and original second-control
projection. In these samples, Rhino chooses the curvature-derived tangential
control when it differs from the original projection by at most 10% of their
mean magnitude;
all 12 native replays match within `1e-10`. These probes ran on a separate Xvfb
display. The [five-control probe](../../tools/rhino_oracle/fixtures/curve_match_rhino_only.json)
now matches after interpolation at affine Greville parameters into a uniform,
nonrational six-control curve. The
[six-case Rhino probe](../../tools/rhino_oracle/fixtures/curve_match_five_control_far_g2_probe.json)
and [observations](../../tools/rhino_oracle/observations/curve_match_five_control_far_g2_probe.json)
cover both selected ends, uneven knots, a changed middle control, and rational
sources; all six native replays agree within `1e-10`. The
[two-case average boundary](../../tools/rhino_oracle/fixtures/curve_match_average_multispan_curvature_boundary.json)
also now matches within `1e-10`. The
[four-case quadratic arc fixture](../../tools/rhino_oracle/fixtures/curve_match_five_control_quadratic_end.json)
and [observations](../../tools/rhino_oracle/observations/curve_match_five_control_quadratic_end.json)
cover both selected ends with one-sided and average matching; all four native
replays agree within `1e-10`. The
[six-case quadratic NURBS fixture](../../tools/rhino_oracle/fixtures/curve_match_five_control_quadratic_general.json)
and [observations](../../tools/rhino_oracle/observations/curve_match_five_control_quadratic_general.json)
cover both selected ends, rational weights, and uneven knots; all six match
within `1e-10`. The
[four-control quadratic fixture](../../tools/rhino_oracle/fixtures/curve_match_four_control_quadratic_g2.json)
and [observations](../../tools/rhino_oracle/observations/curve_match_four_control_quadratic_g2.json)
exercise two added controls, both selected ends, and rational weights. These live
captures used a separate Xvfb display.
The [Rebuild comparison](../../tools/rhino_oracle/fixtures/curve_match_five_control_rebuild_probe.json)
and [observations](../../tools/rhino_oracle/observations/curve_match_five_control_rebuild_probe.json)
show Rhino's public `NurbsCurve.Rebuild(6, 3, ...)` does not produce the same
far controls as `CreateMatchCurve`. The
[translated rational sweep](../../tools/rhino_oracle/fixtures/curve_match_five_control_rational_translation_sweep.json)
and [observations](../../tools/rhino_oracle/observations/curve_match_five_control_rational_translation_sweep.json)
cover 13 X translations from `-15` to `+15`; all match within `1e-10` after
private-Xvfb capture. The source's homogeneous XYZ control-distance ratio determines
the curvature control offset; Rhino uses the absolute squared difference when
the transverse curvature offset exceeds that radius.
The [five-case supported fixture](../../tools/rhino_oracle/fixtures/curve_match_multispan_preserve_far_curvature.json)
and [Rhino observation](../../tools/rhino_oracle/observations/curve_match_multispan_preserve_far_curvature.json)
cover six- and seven-control cubic sources, both selected ends, a rational
source, and an average match between two six-control curves. Their far three
controls and knot vectors remain unchanged; native replay agrees to `1e-10`.
These captures also ran on a separate Xvfb display.
Replay the supported case with:

```sh
python3 -m tools.rhino_oracle replay tools/rhino_oracle/fixtures/curve_match_multispan_preserve_far_curvature.json --observations tools/rhino_oracle/observations/curve_match_multispan_preserve_far_curvature.json
```

Replay the transition fixture with:

```sh
python3 -m tools.rhino_oracle replay tools/rhino_oracle/fixtures/curve_match_multispan_curvature_b1_rule.json --observations tools/rhino_oracle/observations/curve_match_multispan_curvature_b1_rule.json
```

## Viewport arrangement

The `viewport_arrangement_probe` operation records model viewport bounds,
titles, cameras, projection, floating state, and active view after a bounded
sequence of `NewViewport`, `CloseViewport`, `3View`, `4View`, and viewport split
commands. Run its [fixture](../../tools/rhino_oracle/fixtures/viewport_arrangement.json)
with `tools/rhino_oracle/run_headless.sh rhino tools/rhino_oracle/fixtures/viewport_arrangement.json --timeout 300`.
It uses public [RhinoView properties](https://mcneel.github.io/rhinocommon-api-docs/api/RhinoCommon/html/Properties_T_Rhino_Display_RhinoView.htm).
The [default-view](../viewport-arrangement-rhino-reference.json) and
[Front-active](../viewport-arrangement-front-rhino-reference.json) Rhino 8.32
responses on 2026-09-25 confirm a centered, overlapping Top view and activation
of the previously active view when it closes. A
[repeated-NewViewport run](../viewport-arrangement-repeated-rhino-reference.json)
confirms that successive views stack at the same centered rectangle and close
back through the prior active views. A
[Shaded-source run](../viewport-arrangement-shaded-rhino-reference.json) confirms
that the new Top view starts in Wireframe even when the source is Shaded.
The Wine session required a normal
launcher `stop` before these probes could start.
The [4View projection run](../viewport-arrangement-projection-rhino-reference.json)
checks both first- and third-angle layouts, their camera directions and active
Perspective view, and a Front-active reset. The worker sends Rhino's command-line
option as `Projection=<angle>` followed by Enter.
The [repeated 4View run](../viewport-arrangement-fourview-repeat-rhino-reference.json)
checks a split four-view arrangement, a changed Top camera, a maximized Top
view, and a shifted Perspective camera. In these cases the first plain `4View`
call restored standard views and activated Perspective. Shaded Perspective
survived either projection arrangement, while Right-to-Left and Bottom-to-Top
replacements started in Wireframe.
The [grid-spacing run](../viewport-arrangement-fourview-grid-rhino-reference.json)
sets distinctive grid and snap spacing through each viewport's public
construction-plane API before plain and explicit `4View` calls. It confirms
that unchanged Top keeps its spacing, first-angle Left takes Right's spacing,
and returning to third-angle resets Right's spacing. Each operation first
establishes a standard four-view layout with unit grid and snap spacing.
Changing Top to Bottom or Front to Back before plain `4View` gives the restored
Top or Front and Right the changed viewport's spacing. A changed Right-to-Left
view restores Right's spacing without changing the other grids.

## Orientation and snapping

The [orientation audit](../orientation-audit.md) separates public document insertion
and replacement from actual `Flip` command behavior, retaining full definitions
and selection states. This diagnostic is not an identical-source native replay.
The separate [solid-orientation query](../solid-orientation.md) compares 49 shared
3dm inputs without document insertion: all geometry records and 47 orientations
now match, with two coincident-shell orientations unresolved. Its
[planar follow-up](../planar-solid-orientation.md) retains 68 further cases and eight
fresh-session repeats, exposing ten translation-sensitive Rhino classifications
despite identical geometric definitions after exact inverse translation.
The [Join scale audit](../join-orientation.md) adds full-definition command records
at unit and volume-overflow scales, separating fixed native execution errors
from Rhino's large-coordinate orientation and normalization discrepancies.
The [shared-source document admission probe](../document-brep-admission.md) separates
insertion and replacement using identical caller-owned inputs, with full
definitions, identity, attributes, groups, selection, and immutability checks.
It now resolves 40 normalization differences (56/64 full matches); a separate
16-case actual file-import capture has 14 full matches. Coincident-shell
orientation remains unresolved in both audits, with all numeric fields equal.

The [conic-center audit](../conic-center-audit.md) compares 32 full-coordinate public
API records and 62 calibrated point-prompt records. It keeps API recognition,
interactive snap policy, and their remaining discrepancies separate.

The [unconstrained point-snap probe](../point-snaps.md) records full 3D targets,
snap kinds and source ownership through public `GetPoint` APIs. Its 101 retained
line/mesh cases resolve the previous Mesh Near coordinate discrepancies while
preserving three corner-edge selection differences. Source/camera-only rational
references and native replay remain separate from the observed picked points.
The [mesh-competition follow-up and point replay API](../mesh-snap-order.md) expose
broader wire-selection differences with 84 retained captures, including repeated
vertex-order checks and public picking diagnostics. `projected_object_snap`
accepts calibrated native inputs; `python3 -m tools.rhino_oracle.point_snap_replay`
compares target/kind/source and returns exit status 1 for real parity differences.

## Joining, borders and caps

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
See [joining evidence and limits](../commands/join.md) and the
[event transcripts and measurement correction](../join-cycles.md).
Another [544 encoding and seam records](../join-encodings.md) check rational weights,
higher-degree controls, polycurve leaves, and their local/parent parameter maps.
An additional [48 endpoint-search records](../join-endpoint-search.md) check nearby
connections in the presence of distant unrelated sources, with absolute-only
comparison and exact preservation of the distant source records.
The [one-pass seeded-join audit](../seeded-join.md) replays all 1,197 Join records and
refreshes the 140 workflow cases against an owned private-Xvfb Rhino session.

Border duplication has 86 matching live command records and offline replay
checks, including edge-table permutations. See [border validation](../borders.md)
for the resolved polysurface seam discrepancies and identical-source 3DM protocol.

The [Cap command audit](../commands/cap.md#oracle-evidence) adds 140 matching
shared-input B-rep records, including four resolved kinked-boundary segmentation
gaps. Two additional raw discrepancies cover negative rational weights and a
large knot origin; the latter also retains analytic area/volume witnesses.
Its schema compares exact spatial edge definitions and oriented incidence,
not newly generated cap UV parameterizations. The probe verifies that Rhino
document insertion preserves the shared source before invoking the command.
The [compound follow-up](../cap-compound-orientation.md) adds 100 actual command
records: 96 match after correcting 28 total-volume orientation errors, while
four coincident-shell topology discrepancies remain explicit.

The [B-rep edge assembly audit](../brep-edge-joining.md) compares explicitly paired
native boundaries with public automatic RhinoCommon `JoinBreps` on identical
shared 3DM inputs. It records full surfaces and UV trim definitions in addition
to spatial topology, tolerances, and mass properties. Raw orientation and
gap-rebuilding policy differences are retained separately. This geometry-only
probe does not establish interactive Join behavior or native automatic matching.

The separate [surface Join command audit](../commands/join.md#surfaces-and-polysurfaces)
adds 92 actual Join/JoinCopy records with shared per-source 3DM artifacts.
Its original 26-case discrepancy archive is retained, but four partial-overlap
and sixteen gap cases now fully match; four duplicate-wall cases differ only in zero-volume
face senses. The [follow-up boundary audit](../join-boundary-matching.md) adds 160
records, now 148 fully matching and 12 explicit zero-volume orientation differences.
The [gap-rebuilding audit](../join-gap-rebuilding.md) adds 108 cases and resolves
20 earlier differences. The [selection-distance audit](../join-selection-distance.md)
adds 150 cases and resolves two more earlier differences. The
[corner-only follow-up](../join-corner-clustering.md) adds 84 cases, separating
endpoint movement from edge mating. Across all five audits,
471 of 594 cases fully match; the remaining 123 explicitly cover zero-volume
orientation, cutoff endpoints, order-dependent clusters, and area integration.
The command probe includes spatial and UV geometry, component
tolerances, oriented topology, object identity/selection, attributes, and groups.
Copies with a repeated source retain every disconnected piece. Neither failed
comparisons nor domain differences are normalized out of the saved observations.
Subsequent curved, projective and [short-overlap audits](../join-short-overlaps.md)
bring the cumulative surface-command record to 804 cases: 633 full matches,
32 native errors and 139 other differences. The short-overlap work also corrects
native source construction to use the fixture tolerance, independently of a
command's document-tolerance override, so both engines operate on the same
short-feature inputs.
The [partial-boundary certificate audit](../join-trim-certificates.md) resolves
eighteen of those differences and adds eighty pre-split curved records, retaining
all newly exposed seam-cleanup, outer-knot and area differences. That audit's total
is 651 full matches, 32 native errors and 201 other differences in 884 cases.
The [tensor-isocurve audit](../join-isocurve-certificates.md) adds 33 comparable
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

Shared `box` sources likewise retain the UV definitions of the native-written
3DM inputs used in the saved archives. Two walls use their original cyclic UV
corner order. The source adapter constructs these faces before edge splitting,
reordering, reversal, or command execution. The kernel box constructor follows
Rhino's natural wall axes, as checked by the six `CPlane Object` face captures;
changing that constructor must not silently change a saved replay's inputs.

The [redundant-edge cleanup audit](../join-edge-cleanup.md) adds 32 angular-tolerance
records using the same sixteen shared source artifacts at four document angles.
The cumulative total is 679 full matches, 32 native errors and 238 other
differences in 949 cases. Twelve previous cases become matches and none regress;
small-angle representation-dependent Rhino topology remains an explicit difference.

## Alignment and B-rep edge editing

The `align` object-layout probe compares actual bounding-box, curve and line/plane alignment commands,
including retained IDs, source samples/domains, groups, layer assignment and
pre/postselection cleanup. Its shared `object_layout` module supplies fixture
ownership and recording for Align and Distribute, not the alignment algorithm.
[Alignment evidence](../commands/align.md#oracle-evidence) includes bounding-box,
line/plane and [best-fit plane](../plane-fit.md) commands. It retains raw curved-bound,
mesh-storage and nonunique-normal discrepancies alongside successful comparisons.
All Rhino command probes run on an owned private Xvfb display.
For `ToCurve`, `curve` is an unselected curve's index in `sources` and `selected`
must explicitly omit it. Target IDs are bound only after owned source creation;
arbitrary command text is never accepted as a target. The native adapter uses
the corresponding document UUID. The 39-case curve fixture includes all seven
native target families and retains a separate incomplete-prompt diagnostic.

The `split_edge_command` operation uses the shared owned edge-edit fixture and
history recorder, with a separate strict point-input grammar. Its
[21-case record](../split-edge-provenance.json) exercises actual mouse-selected
SplitEdge commands, including full-batch duplicate failure and unchanged
endpoint-only replacements with real Undo/Redo. Native replay compares all
ordered geometry/attribute/history fields without component renumbering; see
[SplitEdge](../commands/split-edge.md) for interaction and search limitations.
An additional [19 distance-constraint records](../split-edge-distances.md) use
bounded interleaved typed distance/point inputs and real mouse clicks. They retain
curved Rhino inversion residuals and compare those cases at an explicitly separate
bound, without weakening the original observations or omitting numeric fields.
The later [17 snap observations](../split-edge-snaps.md) retain offset feature
clicks and unsnapped controls. Fifteen model-space snap locations replay complete
geometry/history; two uncalibrated screen-only controls are explicitly unsupported.
Nonuniform curve and edge witnesses distinguish arc-length Mid from parameter Mid.
The [composite/surface snap audit](../composite-feature-snaps.md) adds eleven matching
End/Mid captures, one retained uncaptured-center discrepancy and five Center
hover diagnostics. Those historical uncalibrated discrepancies remain explicit;
they are not relabeled as passing model-target or pixel replays.
The subsequent [22 calibrated Center-hover observations](../center-hover-snaps.md)
record the camera at the actual point prompt. Fourteen independently computed
native captures drive complete geometry/history replay; eight empty-center misses
compare admission only. Strict optional `record_viewport`, `persistent_snaps` and
separate `pick.aim` fields keep camera input and declared model targets distinct.
Two rejected point-target hypotheses remain unchanged in the requests.
The [four two-pick one-shot records](../object-snap-controls.md) add eight calibrated
captures and complete history comparisons, including restoration of persistent
modes after the first pick. Native menu/prompt lifecycle tests are separate from
these camera-calibrated core capture replays.
The [37 Mid-hover records](../mid-hover-snaps.md) add 26 calibrated captures with
complete geometry/history replay and 11 mixed-mode admission-only misses. One
unsnapped control fails to split and has no Undo/Redo; that limitation is retained.
These cases cover segment/boundary hover, opposite-seam conic targets and
curve-distance ranking across competing objects and polyline segments.
The [44 polygon Center records](../polygon-center-snaps.md) add 34 complete calibrated
capture/history matches and ten admission-only misses. Cases distinguish corner
averages from area/bounds centers, retain collinear/repeated curve corners,
exclude internal subdivisions of straight surface edges, and include nonplanar
curve captures alongside warped-surface misses.
The [44 circular NURBS records](../circular-center-snaps.md) add 34 complete capture
replays and four admission-only misses. The
[elliptical follow-up](../elliptic-center-snaps.md) resolves four more captures;
two negative-weight recognition differences remain. Both outside-arc controls
fail to split; the other 42 records include actual Undo/Redo.

The shared `serde_json` dependency explicitly enables round-trip float parsing.
Standalone oracle/document builds must not depend on app or test dependencies
to enable numerical fidelity through Cargo feature unification. A package-only
regression checks saved binary32 coordinates and 10,000 binary64 round trips.

## Surface and mesh topology

The `surface_wires` probe compares natural surface-to-B-rep topology and wire
geometry against Rhino `CreateFromSurface`/`GetWireframe`. The
[paired surface-wire record](../surface-wire-frames.md) contains 12 operations;
ten pass and two translated density-3 cases remain discrepancies. Its bounded
correspondence diagnostic requires unique nearest six-sample matches to form
a bijection, then compares unchanged coordinates and topology fields.

The `trimmed_surface_isocurves` probe accepts a `TrimmedBrepFixture` and 1–64
native `[u,v]` parameter pairs. It returns samples in face, query, U/V, segment,
point order. Both engines normalize each extracted curve's domain *after*
extraction, so differing output parameter origins do not hide geometric errors.
The [paired isocurve fixtures and recorded comparison](../brep-isocurves.md)
cover local and translated paraboloid disks/annuli: local cases pass, while
large-offset cases remain explicit Rhino discrepancies.

The `mesh_face_normals` probe returns one normal per stored triangle or quad.
Its [fixture](../../tools/rhino_oracle/fixtures/mesh_face_normals.json) and
[Rhino 8.32 record](../../tools/rhino_oracle/observations/mesh_face_normals.json)
cover mixed face types, warped quads, cyclic/reversed winding, and a translated,
uniformly scaled copy. The native regression checks independent analytic unit
directions to `4 * f64::EPSILON`, then checks exact equality with Rhino after
conversion to its `Vector3f` face-normal storage representation. Native geometry
and probe results retain double precision; the default `1e-10` comparison
threshold is tighter than this Rhino collection's storage precision.
Extreme-scale native tests are separate from these ordinary-scale Rhino records.

The [edge-split fixture](../../tools/rhino_oracle/fixtures/mesh_split_edge.json)
has a [27-case Rhino 8.32 record](../../tools/rhino_oracle/observations/mesh_split_edge.json).
A native replay checks exact acceptance, face indices, vertex coordinates, and
ordering for all cases, including endpoint/outside parameters, surviving fans,
orientation conflicts, and mixed triangle/quad non-manifold incidences.
For the three-face mixed incidence, full welding produces eight vertices;
partial or no endpoint welding produces 24, with eight replacement triangles
in all three cases. These dyadic-coordinate fixtures need no comparison epsilon.

The [edge-collapse fixture](../../tools/rhino_oracle/fixtures/mesh_collapse_edge.json)
has a [15-case Rhino 8.32 record](../../tools/rhino_oracle/observations/mesh_collapse_edge.json).
The shared mesh-edit replay checks exact acceptance, coordinates, face indices,
and ordering. Cases include empty results, triangle/quad reduction, seams,
non-manifold edges, unused vertices, and disconnected coincident endpoint fans.
The added coincident-peer case confirms that peers outside the selected edge
move to its midpoint without merging their separate raw vertices. These ordinary
coordinate records do not establish parity for subnormal midpoint rounding.

The [edge-weld fixture](../../tools/rhino_oracle/fixtures/mesh_weld_edge.json) has a
[nine-case Rhino record](../../tools/rhino_oracle/observations/mesh_weld_edge.json).
It exercises the actual `WeldEdge` command with selected edge subobjects,
including reversed non-manifold face order, partial seams, empty/naked selections,
unused vertices, and disjoint seams. Replay compares acceptance, removed-vertex
counts, ordered face coordinates, and vertex-to-face sharing groups exactly.
Unlike split/collapse records, this representation does not compare raw index
numbering or identify which coincident source index survives.

The [vertex-weld fixture](../../tools/rhino_oracle/fixtures/mesh_weld_vertex.json) has
a [ten-case Rhino 8.32 record](../../tools/rhino_oracle/observations/mesh_weld_vertex.json).
It exercises the actual `WeldVertices` command using selected topology vertices:
both seam endpoints, reversed selection order, empty/naked/already-welded
selections, a closed fan, vertex-only contact, non-manifold incidence, and two
incident seams. It uses the same face-coordinate and sharing-group representation
as edge welding, with exact replay and the same raw-index comparison limitation.

The [vertex-unweld fixture](../../tools/rhino_oracle/fixtures/mesh_unweld_vertex.json)
also has a [ten-case Rhino 8.32 record](../../tools/rhino_oracle/observations/mesh_unweld_vertex.json).
It covers empty/naked/already-unwelded selections, a closed fan, duplicate and
reversed selections, cube corner/all-vertex edits, a non-manifold fan, and a
fully separated triangle. Replay checks acceptance, added-vertex counts, ordered
face coordinates, and sharing groups exactly, with the same raw-index limitation.

The [angle-unweld fixture](../../tools/rhino_oracle/fixtures/mesh_unweld.json) has a
[six-case Rhino 8.32 record](../../tools/rhino_oracle/observations/mesh_unweld.json).
Replay checks exact raw vertex coordinates, face indices, ordering, and added
vertex counts for zero/positive flat thresholds, equal/above right-angle
thresholds, already-unwelded faces, and cube creases. Separate non-manifold probes
exposed a [face-order-dependent mismatch](../mesh-unweld-nonmanifold.md), now corrected
by radial face traversal and singleton-group ordering. All 69 non-manifold cases
are enabled parity regressions, including 24 four-face permutations and six
source-vertex reorderings; the earlier two ignored tests are enabled again.
The Rhino-only `mesh_radial_topology` probe records public edge ordering and
incidence separately; its twelve cases match the native radial sorter and isolated
the original discrepancy to subsequent grouping/rebuilding.

The [edge-unweld fixture](../../tools/rhino_oracle/fixtures/mesh_unweld_edge.json) has
a [19-case Rhino 8.32 record](../../tools/rhino_oracle/observations/mesh_unweld_edge.json).
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

## Short-curve selection

The [short-curve selection diagnostic](../short-curve-selection-measurement.json)
embeds four requests and responses (40 line lengths). `short_curve_selection`
creates owned line objects, runs the actual `SelShortCrv` command, and records
selected source indices, restoring the original selection and deleting only
its temporary lines. This is a Rhino-only probe; command tests replay its
measurements. The measured relative `1e-6` allowance is more specific than the
[help page's “less than” description](https://docs.mcneel.com/rhino/8/help/en-us/commands/selection_commands.htm#SelShortCrv).

The [circle follow-up](../short-curve-circle-measurement.json) records 15 analytic
circles and 15 rational NURBS circles. `curve_kind` explicitly chooses the source
representation; optional `inspect` records `GetLength`, `IsShort(limit)`, and
`IsShort(limit × 1.000001)` without changing the command under test.
All analytic and NURBS cases now replay successfully. The original mismatch was:
for nominal length `0.999999`, Rhino reports length `0.9999990022994623`, yet
both shortness queries return false and the command leaves it unselected.
The previous native length-based predicate selected it. A separate
[adaptive shortness predicate](../curve-shortness.md) now reproduces the observed
selection without changing NURBS geometry or accurate length measurement.
Rhino documents [IsShort](https://mcneel.github.io/rhinocommon-api-docs/api/RhinoCommon/html/M_Rhino_Geometry_Curve_IsShort.htm)
as a faster alternative to calculating length.

The [representation follow-up](../short-curve-representation-measurement.json)
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

## Soft SubD creases

[SubDCrease discovery](soft-crease.md) retains 32 native workflows for the next
implementation step, including cancellations and ineligible source diagnostics.
