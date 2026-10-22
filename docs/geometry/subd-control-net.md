# SubD control-net meshes

[SubD geometry](../subd.md) · [Native observations](../oracle/control-polygon.md)

`SubD::control_net_mesh(priority)` converts a private decoded copy through public
OpenNURBS. The result contains a `TriangleMesh`, stored vertex normals and texture
coordinates. It preserves native face order, crease seams and n-gon boundaries
and member-face indices.

```rust
let result = subd.control_net_mesh(SubDControlNetPriority::Geometry)?;
let mesh = result.mesh;
```

Geometry priority preserves control topology and crease information. Texture
priority uses `SubDControlNetPriority::TextureCoordinates`: its face vertices are
duplicated to carry texture corners. The captured cube has eight geometry vertices
and 24 texture vertices. Texture priority's coordinates and normals accompany the
mesh in the result; ordinary document mesh-channel storage and interchange still
need integration. `control_mesh()` remains a geometry-priority convenience method
and now retains n-gon membership.

This is a control net, distinct from the separately evaluated limit surface and
its display tessellation. Source archives and geometry stay unchanged. Native
buffers have bounded counts and validated n-gon spans; Rust copies them before
their owner is released.

The [qualification](../subd-control-net-qualification.json) compares 21 SubD
sources from 20 native workflows. Points agree within `1e-6`, with zero observed
error; topology, n-gons, stored normals and texture coordinates match exactly.
The existing command's SubD integration, complete mesh-channel preservation and
comparative performance remain separate work.
