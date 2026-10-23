# RebuildMeshNormals

[Meshes and SubDs](meshes-index.md) · [Mesh editing](mesh-editing.md)

`RebuildMeshNormals` reconstructs stored mesh normals from the current face
winding. Start with meshes preselected, or start the command, select meshes and
press Enter. Escape or Cancel abandons command-first selection. There are no
numeric options.

The geometry API is `TriangleMesh::rebuilt_normals`. It removes unused vertices
in source order and remaps face references, UV coordinates, colors and n-gon
boundaries. Referenced vertices at identical positions remain independent, so
unwelded shading seams survive. OpenNURBS rebuilds face normals and sums their
float contributions into vertex normals, replacing absent, incorrect, zero or
nonunit stored normals. Cancelling contributions use world Z. Face order and
winding stay the same.

N-gon overlays survive. Processed meshes retain their IDs, attributes and groups,
move to the object table end and record one Undo step, including when their
normals were already correct. Preselected meshes stay selected after acceptance;
command-first picks are released. Undo restores source geometry and preselection;
Redo releases processed selection. Non-mesh selections are skipped. All results
are staged before mutation, preserving geometry, order, selection and Redo when
a late source fails conversion.

The [qualification record](../rebuild-normals-qualification.json) covers 19
independent SDK inputs and 22 native command workflows. It checks raw channels,
interspersed unused vertices, n-gons, unwelded seams, nonmanifold and opposed
faces, small geometry, cancellation, object order and Undo/Redo. Native document
admission populated initially absent normal channels; missing-channel rebuilding
is qualified separately through SDK inputs.

A collinear face remains stored and contributes zero. Its unit-direction query
is explicitly declined locally. Cached face-normal storage, general mesh repair,
physical native postselection, restricted objects, grips, numeric extremes and
comparative performance remain unqualified.

[Rhino command reference](https://docs.mcneel.com/rhino/8/help/en-us/commands/rebuildmeshnormals.htm)
· [Public SDK method](https://mcneel.github.io/rhinocommon-api-docs/api/RhinoCommon/html/M_Rhino_Geometry_Mesh_RebuildNormals.htm)
