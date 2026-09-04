# File formats

[Project overview](../README.md) · [Command reference](commands/README.md)

Both ASCII and binary STL are supported. 3DM import/export uses McNeel's
OpenNURBS toolkit and preserves points, point-cloud locations, lines, NURBS
curves, untrimmed NURBS surfaces, mixed triangle/quad meshes, and editable
rational NURBS B-reps. Mesh faces retain their arity in 3DM round trips. B-rep
interchange retains shared vertices and edges, exact edge and
parameter-space trim curves, face surfaces and orientation, outer and inner
loops, boundary/mated/seam/singular trims, and modelling tolerances. Layer and
object state are also preserved, including the raw RGB display color, its
layer/object/material/parent source, and surface wire density. Named group
definitions and ordered membership survive round trips, including overlapping
and empty groups.
Circles, arcs, ellipses, and polylines are exported without approximation as
rational NURBS curves; canonical degree-one curves return as editable
polylines. Unsupported object types and specialized B-rep trim forms are
counted and reported during import.

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
