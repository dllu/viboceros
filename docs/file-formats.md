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

Initial STEP interchange uses the Apache-2.0 Monstertruck kernel to read
solid/shell B-reps and assemblies, apply instance transforms, and robustly
tessellate exact trimmed surfaces into validated display meshes. Parser,
topology, and unsupported-representation losses are reported instead of being
silent. STL and STEP export tessellate visible NURBS surfaces and B-rep faces;
exact outer and inner p-curves are sampled into a constrained UV triangulation
so holes remain open, with interior knot-span samples refining nonplanar
trimmed surfaces. STEP writes the results as faceted shells with shared
topology and planar faces. Editable STEP B-rep interchange and production
surface and solid modelling are not implemented yet.
