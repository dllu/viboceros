# Imported surface shading

[Viewport controls](interface.md) · [GPU tests](gpu-tests.md)

Shaded and Ghosted views need triangles as well as the B-rep's display wires.
An imported B-rep can have valid trimmed faces even when the mesher cannot
construct a certified, conforming mesh across all its edges. Previously a
meshing error silently left that object with wires only.

The display cache now requests `Brep::display_mesh`. This first tries the
conforming tessellation, then falls back to independently sampled trimmed faces.
The fallback preserves trim holes and face orientation, but can have small gaps
between separately sampled faces. It is a cached display approximation, not a
replacement for the document's exact B-rep. Export, Mesh, and geometric operations
continue to use their existing meshing APIs and shared-boundary audits.
The fallback rejects unsupported internal positional breaks instead of bridging
them with invented triangles.

Two fixes also improve the underlying tessellator:

- Constrained triangulation classifies connected regions using their largest
  triangle, rather than separately classifying tiny exterior triangles along
  almost-collinear boundary samples. This avoids treating floating-point slivers
  outside the trim as material. Trim-area and boundary checks remain in place.
- Trimmed face meshes omit facets that collapse at surface poles or after
  boundary snapping, as the surface-grid mesher already does. Retained facets
  use numerical mesh validation, so the document's modeling angle does not
  reject narrow nonzero triangles.

A local 3DM can be checked without opening or changing a desktop window:

```sh
VIBOCEROS_3DM_FIXTURE=/absolute/path/model.3dm \
  cargo test --release -p viboceros imported_shading_tests -- --ignored --nocapture
```

The test imports the file through the command system, requires a nonempty display
mesh for every surface/B-rep, fits each object independently, and checks actual
filled GPU pixels with wires omitted. Set `VIBOCEROS_SHADING_IMAGES` to an existing
directory to save the individual renders as PPM images. Private model files are
not added to the repository.
