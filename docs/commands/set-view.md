# SetView World and Plan

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

`SetView World` preserves the model, selection, model undo/redo, and any
unfinished modeling prompt. `SetView CPlane`, named views, two-point
perspective, and Rhino's configurable named-view projection/CPlane policy are
still pending. See [Rhino's SetView documentation](https://docs.mcneel.com/rhino/8/help/en-us/commands/setview.htm).

`Plan` changes the active viewport to a parallel view looking down the current
construction plane at its origin. Its camera keeps the plane axes captured at
the time of the command; later CPlane edits leave the camera alone. The plane
itself is unchanged. Projection, drawing, picking, snapping, Zoom Extents, and
UndoView/RedoView use this orientation. Rotated Plan views scan point-cloud
members for picking and snapping because their existing spatial indexes cover
only world XY, XZ, and YZ planes. The command preserves unfinished modeling
prompts and document undo/redo. A live Rhino camera comparison remains pending.
