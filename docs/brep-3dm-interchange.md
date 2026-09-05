# Morphed B-rep 3DM interchange

[File formats](file-formats.md) · [B-rep morphing](brep-morphing.md) · [Oracle](oracle.md)

`three_dm_brep_interchange` fits a native B-rep, writes it through the OpenNURBS
bridge, and has both Viboceros and Rhino read that **same file**. Rhino does not
independently recreate or re-morph the input. This isolates serialization and
reader differences from the different approximation choices of the two fitters.
No import repair, tolerance enlargement, or geometry simplification is performed.

## Checks

The native reader first compares the decoded B-rep against its in-memory fitted
source. Both readers then emit matching records containing:

- Shared vertex/edge indices, face reversals, loop types, edge references,
  trim reversals and boundary/mated/seam/singular classifications.
- All vertex, edge and UV-trim tolerances and trim isoparametric flags.
- Complete NURBS definitions for edges, surfaces and UV trims: degrees,
  control points, weights, knots and native domains.
- 33 original-parameter samples per edge, an entire-domain 9×9 surface grid
  per face, and 33 surface-lifted UV samples per trim.

The fixed native serialization bound is `2e-12 + 1e-14 * max(|a|, |b|)` for
floating-point values; integer topology and record structure must agree exactly.
It is independent of the source morph fitting tolerance. Coefficient checks
complement finite geometric sampling, which alone is not a continuous error
certificate. These fixtures use clamped NURBS; they do not establish interchange
for every possible B-rep representation or knot discontinuity.

After import, both engines must produce a valid smooth-seam mesh at density zero
with `SimplePlanes=false`. The comparison checks closedness, manifoldness,
orientation, boundary-loop count and boundary closure. It does not require the
same mesh density, polygon layout, area approximation or vertex positions.
The native source-face boundary audit remains enabled. Rhino's mesh topology
uses the [exact-coordinate weld check](brep-meshing.md), with an assertion that
welding a private duplicate changes no polygon's ordered XYZ coordinates.

Rhino's public `File3dm.Read` and B-rep `IsValid` check the exported model before
recording or meshing. UV tolerances are read through
[`BrepTrim.GetTolerances`](https://mcneel.github.io/rhinocommon-api-docs/api/RhinoCommon/html/M_Rhino_Geometry_BrepTrim_GetTolerances.htm).
Owned file models, mesh parts, combined meshes, and meshing parameters are disposed
on success and failure. Host tests cover repeated reads, failed validation,
failed meshing/append, and shared artifact lifetime. The host assigns unique paths
in a private temporary directory, ignores caller-provided paths in `compare`
mode, and never derives filenames from operation IDs. The native probe refuses
to overwrite an existing artifact. Standalone native probes remove their own
temporary files; standalone Rhino probes need an existing native artifact.

## Cases and limits

`brep_3dm_interchange.json` contains eight cases: a disk, warped annulus,
outward/inward capped faces, and cubically lifted box, cylinder, cone and sphere.
The cubic map is `(x, y, z + x² + xy/4 + y³)`. Together these exercise holes,
shared edges, seam curves, singular pole trims, and independently fitted faces.
The disk mesh has one closed boundary; the annulus has two; the six solid meshes
have none. Face orientation is retained for the inward solid too.

The annulus and sphere use a source fit tolerance of `1e-5`; the other six use
`1e-6`. The sphere initially requested `1e-6` but exhausted the current fitter's
one-million-direct-sample budget before serialization. That tighter fit remains
unsupported for this case. Using the `1e-5` fitted sphere does **not** relax its
serialization comparison, which still uses the fixed bound above.

On the tested Rhino 8.32 build under isolated Xvfb/Wine/FEX, all eight models
pass import validity, topology/definition comparison and meshing checks. The
largest numeric cross-reader difference is `5.33e-15`.

```sh
tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/brep_3dm_interchange.json \
  --absolute-epsilon 2e-12 --relative-epsilon 1e-14
```

Timing covers repeated file reads and validation, including disposal of the
previous model. Fitting, export, geometry recording and meshing are excluded.
These small warm-cache fixtures and translated Rhino are not a general IO or
geometry-kernel performance benchmark. General Rhino compatibility remains incomplete.

This checkpoint passes 1,328 Rust tests, 29 Python tests, strict Clippy, all
120 native fixtures (1,210 operations), and 369 recorded curve comparisons at
their existing tolerances, in addition to the eight fresh cross-reader comparisons.
