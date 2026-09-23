# Planar hole capping

[Command reference](README.md) · [Surfaces and solids](surfaces.md)

`Cap` fills planar naked-edge loops on selected NURBS surfaces, B-reps, and meshes.
Enter it before selecting objects, or select objects first. There are currently
no options. Mesh caps are triangulated in place; Rhino's mesh `DeleteInput`,
`Crease`, and `Triangles` options and SubD capping are still unavailable. For
nonplanar mesh holes, see [meshes](meshes.md).

Caps retain the spatial boundary geometry. The command subdivides newly capped
edges at C0 knot joins with tangent breaks of at least 1°, independently of
document angular tolerance. Segments retain degree, rational weights, and native parameter
intervals; collinear joins remain inside a single edge. Every incident trim is
updated, including both uses of a seam. Nested loops form annular faces rather
than overlapping disks.
Planar openings can be capped while other nonplanar openings remain open.
For meshes, each unbranched boundary is tested against a best-fit plane using
the document's absolute tolerance. Existing mesh winding is preserved around
the generated triangles. Nonplanar and ambiguous boundaries remain open.
Coplanar mesh boundaries with overlapping bounding boxes are left open to
avoid overlapping disks; annular caps with inner boundaries are not yet built.

Objects retain their IDs, names, layers, colors, and group memberships.
Preselection is retained; command-first selection is cleared on success,
including no-ops. Replacements are staged into one undo step. Invalid arguments,
unsupported direct inputs, numerical validation failures, and resource-limit
errors leave every object unchanged. A no-op adds no geometry history.

The geometry API `Brep::try_cap_planar_holes` preserves shell orientation and
edge-table order. The command turns newly closed inward solids outward. Without
boundary subdivision, a single periodic face's seam edges are placed first.
With subdivision, each edge's first segment retains its table slot; new vertices
and segments are appended in source-edge order, highest cut first. These rules
match the measured ordinary-domain Rhino cases. Already closed inputs are no-ops.

An entirely planar B-rep is unchanged. This test applies to the whole B-rep,
not each disconnected piece: Rhino can cap two separated planar sheets into
zero-volume double sheets. This unusual case is preserved and tested; the
topological `is_solid` predicate does not establish a positive enclosed volume.

## Validation and limits

The implementation follows unambiguous oriented cycles in shared topology;
it does not join nearby endpoints or repair inconsistent edge incidence.
Planarity is conservatively checked on all boundary control points using the
document's absolute tolerance. Mixed-sign rational weights are rejected.
Projected loops are validated with constrained triangulation of sampled trim
regions. Linear spans use exact endpoints; curved spans currently use four
samples per span. This is not certified continuous intersection testing, and
general self-intersecting, touching, nearly coincident, or badly conditioned
boundaries are not a supported repair workflow.
Singular-tangent kinks inside smooth knot spans are not currently located;
degenerate tangents at candidate full-multiplicity knots trigger subdivision.

Containment ordering uses physical sampled area in logarithmic form, retaining
scale without overflowing products. Normalized per-loop area alone cannot order
similar nested loops. Each object permits at most 1,024 boundary cycles and one
million charged control/sample work units, including containment trials.
Shared-edge subdivision additionally permits 100,000 cuts and four million
charged control/sample work units. Complete-knot cuts slice retained controls
directly instead of repeatedly copying the shrinking source curve. Independently
parameterized trims use bounded geometric correspondence; unresolved or
nonmonotone matches fail atomically. Lossless local UV and knot-origin frames
avoid large-parameter quantization during subdivision and boundary validation;
stored surface domains and curve domains are not normalized.
Every result passes the ordinary B-rep incidence and trim/edge correspondence
validation. Tolerance is never silently widened; tolerances below the precision
of translated coordinates can cause an atomic failure.

Generated cap UV frames, surface domains, loop start positions, and cap-face
insertion order are not Rhino parameterization parity claims.

Newly closed results use the conservative [spatial orientation query](../solid-orientation.md).
An inward result is reversed as a whole; components are not independently flipped,
which preserves cavity and mixed-shell senses. An outward compound may have
negative or zero signed volume. If the exact query is unresolved, a single
edge-connected shell retains a numerical signed-volume fallback; this is not a
certified orientation or a self-intersection test. Unresolved compound solids
retain their face senses and report `orientation unresolved` in the command
message. See the [compound audit](../cap-compound-orientation.md).

## Oracle evidence

[109 fixtures](../../tools/rhino_oracle/fixtures/cap.json) and their
[raw Rhino 8.32.26160.13001 observations](../../tools/rhino_oracle/observations/cap.json)
cover every nonempty subset of box faces, pre/postselection, inward shells,
edge permutations, nested/thin annuli, oblique circular and elliptical
extrusions, partially nonplanar holes, and curved domes.
All 109 recorded comparisons pass with absolute epsilon `1e-9` and relative
epsilon `1e-10`. They compare raw spatial edge definitions/domains/samples,
vertices, oriented loop-to-edge incidence, face areas, signed volume, solid and
manifold flags, identity retention, attributes, group count, and selection.
Face/loop records are sorted by edge incidence, explicitly ignoring generated
UV frames, loop starts, and face insertion order.

Both engines read the same native-generated 3DM input. Export checks its native
round trip, and the Rhino probe disables automatic kinky-surface splitting and
verifies insertion did not change the recorded input. An inward *already closed*
source is excluded: Rhino document insertion itself flips it before Cap can run.
Snapshots use Cap's public EndCommand event. Every Rhino run uses an owned
private Xvfb display; probes restore selection, layers, and groups and delete
only their owned objects. These are untimed correctness probes, not benchmarks.

The largest baseline numeric difference is `1.983e-9` in the area of an oblique ellipse
wall. Tighter Rhino integration arguments do not remove it. Independent 70-digit
quadrature of `|ellipse'(t) × extrusion_vector|` gives
`67.1308669524302689324146…`, agreeing with the native result; Rhino reports
`67.130866954412937`. Both raw values are retained, and a separate native test
checks the independent integral within `1e-12`.

[Four formerly discrepant polygon cases](../../tools/rhino_oracle/fixtures/cap_kink_boundaries.json)
now match their [complete Rhino topology records](../../tools/rhino_oracle/observations/cap_kink_boundaries.json).
[12 angle/weight/degree cases](../../tools/rhino_oracle/fixtures/cap_edge_splits.json)
and [15 angular-tolerance/order/curvature cases](../../tools/rhino_oracle/fixtures/cap_edge_splits_angular.json)
also match, for **140 passing command records** overall. These include degree-two
and degree-three segments, rational/nonuniform knot data, inward shells, and
angles immediately below/at/above 1°. Raw observations are retained under the
same basenames in `tools/rhino_oracle/observations`. The two new matrices keep
their actual differing document angular tolerances. Their largest numeric
difference is `6.342e-9` in a curved cubic face area, within the same relative
comparison epsilon.

Two new discrepancies remain explicit, with complete input/output observations:

- [All-negative rational weights](../../tools/rhino_oracle/fixtures/cap_negative_weights.json):
  Rhino reports success but leaves the wall uncapped; native Cap produces a
  valid three-face solid. No weight-sign normalization hides this distinction.
- [A `1e9` knot origin](../../tools/rhino_oracle/fixtures/cap_parameter_origin.json):
  Rhino splits only the second of two triangle-profile kinks (five edges/four
  vertices), while native Cap retains both (seven edges/six vertices). The
  wall's analytic area, `4√29 + √746 + 3√26 = 64.1507183368116966…`, agrees with
  native within `1e-12`; Rhino reports `64.150718708800468`. Native volume is
  within `1e-12` of the analytic `30` after the
  [trim-domain integration correction](../mass-properties.md#trim-domain-regression-audit),
  while Rhino differs by about `1.33e-7`.
  These are topology and integration discrepancies, not a claim that the
  retained curve loci differ by the per-edge sample-record differences.

The [100-case compound follow-up](../cap-compound-orientation.md) adds 96 matching
records and four retained coincident-shell topology differences. It resolves
28 orientation errors in the former total-volume rule without changing the
140 passing ordinary-source comparisons above.

Native tests additionally cover seam splitting, independent rational trim
parameter speeds, full-order knots, nonclamped curves, and rejected ambiguous
correspondence. Full-order interior knot multiplicity cannot enter the shared
3DM matrix: OpenNURBS rejects that single-curve representation at export.

```sh
tools/rhino_oracle/run_headless.sh compare tools/rhino_oracle/fixtures/cap.json --timeout 600 --absolute-epsilon 1e-9 --relative-epsilon 1e-10
tools/rhino_oracle/run_headless.sh compare tools/rhino_oracle/fixtures/cap_kink_boundaries.json --timeout 600 --absolute-epsilon 1e-9 --relative-epsilon 1e-10
tools/rhino_oracle/run_headless.sh compare tools/rhino_oracle/fixtures/cap_edge_splits.json --timeout 600 --absolute-epsilon 1e-9 --relative-epsilon 1e-10
tools/rhino_oracle/run_headless.sh compare tools/rhino_oracle/fixtures/cap_edge_splits_angular.json --timeout 600 --absolute-epsilon 1e-9 --relative-epsilon 1e-10
# cap_negative_weights.json and cap_parameter_origin.json deliberately report differences.
```

McNeel's [Cap reference](https://docs.mcneel.com/rhino/8/help/en-us/commands/cap.htm)
documents the remaining mesh and SubD options.
The [mesh Cap probe](../../tools/rhino_oracle/fixtures/mesh_cap_command.json)
is prepared for command-level comparison. The first run could not execute:
the local Rhino process exited during .NET startup with an access violation.
