# File formats

[Project overview](../README.md) · [Command reference](commands/README.md)

Both ASCII and binary STL are supported. 3DM import/export uses McNeel's
OpenNURBS toolkit and preserves points, point-cloud locations, lines, circular arcs, NURBS
curves, parameterized polylines, exact piecewise polycurves, untrimmed NURBS surfaces, mixed triangle/quad meshes, and editable
rational NURBS B-reps. Mesh faces retain their arity in 3DM round trips. B-rep
interchange retains shared vertices and edges, exact edge and
parameter-space trim curves, face surfaces and orientation, outer and inner
loops, boundary/mated/seam/singular trims, and modelling tolerances. Layer and
object state are also preserved, including the raw RGB display color, its
layer/object/material/parent source, and surface wire density. Named group
definitions and ordered membership survive round trips, including overlapping
and empty groups.
Standalone circle and ellipse objects are exported without approximation as
rational NURBS curves. Arc objects retain their analytic type and native domain.
Polylines retain their native object type and every vertex parameter;
degree-one NURBS remain NURBS instead of being classified by a knot-vector heuristic.
Unsupported object types and specialized B-rep trim forms are
counted and reported during import.
Eight [morphed B-rep cross-reader cases](brep-3dm-interchange.md) check actual
native exports in Rhino, including holes, seams, singular trims and usable meshes.

Polycurves remain composite objects with native line/arc/polyline/NURBS leaves,
independent parameter intervals, and rational control structure. Nested source
composites are flattened on a private copy. Circular segments retain angular
evaluation and endpoint-editing behavior; they are not silently converted to NURBS.
The bridge shares a versioned, validated binary codec
for B-reps and polycurves, checks payload sizes before allocation, and rejects
malformed or trailing data. The typed polycurve payload is version 2; the reader
also accepts version 1 NURBS-only payloads. It does not fit curves or average endpoints.
Free NURBS curves with internal full-order knots are decomposed into valid native
pieces before export. Connected pieces become PolyCurves; positional gaps produce
separate objects with the original attributes. Export reports the actual object
count without editing the document. See [full-order curve interchange](curve-3dm-interchange.md)
for parameter preservation, cross-reader tests, and remaining limits.
The [rational range adapter](rational-3dm-range.md) prevents silent homogeneous
coordinate underflow, chooses safe common weight scales when needed, and imports
subnormal-weight curves and surfaces through direct homogeneous division.
An independently generated Rhino 8 nested line/arc reference is retained in
`crates/viboceros-io/tests/fixtures/`, with its generator and provenance documented
alongside it. Tests check the analytic locus and subsequent round trip.

The low-level 3DM I/O model preserves standard, unitless, unset, and custom
length-unit metadata (`ThreeDmModel::units`). Custom names and finite,
positive metres-per-unit scales round-trip without rescaling coordinates;
new I/O models explicitly default to millimetres. The shared
`LengthUnitSystem` provides validated, checked conversion factors;
`Document::with_units` initializes explicit document units, and 3DM export
retains them. Default documents use millimetres. `Import3dm` converts file
coordinates to document units before editing the document. Its source-space
validation tolerance is converted too, so valid small features are not
discarded merely because their numerical coordinates are small in file units.
Unitless files retain coordinates; unset units and unrepresentable conversion
factors or transformed coordinates are errors. The low-level
`read_3dm_file` still reads raw file coordinates, while
`read_3dm_file_in_units` performs the conversion. Changing an existing
document's units is not yet implemented.

Initial STEP interchange uses the Apache-2.0 Monstertruck kernel to read
solid/shell B-reps and assemblies, apply instance transforms, and robustly
tessellate exact trimmed surfaces into validated display meshes. Repeated
assembly instances share source-space tessellation during each import, but
each transformed mesh is validated independently. Cached tessellations are
released after their last instance; shell-conversion losses are reported
once per source shape, not once per instance. Parser,
topology, and unsupported-representation losses are reported instead of being
silent. STL and STEP export tessellate visible NURBS surfaces and B-rep faces;
exact outer and inner p-curves are sampled into a constrained UV triangulation
so holes remain open, with interior knot-span samples refining nonplanar
trimmed surfaces. STEP writes the results as faceted shells with shared
topology and planar faces. `ExportStep` converts physical document units to
millimetres and writes the correspondingly converted absolute tolerance as
the file's distance accuracy. Unitless and unset documents are rejected;
conversion failures leave an existing destination unchanged. The low-level
`write_step`/`write_step_file` APIs interpret coordinates as millimetres;
their `_in_units` counterparts accept explicit source units and tolerance.
Editable STEP B-rep interchange and production
surface and solid modelling are not implemented yet.

Assembly regression tests include repeated parts beneath a translated,
rotated parent, with expected corner coordinates checked independently of
the importer's matrix arithmetic. These are generated STEP fixtures, not
Rhino parity measurements. `ImportStep` resolves SI prefixes and
conversion-based length units (including nested conversion factors), then
converts coordinates and validation tolerances into document units. This
currently requires a single data section with uniform length units across
contexts. Missing, mixed, cyclic, or unsupported unit definitions and
non-radian angular contexts are rejected before document edits. Mixed-unit
assembly conversion remains unimplemented. The low-level `read_step` and
`read_step_file` APIs retain raw file coordinates; their `_in_units`
counterparts perform checked conversion.
