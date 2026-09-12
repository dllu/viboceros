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

The supported subset has planar surfaces, one outer loop per face, straight
3D edges (including two-control-point degree-one B-splines and linear leaders
of plane/plane intersections), and line UV trims. Plane control rectangles
retain source UV coordinates. Shared edges, trim reversal, and face sense are
kept distinct. Every result passes native `Brep::try_new` validation.

Unsupported curves/surfaces, multiple loops, missing trims, non-manifold edges,
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
