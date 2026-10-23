# Polygon meshes

[Meshes and SubDs](meshes-index.md) · [Command reference](README.md)

Polygon meshes retain triangle/quad faces, logical n-gons, vertex channels,
and exact-location topology. See [mesh topology and editing](../mesh-topology.md)
for kernel ownership, numerical limits and regression evidence.

## Meshing and mesh primitives

[Meshing and primitives](mesh-primitives.md) covers `Mesh`, polygonal primitives,
and conversion to NURBS.

## Mesh extraction and editing

[Extraction and editing](mesh-editing.md) covers offsets, n-gons, face and edge
extraction, triangulation, quadrangulation, edge edits and hole filling.
[InsertPoint](insert-point.md) adds staged edge paths to meshes and SubDs.

## Direction, welding, and topology

[Direction and topology](mesh-topology-commands.md) covers normals, alignment,
extrusion, compaction and decomposition. [Mesh seams](mesh-seams.md) documents
welding and unwelding, including vertex channels and compatibility limits.
