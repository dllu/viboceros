# Rhino compatibility oracle

[Project overview](../README.md)

The versioned Python oracle API runs identical JSON geometry and document-state
batches in a native release build of Viboceros and Rhino 8, recursively checks
results, and reports per-operation timings.

Standard geometry/command batches apply the request's absolute, relative, and angular tolerances to
Rhino's active document and restores its previous settings on success or failure.
This matters for command macros, which read document settings rather than an API
tolerance argument. See [Rhino's document tolerance API](https://developer.rhino3d.com/api/rhinocommon/rhino.rhinodoc/modelabsolutetolerance).
Older command comparisons made before this synchronization need revalidation.

`group_picking.json` is a dedicated, untimed three-line fixture: an idle-event
worker returns control to Rhino's normal UI loop, then the host clicks projected
line locations in the newly owned window and acknowledges each click atomically.
No Enter or selection command substitutes for a mouse pick. Its 52 exact checks
cover ordered group selection, hidden/locked peers and layers, and subsequent
Move commands. Run it as a separate batch with `run_headless.sh` (one iteration);
it cannot be mixed with synchronous geometry operations. See [groups](groups.md).

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

`plane_transforms.json` checks 140 actual transform commands using four affinely
independent point witnesses per operation, including copied/original identity
and selection. Four parallel-reference Shear cases remain explicit diagnostics
in `plane_transform_diagnostics.json`; see [plane transforms](plane-transforms.md).

`interface_commands.json` checks 208 untimed display/snapping transitions across
eight initial states against Rhino's actual commands. The native probe shares
the GUI's parser and state reducer. The worker restores application settings
and all viewport modes; see [interface controls](commands/interface.md).

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
Timings cover repeated API calls after one warm-up and exclude process startup,
fixture construction, and JSON I/O; Rhino timings include its public
Python/RhinoCommon bridge.
