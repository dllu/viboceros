# SubD control-net meshes

[SubD geometry](../subd.md) · [Native observations](../oracle/control-polygon.md)

`SubD::control_net_mesh(priority)` converts a private decoded copy through public
OpenNURBS. It returns a `TriangleMesh` with stored vertex normals and texture
coordinates. It preserves native face order, crease seams and n-gon boundaries
and member-face indices.

```rust
let mesh = subd.control_net_mesh(SubDControlNetPriority::Geometry)?;
let normals = mesh.vertex_normals();
```

Geometry priority preserves control topology and crease information. Texture
priority uses `SubDControlNetPriority::TextureCoordinates`: its face vertices are
duplicated to carry texture corners. The captured cube has eight geometry vertices
and 24 texture vertices. Both priorities use ordinary [mesh vertex channels](mesh-channels.md),
which survive document storage and 3DM interchange. `control_mesh()` remains a
geometry-priority convenience method and retains n-gon membership.

This is a control net, distinct from the separately evaluated limit surface and
its display tessellation. Source archives and geometry stay unchanged. Native
buffers have bounded counts and validated n-gon spans; Rust copies them before
their owner is released.

The [qualification](../subd-control-net-qualification.json) compares 21 SubD
sources from 20 native workflows. Points agree within `1e-6`, with zero observed
error; topology, n-gons, stored normals and texture coordinates match exactly.
Whole-object command integration has its own
[qualification](../control-polygon-qualification.json). Subcurve/face picking
and comparative performance remain separate work. Textured seam welding has a
retained compatibility gap.
