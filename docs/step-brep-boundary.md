# Editable STEP B-rep conversion boundary

[File-format support](file-formats.md) · [Native B-rep geometry](../crates/viboceros-geometry/src/brep.rs)

The `ImportStep` command defaults to display meshes. `ImportStep Native=Yes`
imports supported planar and NURBS shells as editable document B-reps. A low-level
`read_step_planar_shells` API now produces validated native planar B-reps from
source shell definitions; the command uses the assembly-aware unit-converting
reader described below. The general native instance reader also accepts
NURBS/B-spline surfaces and matching curved edges and UV trims. See
[ImportStep](commands/import-step.md).
The mesh route must not be relabeled as an exact B-rep import: tessellation
discards the source surface definitions, shared curve identities, and UV trims.

## Implemented planar source-shell reader

```rust
let shells = viboceros_io::read_step_planar_shells(
    std::fs::File::open("part.step")?,
    viboceros_geometry::Tolerance::DEFAULT,
)?;
// Each entry has source_shell_id and an editable native brep.
```

Results are source shell definitions in entity-ID order, in file coordinates.
This API does not expand assembly instances, apply oriented-shell wrappers, or
convert length units; its tolerance is in source units. It is a conversion
building block, not a replacement for the existing assembly-aware importer.

Assembly discovery now lives in a geometry-independent `step/instance_plan`
module. It resolves supported shape IDs, composed placement matrices, instance
names, unplaced-shape fallback, and representation diagnostics before the mesh
consumer loads geometry. Direct plan tests check all eight source cube corners
for three instances, with and without a noncommuting parent rotation/translation;
removing source shell records leaves the plan unchanged. The source-shell reader
remains independent of placements; the native instance reader below consumes
this plan.

### Assembly-aware planar instances

`read_step_planar_instances(reader, tolerance)` returns a `StepPlanarImport`
containing placed native shell entries and the shared import diagnostics.
Coordinates and tolerance are in file units. Its
`read_step_planar_instances_in_units(reader, &target_units, tolerance)` counterpart
resolves uniform file units and scales the fully placed geometry, including
assembly translations. Tolerance is in target units; source geometry and
placement validation use the corresponding source tolerance. Both paths retain
UV trims, shell sense, occurrence grouping, and diagnostics. Neither inserts
document objects themselves; `ImportStep Native=Yes` inserts their combined
occurrences on the current layer as one undoable command.

Each entry retains `source_shape_id`, `source_shell_id` (the actual shell
reference, including oriented wrappers), the occurrence name, and a
`placement_index` shared by all shells of that shape occurrence. Solid outer
and void shells retain source order and orientation; surface models expand to
their referenced shells. These entries are shells, not independently classified
solids; assembly occurrence grouping does not establish cavity containment.

Source shells are converted once per referenced shell ID, transformed and
validated separately for each placement, then released from the conversion
cache after their last use. Unsupported shell geometry, topology loss, or invalid
placement fails the request without returning partial geometry. Unplaced shapes
and assembly/representation diagnostics follow the mesh reader's policy.
Non-affine matrices are rejected instead of dropping their projective terms.
Placed occurrences are sorted by source shape ID, name, and matrix coefficients
before assigning placement indices, avoiding hash-dependent assembly traversal
order. Unplaced shapes follow in source-ID order. Equal keys remain separate
occurrences. Repeated-parse tests cover both distinct and duplicate names.

Generated regressions check repeated and nested cube placements against every
expected corner, area 286 and signed volume 315, and verify both senses of an
oriented open-shell wrapper without changing UV loops. A hollow cube checks
separate outer/void entries with signed volumes 1000 and -8. This is not yet
evidence of live Rhino assembly parity.
Unit-aware instance tests cover millimetres, centimetres, metres, kilometres,
and microns, checking all placed vertices, quadratic area and cubic volume
scaling, identical UV loops and metadata, and unitless/invalid-unit behavior.

`read_step_planar_shells_in_units(reader, &target_units, tolerance)` additionally
resolves uniform file length units and returns native geometry in the requested
target units. Its tolerance is expressed in target units; source validation uses
the corresponding source tolerance. The geometry transform scales vertices,
shared edge curves, surface control points, and model-space tolerances, while
preserving UV trims and face sense. Assembly expansion and oriented-shell wrappers
remain outside this API.

Both mesh and native unit-aware readers share unit resolution and tolerance
conversion. Missing, mixed, cyclic, or invalid file units and invalid targets
fail before conversion. A unitless target preserves source coordinates, matching
the common `LengthUnitSystem` policy; unset is an error. Tests check millimetres,
centimetres, metres, kilometres, and microns, including quadratic area scaling,
cubic volume scaling, identical UV loops, and unitless/error behavior.

The supported subset has planar surfaces, one outer loop and any number of
strictly disjoint, unnested inner loops per face, straight
3D edges (including two-control-point degree-one B-splines and linear leaders
of plane/plane intersections), and straight UV trims represented as lines or
two-control-point degree-one B-splines. B-spline trim knot vectors and parameter
intervals are preserved, not normalized to a line's `[0,1]` interval. Plane control rectangles
retain source UV coordinates. Shared edges, trim reversal, and face sense are
kept distinct. Every result passes native `Brep::try_new` validation.

Unsupported curves/surfaces, invalid polygon regions, missing trims, non-manifold edges,
and reported source-shell topology losses fail the entire request. No mesh
substitute is returned. This strict planar API remains available separately from
the general native instance reader.

A generated STEP triangle with explicit `SURFACE_CURVE`/`PCURVE` records and
degree-one B-spline parameter curves verifies loss-free loading, retained
`[-3,7]` trim intervals, and native area 50. Direct adapter tests compare nine
evaluation stations on three intervals in both directions; curved and multispan
B-spline trims remain explicitly unsupported.

Native STEP import preserves `POLYLINE` records with two or more points as
degree-one NURBS for both 3D edges and UV trims. Their knots retain the source
`[0,n-1]` parameterization and each source segment exactly. The generated
triangle regression covers two-point surface curves and p-curves with no
reported loading losses and area 50. A multi-segment outer edge with a hole
also retains its bend and face area. The strict planar adapter still accepts
only two-point polylines; it rejects longer polylines instead of collapsing
them to an endpoint chord.

Native edge geometry expressed solely as a `PCURVE` on a STEP plane or on a
linear extrusion of a line is lifted through that basis's affine parameter map.
Polynomial and rational UV controls retain their degree, weights, and knots in
3D. Serialized polygon-hole cases check bent polyline edges on both basis
types; rational quadratics check intermediate evaluation against each source
surface and p-curve. Other p-curve bases still require a separate exact
composition adapter. Supported sweeps can also use an affine-basis p-curve as
their directrix.

Degree-one, two-control-point rational 3D edges and UV trims are also supported
when their weights are finite and positive. Homogeneous source controls are
converted to Euclidean controls with separate weights, retaining knots and
nonuniform parameterization. Direct trim tests use weight ratios 1:4 and 4:1
at scales from `1e-100` to `1e100`, compare against an independent rational
interpolation formula, and reject zero, negative, or non-finite weights.
The STEP triangle regression also covers matching rational 3D/UV curves,
preserving weights 1 and 4, `[-3,7]` intervals, and area 50. The source loader
checks parameter correspondence before retaining explicit p-curves; an unmatched
rational UV parameterization paired with a uniformly parameterized 3D line can
instead produce a reconstructed line trim. This work does not establish general
rational curved-edge or curved-surface import through the strict planar API.

The `native_planar/curves` module owns these 3D-edge and UV-trim adapters,
including wrapper unwrapping and homogeneous-coordinate conversion. The shell
reader handles shared topology, surface construction, incidence and final
validation. Direct rational 3D-edge tests verify controls, weights, orientation
and nine evaluation stations independently, and reject invalid weights,
unrepresentable Euclidean controls, curved spans, and multispan curves.

Tests cover a cube's 8 vertices, 12 edges, 6 faces, vertex bounds, area 286,
and signed volume 315; correctly reversed face/bound orientations give volume
-315. Open triangles retain area and reject solid-volume queries, and a curved
surface fixture is rejected. A unit-declaration change leaves source-coordinate
results unchanged. These are generated fixtures, not live Rhino measurements.
Conservative surface-control bounds can exceed trimmed-face bounds and must not
be confused with exact object extents.

## Verified source data

`reported_trimmed_shell` in the STEP reader obtains a
`StepCompressedTrimmedShell` from Monstertruck. This retains vertices, shared
3D curves, face surfaces, directed edge uses, and optional face-local parameter
curves. A parameter curve contains both a UV curve and its supporting surface.

The generated cuboid regression
`parsed_cube_retains_shared_edges_and_face_local_trim_availability` verifies:

- Eight vertices, twelve shared edges, six faces, and no reported topology loss.
- Four directed uses closing each face boundary; each edge has two opposite uses.
- Edge evaluation at its parameter endpoints matches its indexed vertices.
- All 24 face-local UV trims are present. Five stations on each trim map through
  the face surface to the independently oriented straight edge within `1e-12`.

The loader already reverses a face-local trim when its edge use is reversed.
A native `BrepTrim` must preserve that boundary-oriented UV curve while setting
`reversed_3d` relative to the shared 3D edge. Reversing the UV curve again would
break boundary orientation. This regression is a generated straight-edged cube,
not evidence for all STEP trims, periodic seams, singularities, or Rhino parity.

The planar-hole regression uses a 10-by-10 outer square and a clockwise
2-by-2 inner square, both boundary orders, and both face senses. The loader
retains all eight vertices and edges and all eight exact UV trims without
reported losses. UV signed areas remain +100 and -4 regardless of face sense;
the boundary list retains its source order, including inner-first input.
The loader's `FaceBound` representation does not retain a distinct outer-bound
flag. Consequently a converter must not assume that the first loop is outer.
The native reader converts all four fixtures to eight-vertex, eight-edge native
faces with area 96, preserving face sense. A two-hole fixture additionally checks
12 shared edges, three loops, and area 0.87 after millimetre-to-centimetre conversion.

`BrepFace::try_from_polygon_boundaries` classifies the unique counterclockwise
outer boundary and moves it to the front without modifying any source trim.
It verifies certified straight-segment and degree-one polyline UV curves,
closure, nonzero sides and area,
absence of backtracking and self-intersections, strict hole containment, and
absence of hole intersections or nesting. Touching and numerically unresolved
regions are conservatively rejected. Normalized UV coordinates prevent raw
coordinate products from overflowing; an X-interval sweep prunes disjoint side
pairs (worst-case intersection checking remains quadratic). No tessellation is
used in this check or to replace imported geometry.

Tests include concave boundaries, collinear subdivisions, scale/translation
changes, malformed closure, and 1,176 rectangle-hole/order combinations checked
against independent interval predicates. STEP fixtures with outside, touching,
overlapping, or nested holes fail despite loss-free source-shell conversion.
Winding area is normalized separately for each loop, so a small hole is not
compared against the whole face's area. Contact and containment predicates use
signed line distances, not area-valued cross products compared to a length
epsilon. Regressions cover hole widths from `1e-4` through `1e-10` inside a
10-wide face, including both loop orders, separate nearby holes, and rejection
of nested holes; STEP decoding retains all eight vertices and edges. These
checks establish conversion and region validation, not downstream meshing
accuracy for features below the modelling tolerance.
General native
`Brep::try_new` validation currently verifies trim continuity, endpoint/surface
agreement, edge-use types, and winding, but does not establish those planar
region conditions; the polygon constructor establishes them before the native
STEP reader performs full model-space validation.

`BrepFace::try_from_certified_boundaries` also accepts closed rational NURBS
loops of degree at least two with at least three Bézier spans when their
strictly convex endpoint polygon, control sectors, and strictly increasing
control projections along each span chord certify a simple loop.
Exact control-hull checks place curved holes strictly inside a convex polygonal
outer boundary. A curved outer loop must
bow outside its endpoint polygon; holes must fit strictly inside that
polygon. Disjoint control bounds separate holes. Other curved loop
configurations remain unsupported. This does not change other B-rep constructors.

Native loop-winding validation also uses the range-safe UV normalization.
Single degree-one trims with same-sign weights contribute only their endpoints:
their exact path is a straight segment, so interior samples provide no additional
area information. Opposite-sign weights on a single linear span imply an interior
denominator zero and are rejected directly, including poles between sample
stations. Curved and multi-span trims retain sampling; this is not a general
rational pole detector for those curves.
Separately, boundary continuity validation checks every nonempty degree-one
span of both 3D edges and lifted UV curves for opposite-sign weights. Zero-width
spans at full-multiplicity knots are skipped: a sign change between separate
one-sided pieces is not an interpolated denominator zero. Regression tests
distinguish these valid joins from poles in either span of a linear B-spline.
Regressions check both winding directions with weight ratios up to `1e12`,
coordinates of magnitude `1e100`, and equal-weight loops spanning `-f64::MAX`
to `f64::MAX`. This avoids overflowing raw coordinate differences while testing
winding; it does not establish model-space area or meshing at those extremes.
Normalization scale discovery scans the input directly without allocating a
temporary array of relative coordinates. Tests cover every origin corner of a
square from `f64::MIN_POSITIVE` through `f64::MAX`, as well as empty and repeated
point inputs. The overflow fallback still scans the complete input in scaled
coordinates; this removes temporary storage, not the output polygon allocation.

## Native representation requirements

Native `BrepEdge` requires a `NurbsCurve`; `BrepFace` requires a `NurbsSurface`;
`BrepTrim` requires a `NurbsCurve2`, endpoint references, edge reversal, trim type,
isoparametric classification, and tolerances. Constructed B-reps must pass the
kernel's topology and geometric validation before document insertion.

The source curve variants include lines, polylines, conics, B-splines, NURBS,
surface curves, parameter curves, and intersection curves. Source surfaces
include elementary surfaces, sweeps, B-splines, and NURBS. Merely converting a
surface's geometric locus is insufficient: its UV parameterization must match
every trim, or trims must undergo the same verified parameter mapping.

The general native instance path reuses assembly placement, unit conversion,
import diagnostics, and document transactions from the planar path. It maps
supported B-spline/NURBS controls and knots without tessellation, including
face-local UV curves. Analytic circle and ellipse arcs on supported faces use
exact multi-span rational 3D and UV NURBS conversions. Bounded parabolas and
hyperbolas use exact quadratic conversions for 3D edges and UV trims. Cylindrical
and nonsingular conical faces with straight UV iso-trims spanning up to one turn use
exact rational patches, including paired `SEAM_CURVE` uses on a full-turn wall;
angular parameters within each arc are reparameterized consistently with their
trims. Straight UV iso-trims can be certified collinear higher-degree p-curves.
Ring-torus patches and spherical bands away from the poles with straight
UV iso-trims use exact rational biquadratic patches, including full-angle seam
strips. Linear extrusions of line, polyline, bounded conic, and B-spline/NURBS
directrices use exact tensor-product NURBS patches. Revolutions of those
directrices with straight UV iso-trims use exact rational patches over angles
up to one turn, including paired
full-turn seams. Polar singular trims remain unsupported. The path
follows the 3D leader of `SURFACE_CURVE` and `INTERSECTION_CURVE` sweep
directrices when that leader is one of these supported curves. It
validates shared topology in `Brep::try_new`. NURBS faces with multiple loops of
certified straight-segment or degree-one polyline UV trims use strict polygon
boundary validation, including loops represented by one closed polyline trim.
Certified Bézier-span NURBS UV holes and outer loops are accepted under the
containment and sector certificates above. Other curved multi-loop regions
remain unsupported.
Other surface types, periodic seam arrangements, and missing UV curves still
need representation adapters or explicit topology handling. The default mesh import
remains available for display of such files.
