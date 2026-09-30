# Zoom

[Interface commands](interface.md) · [Viewport controls](../interface.md)

`Zoom Extents` or `ZE` fits visible geometry in the active standard parallel,
CPlane Plan, Perspective, or TwoPointPerspective viewport. Names and options
accept case-insensitive Rhino-style prefixes. This implements the active-view Extents action described in
[Rhino's Zoom documentation](https://docs.mcneel.com/rhino/8/help/en-us/commands/zoom.htm).

`Zoom Selected` or `ZS` uses the same fitting policy but only includes visible
selected objects. Distant unselected objects do not affect the fit, even if
their coordinates exceed the supported camera/GPU range. An empty eligible
selection leaves the view unchanged; this action does not start a selection
prompt or change which objects are selected.

`ZoomEnds` and `ZoomEnds All` fit the active view to the natural end markers of
visible selected curves. Open curves contribute both endpoints; closed curves
contribute their seam, and polycurves also contribute segment joints. Interior
control points and unselected geometry do not enlarge the fit. Empty eligible
selection leaves the view unchanged. The command preserves selection, unfinished
modeling input, document history, and independent viewport history.

`ShowEnds` displays the markers of currently selected visible curves in every
viewport. With no eligible preselection, it starts curve picking: click or
window curves in a viewport, then press Enter to finish or Esc to restore the
previous display. This temporarily takes viewport input from an unfinished
modeling or view prompt, which resumes afterward. The End Analysis menu filters
open starts, open ends, closed seams,
and polycurve joints. Green, blue, magenta, and purple identify those categories;
the current marker has an orange outer ring. Right-click a category checkbox
to show only that category; right-click it again to show all categories. The
menu can switch to a single custom marker color and add or remove currently
selected curves from the live analysis without editing the model. Its Pick
controls also collect curves with viewport clicks or windows; Esc restores the
prior source list. `ZoomEnds`
fits the markers enabled in the active
End Analysis session, even if selection changes afterward. `ZoomEnds Current`
fits the current marker; `Next` and `Previous` cycle through enabled markers,
wrapping at either end. These options require an active End Analysis session.
`ZoomEnds Mark` creates one point object at the current marker, or one per
enabled marker if `ZoomEnds All` was the last successful zoom. All points share
one undo step. Source edits and visibility changes update the displayed markers.
`ShowEndsOff` or Close hides
them. A fresh `ShowEnds` replaces the session from the current selection; an
empty eligible selection leaves the existing session in place until a new curve
is picked. The session does not
create document objects or history entries until `Mark` is used. Rhino's
[End Analysis documentation](https://docs.mcneel.com/rhino/8/help/en-us/commands/showends.htm)
describes these controls in Rhino.

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
pan resets, and the existing view orientation is retained. Imported off-axis target
offsets and shifted frusta are cleared. Two-point projection locks remain active.
Parallel views change scale; Perspective changes camera distance without changing
its lens. Fitting uses the full box in camera coordinates, adds a small depth
allowance, and expands screen dimensions to the viewport aspect ratio. The default
`SetZoomExtentsBorder` factors are `ParallelView=1.1` and `PerspectiveView=1`:
the parallel factor leaves about 4.55% of the viewport's limiting dimension on
each side,
while perspective fits the camera-coordinate box ahead of its nearest face.
World-space corners can therefore leave extra room in an oblique view. Set either
factor independently with `SetZoomExtentsBorder ParallelView=<number>` or
`PerspectiveView=<number>`, or use the View options menu. Values below 1 are
retained but fit as 1, matching Rhino 8.32 captures despite the help page's
description of smaller borders. The settings apply to Extents and Selected in
active and all viewports and persist between sessions. See [Rhino's border option](https://docs.mcneel.com/rhino/8/help/en-us/commands/zoom.htm#setzoomextentsborder)
and [McNeel's default setting example](https://discourse.mcneel.com/t/zoom-problems-in-parallel-views-v7-src21/146152/13).

Hidden objects/layers are excluded; visible locked geometry is included. Fits use
the existing conservative display bounds, so NURBS control hulls can leave extra
space. A screen-space box with both half-extents at most `sqrt(f64::EPSILON)`
uses a one-unit square before aspect fitting, independently of the border factor.
Small nondegenerate models can fit above 2,000 pixels per model unit or below
0.01 perspective camera distance. Other navigation actions still have their own
documented limits.

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

`src/viewport/extents.rs` owns fitting; `src/viewport/clipping.rs` computes
document near/far distances and any parallel camera depth adjustment. After
Extents, Selected, and their All variants, clipping uses the combined visible
scene bounds, including unselected geometry. Hidden objects and hidden layers
are excluded. This can move a parallel camera along its view direction while
preserving its target and screen framing. Fitting and clipping are staged
together before any camera or history changes.
A model-space target is shared by CPU
projection, unprojection, drafting rays, perspective depth, and GPU matrices.
Tests fit translated boxes in the four default viewports, compare GPU and CPU projections,
check picking and unprojection, reject unsupported ranges, exclude hidden
geometry, and execute ZE and ZS during a modeling prompt with redo history present.
All-view tests cover a late perspective failure after valid parallel fits,
missing layout, successful independent fits, and all four command spellings
during an unfinished modeling prompt. Selection-fitting tests exercise the four
default viewport kinds, irrelevant unsupported geometry, empty-selection
no-ops, and retained selection/model history.
The [Zoom camera fixture](../../tools/rhino_oracle/fixtures/zoom_extents_camera.json)
and [Rhino 8.32 capture](../../tools/rhino_oracle/observations/zoom_extents_camera.json)
cover 85 private-Xvfb fits: all six parallel directions, ordinary and two-point
perspective, shifted frusta, varying lenses and borders, translated boxes,
elongated/thin geometry, isolated points, and tiny models. Native replay checks
camera targets, axes, projection, and optical frusta to `2e-12`, CPU screen
coordinates to `1e-8` pixels, GPU agreement within f32 rounding, and retained
selection/model/CPlane history. Parallel and perspective camera locations match;
all explicit public ZoomBoundingBox cases also match clipping distances.
The [clipping captures](../../tools/rhino_oracle/observations/viewport_clipping.json)
add 42 document fits and 39 public constrained near/far setter cases.
[Context captures](../../tools/rhino_oracle/observations/viewport_clipping_context.json)
add 72 Selected fits with visible/hidden unselected geometry in Wireframe,
Shaded, and Ghosted modes. Replay checks initial and redraw near/far distances
separately at relative tolerance `2e-12`. The document clipping calculation
handles thin and tiny models, minimum distances, and near/far ratio constraints.
General navigation/redraw does not yet refresh the stored clip interval;
rendering continues to use its separate dynamic depth range. Curve-end fitting
also retains the explicit bounds fit rather than calibrated document clipping.
Twenty additional [border captures](../../tools/rhino_oracle/observations/zoom_extents_borders.json)
verify factors 0.1, 0.8, 1, 1.5, and 10 through both the public settings API and
`SetZoomExtentsBorder`. Settings retain the requested value; the minimum fit
factor is 1.

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
