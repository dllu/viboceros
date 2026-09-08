# Interface and drafting

[Project overview](../README.md) · [Command reference](commands/README.md)

The application opens with Top, Perspective, Front, and Right viewports.
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
visibility parity, live Rhino comparisons, and large-coordinate GPU precision
remain unverified.
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

Osnap captures visible Point, End, Mid, Center, and Quad features, including
indexed members of point clouds and features on locked objects and layers;
SmartTrack captures local plane-axis alignment from the first picked
point in every viewport. Grid Snap rounds construction-plane picks to the unit grid. Right-drag
pans parallel views and rotates the Perspective view; Shift-right-drag pans the
Perspective view, middle-drag pans any view, and the mouse wheel zooms. A plain
right-click acts as Enter. Outside a drafting command, left-drag from left to
right selects only fully enclosed objects, while right-to-left makes a crossing
selection. Click geometry to replace the selection, Shift-click/drag to add,
and Ctrl-click/drag or Command-click/drag to remove. Click empty space or press
Esc to clear the selection; press Delete to remove selected objects.

Navigation ignores non-finite drag deltas, invalid zoom factors/pointers, and
non-finite or empty zoom rectangles. Pan updates that overflow screen coordinates
are rejected without partially changing zoom state. Regression tests cover all
four view kinds alongside normal pointer-pinned zoom and perspective dolly
behavior. Multi-frame egui event tests verify middle-drag, right-drag, and
Shift-right-drag across all four views: movement accumulates once per frame,
stationary frames do not move the camera, release does not emit Enter or select
geometry, and later pointer movement does not continue navigation. Camera
targets and construction planes stay unchanged during these drags.
`Zoom Extents` (or `ZE`) fits visible objects in the active viewport;
`Zoom Selected` (or `ZS`) fits only the visible selection. Add `All` before the
option, or use `ZEA`/`ZSA`, to fit all four viewports together. See
[zoom behavior and limits](commands/zoom.md).

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
command names appear below the input; press Tab or click a match to complete it.
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
