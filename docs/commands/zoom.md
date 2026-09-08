# Zoom

[Interface commands](interface.md) · [Viewport controls](../interface.md)

`Zoom Extents` or `ZE` fits visible geometry in the active Top, Front, Right,
or Perspective viewport. Names and options accept case-insensitive Rhino-style
prefixes. This implements the active-view Extents action described in
[Rhino's Zoom documentation](https://docs.mcneel.com/rhino/8/help/en-us/commands/zoom.htm).

`Zoom Selected` or `ZS` uses the same fitting policy but only includes visible
selected objects. Distant unselected objects do not affect the fit, even if
their coordinates exceed the supported camera/GPU range. An empty eligible
selection leaves the view unchanged; this action does not start a selection
prompt or change which objects are selected.

`Zoom All Extents` / `ZEA` and `Zoom All Selected` / `ZSA` fit all four viewports.
Visible bounds are computed once, then each view gets its own orientation- and
aspect-ratio-aware fit. All fits are validated before any camera changes: a
missing viewport layout, GPU-range error, or perspective distance-limit failure
leaves every view unchanged. Active-viewport identity is preserved.

Ctrl/Cmd+Shift+E invokes active-view Extents; Ctrl/Cmd+Alt+E invokes All Extents.
Both work with coordinate text focused and leave partially typed input and
unfinished modeling prompts intact. Held-key repeats and matching key releases
are consumed without repeating the action; extra modifiers do not trigger either
shortcut. Tests exercise both Ctrl and macOS Command modifier representations.

`Zoom Factor 2` doubles the active view's target-plane magnification;
`Zoom Factor 0.5` halves it. Factors must be finite and strictly positive,
matching the documented [Rhino Factor option](https://docs.mcneel.com/rhino/8/help/en-us/commands/zoom.htm).
The viewport center stays fixed on the target plane, including after panning.
Parallel views change scale; perspective views change camera distance, retaining
the lens, model-space target, and construction plane. Existing camera limits
clamp extreme factors; factor 1, a factor below camera precision, or an
already-reached limit reports no change.
Factors are parsed and applied in f64, including finite values outside f32 range.
Missing layout, invalid arguments, or an unrepresentable resulting pan leave
the camera unchanged. Like the other Zoom actions, Factor preserves modeling
prompts, selection, and undo/redo. It currently requires an inline factor; bare
`Zoom Factor` does not open a numeric prompt, and `Zoom All Factor` is unsupported.

Perspective wheel zoom pins the point under the pointer on the camera-target
plane (through the target, perpendicular to the viewing direction), rather than
intersecting world Z=0. This remains defined after retargeting and when the world
XY plane is edge-on. The lens, target, and construction plane are unchanged.
Parallel and perspective zoom share an f64 screen-space pan calculation and
reject an unrepresentable final pan before committing scale or camera distance.
Tests cover translated targets, positive/negative/zero camera pitch, zoom in/out,
and large intermediate screen-coordinate differences.

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
The viewport must have been laid out at least once. Fitting conservatively rejects
absolute bounds outside the finite f32 range, or fits requiring a perspective
distance above 1e9, before camera mutation. GPU vertices are rebased around the
fitted target in f64 before conversion to f32, preserving small local features
far from the origin. Very large local extents and f64 model-coordinate rounding
still limit accuracy; see [GPU tests](../gpu-tests.md).

`src/viewport/extents.rs` owns fitting. A model-space target is shared by CPU
projection, unprojection, drafting rays, perspective depth, and GPU matrices.
Tests fit translated boxes in all four views, compare GPU and CPU projections,
check picking and unprojection, reject unsupported ranges, exclude hidden
geometry, and execute ZE and ZS during a modeling prompt with redo history present.
All-view tests cover a late perspective failure after valid parallel fits,
missing layout, successful independent fits, and all four command spellings
during an unfinished modeling prompt. Selection-fitting tests exercise all four view kinds, irrelevant unsupported
geometry, empty-selection no-ops, and retained selection/model history.
No live Rhino camera comparison has been performed for this implementation.

Other Zoom options (including Window and view history) and configurable extents
borders remain unimplemented.
