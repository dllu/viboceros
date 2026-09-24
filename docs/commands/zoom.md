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
prompts, selection, and undo/redo. Bare `Zoom Factor` opens a numeric prompt;
invalid values leave it open for another entry, while Enter or Esc cancels it.
The prompt applies to the viewport active when it started. `Zoom All Factor` is
unsupported, matching Rhino's listed All options.

`Zoom` or `Zoom Window` starts a viewport drag: press the left mouse button,
draw a rectangle, and release. Ctrl/Cmd+W starts the same action. The chosen
rectangle is centered and enlarged to fit within the viewport while preserving
its aspect ratio. A drag shorter than two pixels in either direction leaves the
mode active for another attempt. Esc or a right-click cancels it. Drawing the
window temporarily takes precedence over point drafting and object selection;
the unfinished modeling prompt, selection, and undo/redo history remain intact.
Parallel views change scale and pan; perspective views move the target on its
depth plane and change camera distance while retaining the lens and orientation.
Camera limits may prevent an exact fit. The live Rhino camera comparison remains
pending because recent Wine launches crashed or timed out.

`Zoom Target` or `ZT` first accepts a world/CPlane point for the new view center,
then a second point or viewport click to size a centered window. During mouse
input, object snaps and grid snapping apply to the center pick. The second
point defines a window corner; the fitted window keeps the viewport aspect
ratio. The chosen point becomes the camera rotation target and appears at the
center of the view. Perspective uses its original camera depth at that point
to calculate the new distance, preserving the view orientation and lens.
Parallel views change scale and clear screen pan. Both phases preserve an
unfinished modeling command and model undo/redo. Esc, Enter, or a right-click
cancels the target prompt; a tiny window leaves it open for another corner.
This follows [Rhino's Target option](https://docs.mcneel.com/rhino/8/help/en-us/commands/zoom.htm),
but the live Rhino camera comparison remains pending.

`Zoom In` and `Zoom Out` take one center-focused step in the active viewport.
The initial View zoom scale factor is 0.9: In multiplies target-plane
magnification by 1/0.9; Out multiplies it by 0.9. Set another finite positive
scale with `Options View Zoom ScaleFactor=<number>` or the View options menu.
Values above 1 reverse the apparent direction of In and Out, as in Rhino.
The setting applies across all viewports, shares Factor's camera limits, and
preserves unfinished modeling prompts and model history. It is saved in the
per-user application settings and restored after restarting Viboceros; invalid
saved values fall back to 0.9. Rhino documents the
[View zoom setting](https://docs.mcneel.com/rhino/8/help/en-us/options/view.htm),
and the [McNeel forum explanation](https://discourse.mcneel.com/t/navigation-controls-customization/18619/12)
ties the command options to that setting. A live Rhino camera comparison remains
pending because recent Wine launches crashed or timed out before the worker ran.

Perspective wheel zoom pins the point under the pointer on the camera-target
plane (through the target, perpendicular to the viewing direction), rather than
intersecting world Z=0. This remains defined after retargeting and when the world
XY plane is edge-on. The camera and orbit target translate laterally along the
cursor ray as camera distance changes. The lens, projection center, orientation,
and construction plane are unchanged. Perspective pan also translates the
camera and target instead of shifting the projection center.
Parallel and perspective zoom share an f64 screen-space pan calculation and
reject an unrepresentable final pan before committing scale or camera distance.
Tests cover translated targets, positive/negative/zero camera pitch, zoom in/out,
and large intermediate screen-coordinate differences.

The camera target moves to the center of the combined visible-object bounds,
pan resets, and the existing view orientation is retained. Parallel views change
scale; Perspective changes camera distance without changing its lens. The default
`SetZoomExtentsBorder` factors are `ParallelView=1.1` and `PerspectiveView=1`:
the parallel factor leaves about 4.55% of the viewport's limiting dimension on
each side,
while perspective fits the bounding box up to the viewport edges. Set either
factor independently with `SetZoomExtentsBorder ParallelView=<number>` or
`PerspectiveView=<number>`, or use the View options menu. A value below 1 crops
the fitted bounds. The settings apply to Extents and Selected in active and all
viewports and persist between sessions. See [Rhino's border option](https://docs.mcneel.com/rhino/8/help/en-us/commands/zoom.htm#setzoomextentsborder)
and [McNeel's default setting example](https://discourse.mcneel.com/t/zoom-problems-in-parallel-views-v7-src21/146152/13).

Hidden objects/layers are excluded; visible locked geometry is included. Fits use
the existing conservative display bounds, so NURBS control hulls can leave extra
space. Degenerate/small bounds may leave more space because parallel scale is
capped at 2,000 pixels per model unit and perspective distance is at least 0.01.
A point-sized scene uses the default scale/distance after centering.

The action preserves geometry, selection, model undo/redo, construction planes,
and unfinished modeling prompts. An empty visible scene leaves the camera alone.
The viewport must have been laid out at least once. Fitting validates the actual
target-relative GPU conversion of every bounding-box corner, not its absolute
world-coordinate magnitude. Small screen-plane extents can therefore fit at very
large parallel-view depths, and an isolated finite point can be centered even
at the f64 coordinate limit. Unrepresentable local extents, scales below the
supported minimum, or perspective distances above 1e9 still fail before camera
mutation. GPU vertices are rebased around the fitted target in f64 before
conversion to f32. This does not recover detail already lost in the model's f64
coordinates; see [GPU tests](../gpu-tests.md).

`src/viewport/extents.rs` owns fitting. A model-space target is shared by CPU
projection, unprojection, drafting rays, perspective depth, and GPU matrices.
Tests fit translated boxes in the four default viewports, compare GPU and CPU projections,
check picking and unprojection, reject unsupported ranges, exclude hidden
geometry, and execute ZE and ZS during a modeling prompt with redo history present.
All-view tests cover a late perspective failure after valid parallel fits,
missing layout, successful independent fits, and all four command spellings
during an unfinished modeling prompt. Selection-fitting tests exercise the four
default viewport kinds, irrelevant unsupported geometry, empty-selection
no-ops, and retained selection/model history.
No live Rhino camera comparison has been performed for this implementation.

`UndoView` and `RedoView` step through the active viewport's camera history,
separately from document undo and construction-plane undo. Home and End trigger
them when no text field is focused. The typed commands remain available while a
modeling prompt is unfinished. Each successful Zoom Factor, In/Out, Window,
Target, Extents, or Selected action records one camera step; All records one step in
each affected viewport. Wheel zoom records each scroll update, while a mouse
pan or orbit drag records one step when released. Invalid or unchanged actions
leave history alone, and a new camera action after UndoView discards the redo
branch. Each viewport retains up to 50 prior camera states. View presets and
`SetView World` enter camera history; display modes and construction-plane edits
remain outside it.
See [Rhino's UndoView and RedoView commands](https://docs.mcneel.com/rhino/8/help/en-us/commands/undoview.htm).

Other Zoom options remain unimplemented.
