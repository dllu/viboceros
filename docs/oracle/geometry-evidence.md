# Geometry representation and intersection comparisons

[Oracle overview](../oracle.md) · [Evidence index](README.md)

## Coordinate transforms and face UV

The [SetPt transform fixture](../../tools/rhino_oracle/fixtures/plane_transforms_setpt.json)
compares seven world/CPlane axis combinations, including copy mode, with a
[Rhino 8 observation](../../tools/rhino_oracle/observations/plane_transforms_setpt.json)
captured on a separate Xvfb display. The maximum point-coordinate difference
is `2.28e-15` at `1e-9` absolute and `1e-12` relative tolerance. Replay it with:

```sh
python3 -m tools.rhino_oracle replay tools/rhino_oracle/fixtures/plane_transforms_setpt.json --observations tools/rhino_oracle/observations/plane_transforms_setpt.json --absolute-epsilon 1e-9 --relative-epsilon 1e-12
```

The [two-face UV reference](../surface-face-uv-rhino-reference.json) records four
RhinoCommon projections onto explicitly indexed B-rep underlying surfaces.
The matching [fixture](../../tools/rhino_oracle/fixtures/surface-face-uv-api.json)
ran on a separate Xvfb display; a native regression compares UV parameters,
projected points, and distances to `1e-10`. Python replay matches all four
queries, with maximum absolute error `4.44e-16`:

```sh
python3 -m tools.rhino_oracle replay tools/rhino_oracle/fixtures/surface-face-uv-api.json --observations docs/surface-face-uv-rhino-reference.json --absolute-epsilon 1e-10 --relative-epsilon 1e-10
```

This is a geometry API comparison, not an interactive `EvaluateUVPt` click capture.
The [curved-face fixture](../../tools/rhino_oracle/fixtures/surface-face-uv-curved-api.json)
adds a rational cylinder and a quadratic surface with separate non-unit UV
domains. Its six queries match the [saved Rhino result](../surface-face-uv-curved-rhino-reference.json)
within `1e-10`; the largest absolute replay difference is `3.13e-12`. Both
fixtures also exercise the document-level face-qualified UV query.
Regenerate the curved observation with:

```sh
tools/rhino_oracle/run_headless.sh rhino tools/rhino_oracle/fixtures/surface-face-uv-curved-api.json --timeout 300
```

## Trims, representations and intersections

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
The [certified split follow-up](../surface-curve-certificates.md) replays a fresh
private-Xvfb capture of both recipes after requiring continuous certificates for
the local cutting trims. Its [provenance](../certified-surface-splits-provenance.json)
records the full capture without claiming general curved Boolean parity.

The `trimmed_mass_properties.json` fixture checks [nonplanar trimmed-face area
and signed volume](../mass-properties.md). Its `trimmed_surface_mass_properties`
operation supplies exact spatial and UV loop curves, an underlying surface, and
an optional cap surface sharing those boundaries. Rhino's public B-rep topology
API builds the same input geometry as the native probe, avoiding changes to the
input from a separate trim-fitting operation. Numerical API calls are timed;
the native probe also checks `Area` and `Volume` command results and document state.
The [35 trim-domain frame cases](../mass-properties.md#trim-domain-regression-audit)
vary trim parameters without changing UV coordinates or spatial edges. Their
records include complete trim definitions, extracted outside the timed section,
so implicit input normalization cannot masquerade as large-domain coverage.

The `curve_area.json` fixture checks eight [enclosed curve-area cases](../curve-area.md)
against analytic references and Rhino's public API. The retained strict comparison
has two conic discrepancies; it is not an all-passing parity claim.

The `non_manifold_selection.json` fixture checks the actual
[`SelNonManifold` command](../non-manifold-selection.md) on mesh and B-rep objects,
with separate unselected controls and additive-selection cases.

The `polycurve.json` fixture exercises [exact piecewise curves](../polycurves.md).
It preserves and compares segment definitions and domains, then tests reversal,
trimming, splitting, length-based reparameterization, derivatives, and division.
The `curve_division_contract.json` fixture checks open/closed division endpoint
rules separately, including the `include_ends=false` case.
`polycurve_document.json` exercises actual extraction and explode commands,
duplicate comparison, and 3DM round-trip segment definitions; the native side also
checks undo. See [polycurve documentation](../polycurves.md) for compatibility boundaries.

`edge_surface.json` and `edge_surface_command.json` compare [Coons edge surfaces](../edge-surfaces.md)
through both the geometry API and document command, including full NURBS
coefficients, direct samples and singular-side topology. The zero-weight-control
case uses exact comparison-degree elevation while retaining original-surface
samples. `edge_surface_3dm_interchange.json` separately checks both readers of the
same native-written files, without changing the serialized basis.

`point_grid.json` checks [point-grid surface construction](../point-grid-surfaces.md),
selecting the [native producer](../smlib-point-grid.md) when `native-smlib` is enabled,
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

`curve_native_parameters.json` covers the shared [native parameter contract](../curve-parameters.md)
for all seven curve families. It compares domains, first/second derivatives,
tangents, equal-length division parameters, reversal, transforms, and NURBS definitions.
`curve_native_editing.json` extends those records with native trims, cyclic subcurves,
splits, seam relocation, periodic curves, and unequal rational weights. See
[domain-editing validation](../curve-domain-editing.md) for the separate geometry and
Rhino length-inversion comparison limits.

`nurbs_rational_jets.json` adds 23 degree-one and higher rational derivative,
weight-scale, and seam comparisons. `nurbs_translated_jets.json` is a separate
diagnostic exposing Rhino's large-coordinate cancellation, not a passing reference
at the ordinary epsilon. Its `curve_native` operations set `differential_only=true`
to omit length/division calls, isolating derivative evaluation from Rhino's failed
translated length inversion. See [rational numerical validation](../nurbs-numerics.md)
for analytic checks, observed errors, and remaining kernel limits.

`curve_sided_evaluation.json` compares 45 native left/right limit cases via
`sided_parameters`, including composite child knots and stationary endpoints.
See [one-sided evaluation](../curve-sided-evaluation.md) for the exact-limit contract,
Rhino probe construction, and native-only full-order knot tests.

`curve_frames.json` and `curve_frames_multispan.json` compare 22 sets of
[rotation-minimizing frames](../curve-frames.md), keeping points/tangents separate
from the relative-rotation accuracy checks. `curve_frames_diagnostics.json`
isolates seven known Rhino availability, large-coordinate, and corner-query
differences; it is not a passing reference. `curve_array.json` now records
unrounded endpoints. `curve_array_corner_diagnostics.json` is a separate,
untimed actual-command probe exposing Rhino's query-dependent corner twist.

`sweep1.json`, `sweep1_command.json`, `sweep1_multisection.json`, and
`sweep1_weights.json` check 49 [one-rail sweep cases](../sweep1.md)
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
analytic checks preserve the original ruled profiles. See [Loft](../loft.md) and
[endpoint-weight numerics](../nurbs-numerics.md#endpoint-weight-normalization).

`surface_jets.json` checks 20 [rational surface differential cases](../surface-evaluation.md),
including second partials, parameter-domain changes, continuation, and exact
quadrant limits. `surface_translated_jets.json` separately diagnoses large-coordinate
cancellation and is not a passing reference at the ordinary epsilon.

`surface_curvature.json` checks 27 [surface-curvature API cases](../curvature.md),
including one-sided knot limits and singular-pole availability. Spatial shape
operators avoid arbitrary principal-direction signs and umbilic axes.
`surface_curvature_umbilic.json` isolates Rhino's roughly `7.45e-9` repeated
eigenvalue split on a sphere and uses its own `1e-8` absolute comparison limit.
`curvature_command.json` checks 17 actual command cases, including permanent
markers, source geometry and attributes, and retained selection. These command
records verify that a measurement was reported; the API records separately
check unrounded numerical results.

`curve_surface_morph.json` separates [direct point-map and curve-fit validation](../curve-morphing.md).
Its eight direct maps match at absolute `2e-12`; fitted outputs use `1e-5` because
Rhino's observed fit error exceeds the requested `1e-9`. Native fits are separately
asserted against their direct maps at the requested tolerance. `surface_orient.json`
now compares unrounded curve samples and document state, not rounded control nets.
`surface_surface_morph.json` adds five [surface-fitting cases](../surface-morphing.md)
with 1,089 offset-grid direct-map and fitted samples per case; native fit errors
are independently checked at each case's requested tolerance.
`brep_surface_morph.json` adds four [trimmed B-rep morph cases](../brep-morphing.md),
checking shared topology, direct maps, and fitted geometry separately. Edge
samples use closest-point correspondence because Rhino changes their parameter
speeds; native fits additionally retain original-parameter accuracy checks.
`brep_mesh_boundaries.json` checks [open and closed shared-boundary meshing](../brep-meshing.md)
on five independently refined box-face subsets. It compares boundary geometry
and incidence properties, not identical mesh element counts, and separately
validates native `Mesh` command results.

`curve_3dm_interchange.json` checks Rhino's reading of actual Viboceros-written
files, including decomposed full-order knots and all visibility/locking states.
Unlike independent-construction probes, `compare` supplies shared private artifact
paths and cleans them after both readers finish. See [curve interchange](../curve-3dm-interchange.md).
`brep_3dm_interchange.json` uses the same shared-artifact workflow for eight
[nonlinearly morphed B-reps](../brep-3dm-interchange.md), comparing complete NURBS
definitions, topology, tolerances, geometry samples and post-import mesh topology.
`rational_3dm_range.json` checks four extreme-coordinate/weight cases using zero
absolute epsilon and relative `1e-12`; see [numeric range validation](../rational-3dm-range.md).

`curve_parameter_map.json` adds both native/rational parameter maps to those records.
`curve_native_cutting.json` tests cutting-object Split and Trim on all curve families,
including wrapped outputs, seam hits, and projected cuts. `curve_native_extrusion.json`
checks profile-domain preservation. See [native cutting validation](../curve-cutting.md)
for the 75 cases, numeric limits, and the separate ill-conditioned legacy Trim
tangent comparison. Curve-cut command records sort by native domain, not rounded
world-space endpoint coordinates.

`curve_extrusion_directions.json` retains 27 actual numeric `ExtrudeCrv` commands,
including reversed and offset planar profiles, mixed selections, skew lines,
BothSides, signed distances and rotated CPlanes. Replay compares complete raw
surface definitions and output order. Both portable and native builds exercise
the fixture; see [numeric extrusion qualification](../smlib-extrusion-direction.md).

`curve_join_close.json` compares 39 mixed joining and closure cases, including
full NURBS definitions, retained intervals, representation, and length. It tests
the batch `JoinCurves` API separately from actual `Join`/`CloseCrv` commands.
Join command records also compare source identity, names, and overlapping groups;
document results are sorted by their unique source names. The native command
path executes the real command registry. See [curve editing](../curve-editing.md).

The [curved face Intersect fixture](../../tools/rhino_oracle/fixtures/curved_brep_face_intersect_command.json)
compares three live cylinder/plane surface-B-rep and B-rep/B-rep commands. All
three [saved observations](../../tools/rhino_oracle/observations/curved_brep_face_intersect_command.json)
match after canonicalizing the direction of straight intersection curves;
Rhino chooses branch directions from input order. The
[circle section fixture](../../tools/rhino_oracle/fixtures/curved_brep_face_circle_intersect_command.json)
adds fourteen matching [Rhino observations](../../tools/rhino_oracle/observations/curved_brep_face_circle_intersect_command.json)
for surface order, reversed selection, plane normals, rotated axes, and section
heights. Full cylinder sections now use Rhino's signed `2π` domain and winding.
The [closed cylinder fixture](../../tools/rhino_oracle/fixtures/cylinder_brep_intersect_command.json)
and [saved observations](../../tools/rhino_oracle/observations/cylinder_brep_intersect_command.json)
also probe a full-domain curved wall inside a three-face solid. Rhino gives its
closed circular section a `4π` domain. The
[steep and oblique section fixture](../../tools/rhino_oracle/fixtures/curved_cylinder_brep_intersect_command.json)
has three more [saved Rhino results](../../tools/rhino_oracle/observations/curved_cylinder_brep_intersect_command.json).
The kernel joins exact rational wall arcs to cap segments for a steep cut and
keeps an untrimmed oblique cut as an exact rational ellipse. Rhino's Intersect
command returns fitted cubic curves for these cuts, with sampled boundary
deviation up to `3.5e-6`; the native curves' sampled deviation is below `1e-14`.
The [section geometry audit](../../tools/rhino_oracle/audit_cylinder_sections.py)
checks planarity, solid boundary, bounds, and closure independent of curve
degree and parameterization. Full NURBS definitions differ in these cases.
The B-rep intersection path also clips sections against curved face trim loops.
Height and angular isocurve trims on cylinder walls have exact kernel tests.
An exact non-isocurve split test covers a tilted ellipse trim, transverse
quarter-circle sections, and a single-point contact at the cut endpoint;
general curved trim loops still need broader oracle coverage.
The [trimmed wall Intersect fixture](../../tools/rhino_oracle/fixtures/trimmed_cylinder_face_intersect_command.json)
compares four [Rhino observations](../../tools/rhino_oracle/observations/trimmed_cylinder_face_intersect_command.json)
for lower and upper wall pieces, a disjoint plane, and a planar B-rep cutter.
All four match full curve definitions, including Rhino's `4π` domain for a
closed section of a trimmed cylindrical face.
The [angular wall fixture](../../tools/rhino_oracle/fixtures/angular_cylinder_face_intersect_command.json)
matches three more [Rhino observations](../../tools/rhino_oracle/observations/angular_cylinder_face_intersect_command.json)
for both half-cylinder faces and a B-rep cutter. Rhino doubles the parameter
interval of each exact semicircular section, so the two halves use `0..2π`
and `2π..4π`.
The [trimmed planar face fixture](../../tools/rhino_oracle/fixtures/curved_surface_trimmed_plane_intersect_command.json)
checks the opposite arrangement: a cylindrical surface against either half
of a split planar B-rep face. Both [Xvfb-captured Rhino results](../../tools/rhino_oracle/observations/curved_surface_trimmed_plane_intersect_command.json)
match the complete native NURBS definitions within `1e-10`.
The [coincident trimmed planar face fixture](../../tools/rhino_oracle/fixtures/coincident_trimmed_planar_face_intersect_command.json)
checks a planar surface against a split planar B-rep face, with the surface
either enclosing or cutting through the face. Both [Xvfb-captured Rhino results](../../tools/rhino_oracle/observations/coincident_trimmed_planar_face_intersect_command.json)
match the native closed perimeters exactly after canonicalizing linear curve
orientation and parameterization (`2/2` cases, zero coordinate difference).
The [coincident trimmed B-rep face fixture](../../tools/rhino_oracle/fixtures/coincident_trimmed_brep_faces_intersect_command.json)
uses the same two regions as B-rep/B-rep command inputs. Both [Xvfb-captured Rhino results](../../tools/rhino_oracle/observations/coincident_trimmed_brep_faces_intersect_command.json)
also match the native closed perimeters exactly after the same linear curve
canonicalization (`2/2` cases, zero coordinate difference).
The [coincident box fixture](../../tools/rhino_oracle/fixtures/coincident_box_brep_intersect_command.json)
compares identical boxes and coaxial boxes with overlapping heights. The
[edge audit](../../tools/rhino_oracle/audit_coincident_box_edges.py) verifies that
both engines return every one of the twelve overlap-box edges exactly once,
with zero coordinate error in both cases. Rhino joins the coaxial case into
five curves; Viboceros joins the same edges into four.
The [bent planar strip fixture](../../tools/rhino_oracle/fixtures/coincident_bent_strip_surface_intersection_command.json)
checks an enclosing rectangle and a partial rectangle against a quadratic
planar strip, repeats the partial overlap with surface/B-rep and B-rep/B-rep
command inputs, and intersects two bent strips in both input orders and with
shifted parameter domains. All ten [Xvfb-captured Rhino results](../../tools/rhino_oracle/observations/coincident_bent_strip_surface_intersection_command.json)
match the complete native degree-two curves, including control points, knots,
and domains, within `1.8e-15`. The [transposed strip fixture](../../tools/rhino_oracle/fixtures/coincident_transposed_bent_strip_surface_intersection_command.json)
repeats seven surface cases with the curved direction in V; its
[Xvfb observations](../../tools/rhino_oracle/observations/coincident_transposed_bent_strip_surface_intersection_command.json)
also match the native curves exactly within floating-point tolerance.
The [rational strip fixture](../../tools/rhino_oracle/fixtures/coincident_rational_bent_strip_surface_intersection_command.json)
and its [transposed companion](../../tools/rhino_oracle/fixtures/coincident_transposed_rational_bent_strip_surface_intersection_command.json)
add eight weighted planar-strip overlaps. Their
[U-direction](../../tools/rhino_oracle/observations/coincident_rational_bent_strip_surface_intersection_command.json)
and [V-direction](../../tools/rhino_oracle/observations/coincident_transposed_rational_bent_strip_surface_intersection_command.json)
Xvfb observations match all native control points, weights, knots, and domains.
The [mixed-axis strip fixture](../../tools/rhino_oracle/fixtures/coincident_mixed_axis_bent_strip_surface_intersection_command.json)
intersects U-curved strips with V-curved strips in both orders, with polynomial
and rational weights, shifted parameter domains, and rotated geometry. All
twelve [Xvfb observations](../../tools/rhino_oracle/observations/coincident_mixed_axis_bent_strip_surface_intersection_command.json)
match the full native curves.
The [nonproportional rational strip fixture](../../tools/rhino_oracle/fixtures/coincident_nonproportional_rational_strip_surface_intersection_command.json)
and its [orientation variants](../../tools/rhino_oracle/fixtures/coincident_nonproportional_rational_strip_orientation_command.json)
cover twelve overlaps whose two weight rows are not proportional, including
transposed and mixed-axis inputs. The
[U-direction observations](../../tools/rhino_oracle/observations/coincident_nonproportional_rational_strip_surface_intersection_command.json)
and [orientation observations](../../tools/rhino_oracle/observations/coincident_nonproportional_rational_strip_orientation_command.json)
match all native curves. A separate
[refined-strip fixture](../../tools/rhino_oracle/fixtures/coincident_refined_nonproportional_strip_surface_intersection_command.json)
checks degree elevation and an inserted knot in four more cases; its
[Xvfb results](../../tools/rhino_oracle/observations/coincident_refined_nonproportional_strip_surface_intersection_command.json)
match within `7.2e-15`.
The [bilinear plane fixture](../../tools/rhino_oracle/fixtures/bilinear_plane_intersect_command.json)
adds five saddle-patch `Intersect` cases: two conic branches, a clipped branch,
two plane-contained rulings, two isolated corner contacts, and rationally
weighted conics. The [saved Rhino results](../../tools/rhino_oracle/observations/bilinear_plane_intersect_command.json)
fit cubics to the conics, while the native kernel keeps exact rational
quadratics. The [locus audit](../../tools/rhino_oracle/audit_bilinear_plane_sections.py)
checks the resulting geometry independently of curve degree and parameterization.
Its maximum sampled surface deviation is `2.63e-6` for Rhino's fitted curves
and below `7e-16` for the native exact curves. The straight rulings and corner
points have the same locus; only the corner-point case matches full output
definitions at the default `1e-10` comparison tolerance.

```sh
python3 -m tools.rhino_oracle.audit_bilinear_plane_sections \
  tools/rhino_oracle/fixtures/bilinear_plane_intersect_command.json \
  tools/rhino_oracle/observations/bilinear_plane_intersect_command.json
```

```sh
python3 tools/rhino_oracle/audit_cylinder_sections.py \
  tools/rhino_oracle/fixtures/curved_cylinder_brep_intersect_command.json \
  tools/rhino_oracle/observations/curved_cylinder_brep_intersect_command.json \
  --max-error 5e-6
```
