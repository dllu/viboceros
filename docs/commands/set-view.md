# SetView World, SetView CPlane, and Plan

[Interface commands](interface.md) · [Viewport controls](../interface.md)

`SetView World Top|Bottom|Front|Back|Right|Left|Perspective` resets the active
viewport to a standard world view. The six parallel views share checked CPU/GPU
projection, depth ordering, grid and point drafting, object picking, and indexed
point-cloud snaps. Bottom reverses Top's vertical axis; Back and Left reverse
the horizontal axes of Front and Right. Perspective retains the existing 35°
vertical field of view.

The command restores the default camera target, pan, orientation, and zoom.
Parallel options also set the matching world construction plane. Perspective
keeps the current construction plane. View undo restores the prior camera and
projection; construction-plane history remains independent. Switching through
the viewport menu preserves the current camera target and zoom while changing
the view direction and construction plane.

`SetView CPlane Top|Bottom|Front|Back|Right|Left` points the active camera along
one of the six standard directions of the current construction plane. It centers
the camera on the CPlane origin and resets pan and zoom. The construction plane
and viewport projection remain unchanged: a parallel viewport stays parallel,
and a perspective viewport keeps its field of view. The camera captures the
plane orientation, so later CPlane edits do not rotate it. View history restores
the captured orientation, and perspective orbit continues from it.

Both SetView forms preserve the model, selection, model undo/redo, and any
unfinished modeling prompt. Named views, two-point perspective, and Rhino's
configurable named-view projection/CPlane policy remain pending. See
[Rhino's SetView documentation](https://docs.mcneel.com/rhino/8/help/en-us/commands/setview.htm).

`Plan` changes the active viewport to a parallel view looking down the current
construction plane at its origin. Its camera keeps the plane axes captured at
the time of the command; later CPlane edits leave the camera alone. The plane
itself is unchanged. Projection, drawing, picking, snapping, Zoom Extents, and
UndoView/RedoView use this orientation. Rotated Plan views use the cloud's shared
tree with lazy three-dimensional subtree bounds for picking and snapping. The
command preserves unfinished modeling prompts and document undo/redo. A live
Rhino camera comparison for these commands remains pending.
The [camera oracle fixture](../oracle.md#setview-cplane-camera-probe) is ready to
record the corresponding public Rhino viewport properties when Rhino starts.

`NextViewport` and `PrevViewport` cycle through the four viewports, wrapping at
the ends. Ctrl/Cmd+Tab and Ctrl/Cmd+Shift+Tab run them without moving focus out
of the command field. `NextOrthoViewport` skips perspective views, while
`NextPerspectiveViewport` skips orthographic views. When no matching viewport
exists, the active view stays put. These commands change only the active
viewport; cameras, modeling prompts, selection, and model history stay intact.
See [Rhino's viewport navigation commands](https://docs.mcneel.com/rhino/8/help/en-us/commands/nextviewport.htm).
