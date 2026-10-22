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
operations on a packed textured cube and a mesh with an n-gon. All 28
match within `1e-6`, including stored channel ordering. Face cycles are compared
up to rotation, and n-gon boundaries by membership.
Two native 3DM fixtures also round-trip with exact channel values. Broader edit
compatibility, texture display and comparative performance remain unqualified.

Whole-mesh welding now blends stored UVs, colors and normal directions instead
of copying one seam value and rebuilding geometric normals. It preserves
custom stored normals. The [expanded Weld comparison](../mesh-weld-channels-qualification.json)
records 96 valid cases: 71 match and 25 retain explicit differences in traversal,
ordering or attribute reductions. Seven differences repeat cached radial-sort
variants of existing scenes. The remaining scenes include face and raw-vertex
permutations, triangulated cubes and grids; all 12 open and closed fan cases match. Seventeen
invalid native inputs remain diagnostics. Eight diagnostics retain raw vertices
before compaction; compacting them reproduces their separately captured valid
twins exactly.
Four transparency profiles match across three source meshes, including exact
OpenNURBS alpha bytes (zero is opaque). Eight cached radial-sort variants reproduce
their native twins exactly. Fifteen command cases check metadata, groups and
Undo/Redo. Broader Weld parity remains open.

Angle unwelding exposes `unwelded_vertices_with_normal_update(angle, modify_normals)`.
Preserve mode copies source normals to separated vertices. Update mode recomputes
only replacement normals through OpenNURBS; untouched vertices retain their stored
values. Neither mode creates a missing normal channel. The
[native comparison](../mesh-unweld-channels-qualification.json) matches all 18
captured cases and retains 27 command comparisons, including the default update
policy. Broader normal reductions, non-manifold cases, and edge/vertex command
normal options remain separate work.
