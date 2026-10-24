# Mesh split attributes and source faces

MeshSplit transfers stored UVs, normals and RGBA colors to new mesh vertices.
Coincident raw vertices retain distinct attribute values across texture or normal
seams. Original corners retain their values; new normals use normalized linear
interpolation. UVs and normals retain native float storage. Color bytes include
OpenNURBS transparency and are rounded during interpolation.

SMLib face attributes carry each target triangle's source index through imprints,
triangulation and component copies. The adapter maps those indices to original
triangle/quad faces. This removes the command's nearest-face inference, including
its spatial-index preparation and ambiguity on coincident source geometry.
`MeshChannelSource` prepares OpenNURBS' quad diagonals once per target and reuses
the shared channel builder. It duplicates output vertices when source faces
provide different stored values. Logical polygons are reconstructed afterward.

`CreateNgons=No` removes logical polygons from admitted results. An unsplit source
retained by `DiscardUnsplitMeshes=Yes` remains unchanged, including its existing
n-gons. Geometry and channel transfer finish before document changes, preserving
selection and Redo on failure. Source geometry storage returns unchanged on Undo.

The [qualification](mesh-split-channels-qualification.json) retains 21 new Rhino
workflows with preselection, triangles, quads, welded boxes, coplanar and folded
attribute seams, partial/disjoint cuts, existing n-gons, projected curves and
infinite planes. All 21 command and 21 picker workflows match bidirectional raw
vertex-channel samples, at `1e-6` for UVs/normals and one byte for colors; surface
geometry uses `1e-7`. Eighteen also match all n-gon checks. Three captured native
n-gon boundaries omit part of their covered faces on source seams; our overlays
retain complete boundaries. Native raw vertex order, triangulation, shading
between vertices, overlapping source faces and comparative performance remain
unqualified. Tiny/overflowing barycentric arithmetic and invalid source-face
maps have separate geometry tests.
