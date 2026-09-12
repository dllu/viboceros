# Editable STEP B-rep conversion boundary

[File-format support](file-formats.md) · [Native B-rep geometry](../crates/viboceros-geometry/src/brep.rs)

STEP import currently produces display meshes, not editable native B-reps.
The mesh route must not be relabeled as an exact B-rep import: tessellation
discards the source surface definitions, shared curve identities, and UV trims.

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

An implementation therefore still needs exact representation adapters, trim
parameter/orientation conversion, shared topology construction, and explicit
handling of missing or unrepresentable data. Assembly transforms, unit conversion,
import loss reporting, and document transactions must also cover the native path.
The current mesh import remains available, but is not proof of any of these
unimplemented native-conversion requirements.
