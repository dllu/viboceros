# Editable STEP B-rep conversion boundary

[File-format support](file-formats.md) · [Native B-rep geometry](../crates/viboceros-geometry/src/brep.rs)

The `ImportStep` command currently produces display meshes. A low-level
`read_step_planar_shells` API now produces validated native planar B-reps from
source shell definitions; it is not yet integrated into document import.
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
removing source shell records leaves the plan unchanged. The native source-shell
reader does not consume this plan yet: shape-to-shell expansion, oriented shell
sense, and per-instance native geometry validation still need integration.

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
of plane/plane intersections), and line UV trims. Plane control rectangles
retain source UV coordinates. Shared edges, trim reversal, and face sense are
kept distinct. Every result passes native `Brep::try_new` validation.

Unsupported curves/surfaces, invalid polygon regions, missing trims, non-manifold edges,
and reported source-shell topology losses fail the entire request. No mesh
substitute is returned. General B-spline/NURBS surfaces and curved trims remain
unimplemented in this path.

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
It verifies degree-one, single-span UV curves, closure, nonzero sides and area,
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
STEP reader performs full model-space validation. This does not extend region
validation to arbitrary curved trims or change other B-rep constructors.

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

Beyond the implemented planar subset, conversion still needs representation adapters, trim
parameter/orientation conversion, shared topology construction, and explicit
handling of missing or unrepresentable data. Assembly transforms, unit conversion,
import loss reporting, and document transactions must also cover the native path.
The current mesh import remains available, but does not establish those remaining
native-conversion requirements.
