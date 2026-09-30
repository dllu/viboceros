# SetView World, SetView CPlane, and Plan

[Interface commands](interface.md) · [Viewport controls](../interface.md)

`SetView World Top|Bottom|Front|Back|Right|Left|Perspective|TwoPointPerspective`
sets the active viewport to a standard world view. The six parallel views share
checked CPU/GPU projection, depth ordering, grid and point drafting, object picking, and indexed
point-cloud snaps. Bottom reverses Top's vertical axis; Back and Left reverse
the horizontal axes of Front and Right.

Parallel options restore the default camera target, pan, orientation, and zoom
and set the matching world construction plane. `World Perspective` preserves
the camera target and distance, uses a 50 mm lens and the standard world camera
direction, and keeps the current construction plane. View undo restores the
prior camera and projection; construction-plane history remains independent. Switching through
the viewport menu preserves the current camera target and zoom while changing
the view direction and construction plane.

`World TwoPointPerspective` levels the standard camera with World Z as its up
direction. It preserves the camera target and distance and keeps the perspective
field of view; conversion from a parallel view uses a 20 mm lens. It resets
the lens shift and sets the construction-plane axes to World Top while retaining
the plane origin. Right-drag horizontally rotates around the vertical axis.
Vertical right-drag changes camera height and shifts the frustum while keeping
the camera level. Shift+right-drag and middle-drag pan the view. Named views,
working 3DM views, and view history retain the projection and lens shift.
The [saved camera probe](../oracle.md#world-perspective-and-two-point-camera-probe)
checks both world perspective presets and six navigation drags. Document-based
near/far clipping recomputation and configurable navigation settings remain
pending; these comparisons check camera geometry and optical projection.

`SetView CPlane Top|Bottom|Front|Back|Right|Left` points the active camera along
one of the six standard directions of the current construction plane. It centers
the camera on the CPlane origin and resets pan. The construction plane and
viewport projection remain unchanged: a parallel viewport stays parallel and
keeps its drawing scale, while a perspective viewport keeps its field of view
and camera distance. The camera captures the plane orientation, so later CPlane
edits do not rotate it. View history restores the captured orientation, and
perspective orbit continues from it. The
[live camera probe](../oracle.md#setview-cplane-camera-probe) confirms
that Rhino also preserves parallel frustum width through all six directions.

Both SetView forms preserve the model, selection, model undo/redo, and any
unfinished modeling prompt. [Named views](named-view.md) save and restore
camera and CPlane state and persist them in 3DM files. Rhino's configurable
named-view projection/CPlane policy and bare `SetView` option prompts remain
pending. See
[Rhino's SetView documentation](https://docs.mcneel.com/rhino/8/help/en-us/commands/setview.htm).

`Plan` changes the active viewport to a parallel view looking down the current
construction plane at its origin. Its camera keeps the plane axes captured at
the time of the command; later CPlane edits leave the camera alone. The plane
itself is unchanged. Projection, drawing, picking, snapping, Zoom Extents, and
UndoView/RedoView use this orientation. Rotated Plan views use the cloud's shared
tree with lazy three-dimensional subtree bounds for picking and snapping. The
command preserves unfinished modeling prompts and document undo/redo. It keeps
the drawing scale when the source view is parallel, matching the
[live Plan probe](../oracle.md#plan-camera-probe). Rhino's perspective-to-parallel
Plan conversion preserves raw frustum width while changing scale at the former
camera target by the camera-distance/frustum-near ratio. The native view retains the frustum near
distance through 3DM import, named views, and view history and uses it for this
conversion. The six World parallel presets still need live camera comparisons.

`NextViewport` and `PrevViewport` cycle through the four viewports, wrapping at
the ends. Ctrl/Cmd+Tab and Ctrl/Cmd+Shift+Tab run them without moving focus out
of the command field. `NextOrthoViewport` skips perspective views, while
`NextPerspectiveViewport` skips orthographic views. When no matching viewport
exists, the active view stays put. These commands change only the active
viewport; cameras, modeling prompts, selection, and model history stay intact.
See [Rhino's viewport navigation commands](https://docs.mcneel.com/rhino/8/help/en-us/commands/nextviewport.htm).
