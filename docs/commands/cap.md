# Planar hole capping

[Command reference](README.md) · [Surfaces and solids](surfaces.md)

`Cap` fills planar naked-edge loops on selected NURBS surfaces and B-reps.
Enter it before selecting objects, or select objects first. There are currently
no options. Mesh and SubD Cap are not implemented; for existing mesh hole
operations see [meshes](meshes.md).

Caps retain the spatial boundary curves, including rational weights, knots,
and domains. Their projected parameter-space trims share the original edges
and vertices. Nested loops form annular faces rather than overlapping disks.
Planar openings can be capped while other nonplanar openings remain open.

Objects retain their IDs, names, layers, colors, and group memberships.
Preselection is retained; command-first selection is cleared on success,
including no-ops. Replacements are staged into one undo step. Invalid arguments,
unsupported direct inputs, numerical validation failures, and resource-limit
errors leave every object unchanged. A no-op adds no geometry history.

The geometry API `Brep::try_cap_planar_holes` preserves shell orientation and
edge-table order. The command turns newly closed inward solids outward and
puts a single periodic face's seam edges first, matching the measured Rhino
command behavior. Already closed inputs are no-ops.

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

Containment ordering uses physical sampled area in logarithmic form, retaining
scale without overflowing products. Normalized per-loop area alone cannot order
similar nested loops. Each object permits at most 1,024 boundary cycles and one
million charged control/sample work units, including containment trials.
Every result passes the ordinary B-rep incidence and trim/edge correspondence
validation. Tolerance is never silently widened; tolerances below the precision
of translated coordinates can cause an atomic failure.

Generated cap UV frames, surface domains, loop start positions, and cap-face
insertion order are not Rhino parameterization parity claims. Multi-component
orientation repair is also incomplete: the command reverses a newly closed
B-rep as a whole when its signed volume is negative; independently reversing
mixed-orientation closed components is not implemented.

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

The largest numeric difference is `1.983e-9` in the area of an oblique ellipse
wall. Tighter Rhino integration arguments do not remove it. Independent 70-digit
quadrature of `|ellipse'(t) × extrusion_vector|` gives
`67.1308669524302689324146…`, agreeing with the native result; Rhino reports
`67.130866954412937`. Both raw values are retained, and a separate native test
checks the independent integral within `1e-12`.

[Four explicit topology gaps](../../tools/rhino_oracle/fixtures/cap_topology_gaps.json)
retain [their complete Rhino records](../../tools/rhino_oracle/observations/cap_topology_gaps.json).
For triangular and concave polygon-profile extrusions, Rhino splits kinked
boundary curves into additional edges/vertices while retaining the wall face.
Native Cap keeps those boundaries whole. Both have three faces and matching
area, volume, and document state; their topology records intentionally do not
match. Offline tests check this distinction rather than flattening away the gap.

```sh
tools/rhino_oracle/run_headless.sh compare tools/rhino_oracle/fixtures/cap.json --timeout 600 --absolute-epsilon 1e-9 --relative-epsilon 1e-10
# Expected to report the retained boundary-segmentation differences:
tools/rhino_oracle/run_headless.sh compare tools/rhino_oracle/fixtures/cap_topology_gaps.json --timeout 600 --absolute-epsilon 1e-9 --relative-epsilon 1e-10
```

McNeel's [Cap reference](https://docs.mcneel.com/rhino/8/help/en-us/commands/cap.htm)
also documents mesh and SubD options outside this implementation's scope.
