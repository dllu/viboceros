# Mesh vertex channels

[Geometry guides](README.md) · [SubD control nets](subd-control-net.md)

`TriangleMesh` stores optional texture coordinates and vertex normals alongside
positions, faces, n-gons and vertex colors. Each channel has one entry per raw
vertex. Coincident vertices may carry different UVs at a texture seam.

```rust
let uv = mesh.texture_coordinates(); // Option<&[[f64; 2]]>
let normals = mesh.vertex_normals(); // Option<&[Vector3]>
```

The setters check channel lengths and finite float range, and round to native
single precision. Stored normals retain zero and nonunit values on assignment.
Affine transforms update normal directions; singular transforms use OpenNURBS
to recompute them from the transformed faces. Face records can remain stored
after a transform collapses their geometry.

Subsets and compaction copy attributes by source vertex index. Refinement
interpolates channels from raw endpoints, preserving separate seams. Appending
meshes retains a channel only when every input provides it. A raw vertex edit
retains stored attributes; operations that rebuild normals do so explicitly.

3DM import and export use native mesh texture and normal arrays. The internal
bridge payload accepts legacy version 2 and adds version 3 only for these
channels. STL has no corresponding attribute storage.

The [native comparison](../mesh-channels-qualification.json) retains 28 SDK
operations on a packed textured cube and a mesh with an n-gon. Twenty-seven
match within `1e-6`, including stored channel ordering. Face cycles are compared
up to rotation, and n-gon boundaries by membership. The textured cube's Weld
case differs in vertex order and seam averaging; it remains unqualified.
Two native 3DM fixtures also round-trip with exact channel values. Broader edit
compatibility, texture display and comparative performance remain unqualified.

Whole-mesh welding now blends stored UVs, colors and normal directions instead
of copying one seam value and rebuilding geometric normals. It preserves
custom stored normals. The [expanded Weld comparison](../mesh-weld-channels-qualification.json)
records 47 valid cases: 24 match and 23 retain explicit differences in traversal,
ordering or attribute reductions. Nine invalid native inputs remain diagnostics.
Three command regressions check metadata, groups and Undo/Redo. Broader Weld
parity remains open; passing regressions do not qualify the differing cases.
