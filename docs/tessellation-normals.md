# Surface tessellation channels

`NurbsSurface::tessellate_with_normals(samples_per_span, tolerance)` and
`Brep::tessellate_with_normals(samples_per_span, tolerance)` return a
`SurfaceTessellation`: the ordinary `TriangleMesh` plus three
`TessellationCorner` records per triangle, in the mesh's triangle/corner order.
The ordinary tessellation API and mesh representation remain available.

Each corner stores its local surface UV parameters and additive `uv_origin`,
source B-rep face index
(`None` for a standalone surface), optional oriented unit normal, and
`TessellationNormalSource`. For example:

```rust
let output = surface.tessellate_with_normals(8, tolerance)?;
for (triangle, corners) in output.mesh.triangles().iter().zip(&output.corners) {
    for (&vertex, sample) in triangle.iter().zip(corners) {
        let position = output.mesh.vertices()[vertex as usize];
        // Retain sample.uv + sample.uv_origin as a pair with the normal/face.
    }
}
```

Regular normals use the scale-independent surface normal evaluator. Incident
triangle parameters choose one-sided derivatives at knot breaks; B-rep face
reversal changes the normal's sense. Separate corners preserve intentional
creases even at coincident positions. For B-reps, normals are evaluated in the same translated parameter chart as the
mesh. Retain `uv` and `uv_origin` separately: their floating-point sum may erase
local samples in very large native parameter domains. UV samples survive irregular trim sampling
and the shared-edge conforming fallback. Boundary positions can be snapped to
exact topological edges within the supplied tolerance.

At a singular parameter, the API attempts a numerically converged approach along
the incident triangle's UV ray. Such normals are marked `SampledLimit`, not
certified as path independent. An unresolved corner has `normal = None` and
`normal_source = Unavailable`; other corners retain their channels. Exporters
may use that triangle's geometric normal as a local fallback.

Channels are opt-in export data. Existing mesh editing and interchange functions
accept `TriangleMesh`, so custom exporters must retain these accompanying
channels themselves. The ordinary display tessellation does not allocate them.

Regression coverage includes bilinear surface derivatives, one-sided crease
normals, singular collapsed corners, reversed B-rep faces, irregular trimmed
holes, and conforming seams between unequal face grids:

```sh
cargo test -p viboceros-geometry tessellation:: --lib
```
