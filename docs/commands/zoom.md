# Zoom to extents

[Interface commands](interface.md) · [Viewport controls](../interface.md)

`Zoom Extents` or `ZE` fits visible geometry in the active Top, Front, Right,
or Perspective viewport. Names and options accept case-insensitive Rhino-style
prefixes. This implements the active-view Extents action described in
[Rhino's Zoom documentation](https://docs.mcneel.com/rhino/8/help/en-us/commands/zoom.htm).

The camera target moves to the center of the combined visible-object bounds,
pan resets, and the existing view orientation is retained. Parallel views change
scale; Perspective changes camera distance without changing its lens. All eight
box corners are fitted with a five-percent margin on each screen edge. Hidden
objects/layers are excluded; visible locked geometry is included. Bounds use
the existing conservative display bounds, so NURBS control hulls can leave extra
space. Degenerate/small bounds may leave more space because parallel scale is
capped at 2,000 pixels per model unit and perspective distance is at least 0.01.
A point-sized scene uses the default scale/distance after centering.

The action preserves geometry, selection, model undo/redo, construction planes,
and unfinished modeling prompts. An empty visible scene leaves the camera alone.
The viewport must have been laid out at least once. Bounds outside the GPU's
finite f32 coordinate range, or requiring a perspective distance above 1e9,
return an error before camera mutation. This does not eliminate f32 GPU precision
loss for geometry far from the origin.

`src/viewport/extents.rs` owns fitting. A model-space target is shared by CPU
projection, unprojection, drafting rays, perspective depth, and GPU matrices.
Tests fit translated boxes in all four views, compare GPU and CPU projections,
check picking and unprojection, reject unsupported ranges, exclude hidden
geometry, and execute ZE during a modeling prompt with redo history present.
No live Rhino camera comparison has been performed for this implementation.

Other Zoom options (including All, Selected, Window, and view history), zoom
shortcuts, and configurable extents borders remain unimplemented.
