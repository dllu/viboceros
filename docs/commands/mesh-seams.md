# Mesh seams

[Mesh commands](meshes.md) · [Stored vertex channels](../geometry/mesh-channels.md)

`WeldEdge` and `UnweldEdge` also support [SubD edges](subd-edge-seams.md).
`WeldVertices` and `UnweldVertex` also support [SubD vertices](vertex-seams.md),
with their own tag, grip-selection and history behavior.

Seam edits preserve object identity, attributes, groups and selection, and commit
one undo step. Invalid components or options reject the whole edit. Component
selectors take unique indices; the geometry APIs also accept repeated indices.

## Welding

`Weld` merges exactly coincident endpoints where faces share a whole edge and
their normal angle is within its tolerance. The default is 180 degrees;
`Angle=90` is also accepted. It compacts unused vertices, keeps vertex-only
contacts separate, and blends stored UVs, colors and normal directions.
Connected-face ordering and some attribute reductions still differ from Rhino;
see the [measured limits](../geometry/mesh-channels.md).

`WeldEdge` merges raw endpoint sets at selected exact-location topology edges.
Use `Edges=0,2`, `Edges=All`, or omit the selector to pick one edge. The first
incident face supplies the surviving vertex; unrelated fan components remain separate,
including at half-welded and non-manifold seams. Naked or already-welded
selections still compact unused vertices. Stored normals average per incident
face use, including repeated uses of a shared non-manifold endpoint. UVs and
colors retain the survivor's values.

`WeldVertices` welds every joined seam incident to selected topology vertices,
including both endpoints of each seam. Use `Vertices=0,2`, `Vertices=All`, or
omit the selector for one viewport pick. It walks radial face fans, ignores
coincident vertex-only contacts, limits non-manifold edges to their first two
face uses, and compacts unused vertices. It blends UVs, stored normals and colors
at new joins; a fan's closing join copies its established side. Overlapping
selections update individual face corners rather than globally merging every alias.

These commands derive missing normal data when an edit needs it, preserve custom
directions, and retain zero directions when opposing normals cancel. A change
to normal data alone does not create a command history entry.
The [component welding comparison](../mesh-component-weld-qualification.json)
records 56 cases: all match positions, winding and raw sharing; 52 also match
stored channels and raw numbering. Four captures of adjacent or all-vertex
selections retain explicit attribute or ordering differences.

## Unwelding

`Unweld` separates edges whose face-normal angle is greater than or equal to
its tolerance, retaining smoother face regions. The zero-degree default
separates every adjacent face region. `ModifyNormals=No` copies stored normals
to separated vertices; the default `Yes` updates replacement normals while
retaining untouched stored values. This whole-mesh command keeps a missing
normal channel absent. Its [comparison](../mesh-unweld-channels-qualification.json)
records 18 native cases and 27 local command checks.

`UnweldEdge` adds seams at selected exact-location topology edges. Use `Edges=0,2`,
`Edges=All`, or omit the selector to pick one edge. It partitions closed and
high-valence face fans while preserving existing seams. At a non-manifold edge,
the first two face uses are separated; additional uses retain their original
sharing. An endpoint with existing partial sharing stays unchanged.

`UnweldVertex` gives each face incident to a selected topology vertex its own
raw vertex. Use `Vertices=0,2`, `Vertices=All`, or omit the selector for a pick.
It separates the selected vertices and compacts unused vertices.

Both component commands honor `ModifyNormals=No` by copying source directions.
The default `Yes` updates affected normals, including smoothing across an extra
split needed to represent a single crease on a closed fan. Untouched vertices
keep their stored normals. Component edits can compute a missing normal channel.
Shaded and ghosted views use stored normal directions; zero normals fall back
to their face direction.
The [SDK normal-policy contract](https://mcneel.github.io/rhinocommon-api-docs/api/RhinoCommon/html/M_Rhino_Geometry_Mesh_UnweldEdge.htm)
describes copying versus recalculating directions.

The [component comparison](../mesh-component-unweld-qualification.json) checks
72 cases for geometry, winding, UVs, normals, colors and vertex sharing. Eight
multi-edge cases still differ in raw vertex numbering; these differences remain
explicit. Local command checks also cover picking aliases, default options,
metadata, selection and Undo/Redo.
