# Interface and drafting

[Project overview](../README.md) · [Command reference](commands/README.md)

The application opens with Top, Perspective, Front, and Right viewports.
Each supports wireframe, shaded, and ghosted display, with independent navigation.
The layer pane creates, renames, recolors, shows, locks, activates, and deletes
empty layers; it reports object counts and combines edits into one undo step.

Osnap captures visible Point, End, Mid, Center, and Quad features, including
indexed members of point clouds and features on locked objects and layers;
SmartTrack captures horizontal and vertical alignment from the first picked
point. Grid Snap rounds construction-plane picks to the unit grid. Right-drag
pans parallel views and rotates the Perspective view; Shift-right-drag pans the
Perspective view, middle-drag pans any view, and the mouse wheel zooms. A plain
right-click acts as Enter. Outside a drafting command, left-drag from left to
right selects only fully enclosed objects, while right-to-left makes a crossing
selection. Click geometry to replace the selection, Shift-click/drag to add,
and Ctrl-click/drag or Command-click/drag to remove. Click empty space or press
Esc to clear the selection; press Delete to remove selected objects.

Commands are case-insensitive. Typing while another non-text UI element or a
viewport is active moves the text to the command line automatically. Matching
command names appear below the input; press Tab or click a match to complete it.

At an active point prompt, type `x,y[,z]` or mix typed coordinates with viewport
picks. `w` selects world coordinates, `r`/`@` supplies a relative displacement,
and forms such as `5<30` supply polar coordinates. Typed values bypass snapping.
For example: enter `Line`, then `0`, then `r4,3`. Invalid coordinates keep the
prompt active and remain editable. See [typed point input](point-input.md) for
construction planes, supported syntax, and current limits.

Circle, Polygon, Rectangle, MeshPlane, Box, and MeshBox capture their construction
plane on the first accepted pick. `Box` now supports the first corner, opposite
base corner, and height-point workflow. See [construction-plane primitives](construction-planes.md).

With a curve or surface selected, `Curvature` starts a one-pick measurement.
`Curvature MarkCurvature=Yes` also adds permanent osculating markers; Esc cancels
without changing the document. Continuous hover analysis is not yet implemented.
See [curvature measurement](curvature.md) for reporting and marker behavior.
