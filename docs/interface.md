# Interface and drafting

[Project overview](../README.md) · [Command reference](commands/README.md)

The application opens with Top, Perspective, Front, and Right viewports.
Construction-plane grid lines clip against the perspective camera plane and then
against the viewport rectangle before stroke tessellation. A line with one endpoint
behind the camera retains its visible part, and near-plane projections never send
huge screen coordinates to egui. The grid remains a background drafting overlay;
it does not write model depth. Regression tests cover both endpoint orders,
multiple camera orientations, fully hidden lines, and bounded generated strokes.
Each supports wireframe, shaded, and ghosted display, with independent navigation.
Perspective wire segments crossing the camera plane are clipped before GPU
submission and click/crossing-selection projection, rather than disappearing
because one endpoint is behind the camera. The shared path covers lines,
polylines, sampled curves, and mesh wires. A scale-aware rounding guard keeps
reconstructed endpoints in front of the GPU near-plane floor; source geometry
is not modified. Regressions exercise both endpoint orders, line/polyline/NURBS
picking, crossing selection, and GPU submission.
Shaded and ghosted faces also retain their visible portion: CPU selection clips
triangles into at most two triangles without heap allocation, while GPU submission
keeps the original vertices for hardware clipping and smooth-normal interpolation.
Only the clipped face contributes to camera depth bounds. Tests cover all vertex
permutations with zero, one, two, or three hidden corners, winding preservation,
face picking, crossing selection, and CPU/GPU projection agreement. These are
submission and projection tests. Separate [offscreen GPU tests](gpu-tests.md)
check actual face and wire pixel coverage on the production renderer. Full
visibility parity and live Rhino comparisons remain unverified.
GPU positions are rebased around each viewport's model-space target in f64
before conversion to f32. Camera matrices and depth bounds use the same local
frame, preserving small features in models translated far from the origin.
Parallel views also apply screen-plane zoom scaling in f64 before GPU conversion.
Per-vertex depth stays f64 until the scene range is known, then maps to a bounded
GPU interval. This avoids absolute-depth cancellation as well as subnormal projection
coefficients when fitting uniformly enormous geometry; pixel tests cover a
`2^126` model-scale change with inverse zoom and a point at minimum zoom.
Additional pixel tests preserve the image after translation along the view-depth
axis to `2^1020`, and retain occlusion between depths distinguishable in f64 but
not in absolute f32 coordinates. Singleton and overflowing depth spans are covered.
Pixel tests cover translations of billions of units; CPU/GPU projection tests
cover trillion-unit offsets. This does not remove f64 model-coordinate rounding,
f32 precision loss across very large local extents, or all near-plane limitations.
When shaded or ghosted faces overlap under a click, the nearest face at the
cursor wins instead of whichever object was inserted first. The picking module
uses screen-space barycentric depth for parallel views and reciprocal-depth
interpolation for perspective, over the same clipped mesh triangles used for
face projection. It checks every candidate triangle, including tessellated
NURBS surfaces and B-reps. Tests cover all four views, both insertion orders,
all three face representations, and sloped/camera-crossing faces against
independent ray intersections. Point/curve feature priority and wireframe
capture remain unchanged; general hidden-point/curve rejection is still open.
Coincident or near-coincident same-rank click hits open a Selection Menu instead
of silently selecting the first object. The menu lists object names and types;
click an entry to choose it, click None or press Esc to cancel, click the original
pick location to cycle, or right-click/press Enter to accept the highlighted entry.
Clicking another object starts a new pick. The current choice is highlighted in
the viewport. Shaded faces at distinct depths retain their nearest-face result.
Moving over an entry changes the highlight. This is a basic choice interface;
mouse-wheel cycling and the full set of Rhino menu options are still open.
The compact toolbar contains Undo/Redo, active-viewport view/display selectors,
Grid Snap, Osnap, and SmartTrack. Modeling commands remain in the command line;
the toolbar wraps at narrow window widths. Undo/Redo buttons are disabled while
a modeling prompt is unfinished.
The layer pane creates, renames, recolors, shows, locks, activates, and deletes
empty layers; it reports object counts and combines edits into one undo step.
Scroll inside the pane to reach lower layers, new-layer controls, and groups in
large documents. The pane heading remains visible while its contents scroll.
Long layer and group names are truncated in rows, with full names on hover;
action buttons and the scrollbar have reserved space so neither obscures the other.
Rows use stable layer/group identities: removing a pressed row cannot redirect
the click to its replacement, and adding a layer preserves new-layer text focus.
Open layer editors adopt external changes to untouched fields while preserving
local drafts. Conflicting name/color edits block Apply with a warning; reopen
Edit to review current values. Deleting a layer closes its editor.

Osnap captures visible Point, End, Mid, Center, Quad, and opt-in Near features, including
indexed members of point clouds and features on locked objects and layers.
Runtime-hidden point-cloud members are excluded from Osnap.
The [capture aperture is square](snap-capture-box.md); admitted candidates retain
Euclidean distance scoring. Curve-hover targets can lie outside the aperture.
Mid uses half arc length on NURBS, individual polycurve segments and surface
boundaries, not the surface's UV center. With Mid alone enabled (including one-shot
Mid), hovering near any part of a curve segment captures its Mid; mixed-mode Mid requires
proximity to the target. Circle/ellipse Mid is opposite the stored seam. Center
captures by hovering near an analytic circle, arc or ellipse (including polycurve
arc leaves), or a closed polygonal boundary. Polygon targets average corners;
planar surfaces and hole-free planar faces use their polygonal boundaries.
Hovering near an empty center is insufficient. Direct features on the same object
take precedence.
Circular and [elliptical NURBS](elliptic-center-snaps.md), natural surface
boundaries and B-rep edges also supply Center, including eligible hole edges.
Recognition is conservative; unrestricted conic/approximate-conic parity remains incomplete.
The Snap modes menu selects individual persistent modes; right-click isolates a
mode and Shift-click selects it for one point. At point prompts, `Point`, `End`,
`Mid`, `Cen`, `Quad`, `Near` and `NoSnap` also supply one-shot overrides. See
[controls and lifecycle](object-snap-controls.md), [Mid/End behavior](composite-feature-snaps.md),
[Mid hover](mid-hover-snaps.md), [analytic Center](center-hover-snaps.md) and
[polygon Center](polygon-center-snaps.md) and [circular NURBS Center](circular-center-snaps.md)
for coverage limits, [Near](near-snaps.md) for screen-space curve targets, and
[snap caching](snap-caching.md) for invalidation and performance.
The separate **Snap to mesh wires** checkbox or `SnapToMeshes Enable` admits
[mesh Near/Mid](mesh-snaps.md). It defaults off, does not change enabled modes,
and preserves point prompts and one-shot overrides. Mesh Mid is direct-only;
Mesh Near uses calibrated endpoint-depth weighting and a
[both-endpoints-inside rule](mesh-snap-endpoints.md). Remaining differences include
corner/parallel/short-wire selection; see the
[full 3D point probes](point-snaps.md) and [competition follow-up](mesh-snap-order.md).
SmartTrack captures local plane-axis alignment from the first picked
point in every viewport. Grid Snap rounds construction-plane picks to the unit grid. Right-drag
pans parallel views and rotates the Perspective view; Shift-right-drag pans the
Perspective view, middle-drag pans any view, and the mouse wheel zooms.
Perspective pan translates the camera and its orbit target in world space.
Wheel zoom dollies along the cursor ray, retaining a fixed field of view and a
centered projection; repeated zooms toward objects far from the origin do not
accumulate an off-axis lens shift. A plain
right-click acts as Enter. Outside a drafting command, left-drag from left to
right selects only fully enclosed objects, while right-to-left makes a crossing
selection. Click geometry to replace the selection, Shift-click/drag to add,
and Ctrl-click/drag or Command-click/drag to remove. Click empty space or press
Esc to clear the selection; press Delete to remove selected objects.
`SelWindow` (`W`) and `SelCrossing` (`C`) force enclosed or crossing selection
for the next drag, regardless of drag direction. They also work during object
selection prompts; Esc cancels the capture. The commands follow Rhino's
[window and crossing selection](https://docs.mcneel.com/rhino/8/help/en-us/commands/selection_commands.htm#SelCrossing).
`SelRectangular` uses the drag direction by default. Enter
`SelectionMode=Window|Crossing|InvertWindow|InvertCrossing` with the command or
before dragging. Inverse Window
selects objects completely outside the rectangle; inverse Crossing also
selects objects that extend outside it. The
[Rhino command](https://docs.mcneel.com/rhino/8/help/en-us/commands/selection_commands.htm#SelRectangular)
also offers an occlusion option, which is not implemented yet.
`SelCircular` picks a center and radius point in one viewport, then selects
objects by their projected position in that circle. It accepts the same four
`SelectionMode` values before or after the command starts; Crossing is the
default. Esc cancels either point prompt. This follows Rhino's
[SelCircular](https://docs.mcneel.com/rhino/8/help/en-us/commands/selection_commands.htm#SelCircular)
view-plane selection.

Navigation ignores non-finite drag deltas, invalid zoom factors/pointers, and
non-finite or empty zoom rectangles. Pan updates that overflow screen coordinates
are rejected without partially changing zoom state. Regression tests cover all
four view kinds alongside normal pointer-pinned zoom and perspective dolly
behavior. Multi-frame egui event tests verify middle-drag, right-drag, and
Shift-right-drag across all four views: movement accumulates once per frame,
stationary frames do not move the camera, release does not emit Enter or select
geometry, and later pointer movement does not continue navigation. Camera
orientation stays unchanged during panning; construction planes stay unchanged
during all navigation.
`Zoom Extents` (or `ZE`) fits visible objects in the active viewport;
`Zoom Selected` (or `ZS`) fits only the visible selection. Add `All` before the
option, or use `ZEA`/`ZSA`, to fit all four viewports together. See
[zoom behavior and limits](commands/zoom.md).

`Zoom` or `Zoom Window` lets you drag a rectangle in any viewport to enlarge
that area; Ctrl/Cmd+W starts it, and Esc or right-click cancels it. The drag
does not consume an unfinished modeling prompt.
`Zoom Target` or `ZT` picks a new camera target, then a corner of the centered
zoom window. Both points can also be typed.
`UndoView` and `RedoView` move through camera changes in the active viewport;
Home and End do the same when a text field is not focused. View history is
separate from model undo and construction-plane undo.

The view-preset menu offers Top, Bottom, Front, Back, Right, Left, and
Perspective. `SetView World <direction>` resets the active camera to that
standard view. `SetView CPlane <direction>` uses the active construction plane
without changing its projection. See [view commands](commands/set-view.md).

`MeshToNURB` also supports [command-first object picking](object-selection.md).
During that prompt, clicks and selection windows add only selectable meshes;
Ctrl/Command removes picks, Enter finishes, and Escape cancels. `SelAll` is filtered
to meshes and `SelNone` clears picks. Accepted options survive cancellation.
`ToNURBS` also supports command-first picking, but Enter finishes selection and
opens a separate options phase. With preselection, options open immediately.
`MeshOptions` opens its triangle-trimming submenu; Enter returns to confirmation,
then another Enter converts. Escape from any phase cancels without remembering
the staged choices. Picks are fixed while confirming options.

Commands are case-insensitive. Typing while another non-text UI element or a
viewport is active moves the text to the command line automatically. Matching
command names appear below the input; Tab/Shift+Tab cycle fuzzy matches, and
Up/Down recall commands saved across sessions. File commands also complete paths.
See [command-line editing](command-line.md) for shortcuts and history storage.
Enter `Help UI` for [interface commands and shortcuts](commands/interface.md).
Display and snapping commands can run during a point or object prompt without losing its
accepted points, construction plane, selection, or undo history. Toolbar toggles
and shortcuts also preserve partially typed coordinates.
`CPlane` edits an independent plane in each viewport without changing its camera.
It also supports nested point prompts and separate plane undo/redo; see
[construction-plane editing](cplane.md).

At an active point prompt, type `x,y[,z]` or mix typed coordinates with viewport
picks. `w` selects world coordinates, `r`/`@` supplies a relative displacement,
and forms such as `5<30` supply polar coordinates. Typed values bypass snapping.
For example: enter `Line`, then `0`, then `r4,3`. Invalid coordinates keep the
prompt active and remain editable. See [typed point input](point-input.md) for
construction planes, supported syntax, and current limits.

Circle, Polygon, Rectangle, MeshPlane, Box, and MeshBox capture their construction
plane on the first accepted pick. `Box` now supports the first corner, opposite
base corner, and height-point workflow. See [construction-plane primitives](construction-planes.md).

Rotate, Mirror, Scale2D, Shear, and ProjectToCPlane also honor construction planes.
Scale2D takes its orientation from the finishing viewport; see
[plane transforms](plane-transforms.md) for the distinct reference rules.

With a curve or surface selected, `Curvature` starts a one-pick measurement.
`Curvature MarkCurvature=Yes` also adds permanent osculating markers; Esc cancels
without changing the document. Continuous hover analysis is not yet implemented.
See [curvature measurement](curvature.md) for reporting and marker behavior.
