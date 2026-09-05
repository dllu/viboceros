# Rhino compatibility oracle

[Project overview](../README.md)

The versioned Python oracle API runs identical JSON geometry and document-state
batches in a native release build of Viboceros and Rhino 8, recursively checks
results, and reports per-operation timings.

Each batch applies the request's absolute, relative, and angular tolerances to
Rhino's active document and restores its previous settings on success or failure.
This matters for command macros, which read document settings rather than an API
tolerance argument. See [Rhino's document tolerance API](https://developer.rhino3d.com/api/rhinocommon/rhino.rhinodoc/modelabsolutetolerance).
Older command comparisons made before this synchronization need revalidation.

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

`curve_3dm_interchange.json` checks Rhino's reading of actual Viboceros-written
files, including decomposed full-order knots and all visibility/locking states.
Unlike independent-construction probes, `compare` supplies shared private artifact
paths and cleans them after both readers finish. See [curve interchange](curve-3dm-interchange.md).
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
