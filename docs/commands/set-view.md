# SetView World, SetView CPlane, and Plan

[Interface commands](interface.md) · [Viewport controls](../interface.md)

`SetView` prompts for `CPlane` or `World`, then a view direction. Type a choice
or click it in the command bar. `SetView World` and `SetView CPlane` start at
the direction prompt. Enter or Esc cancels either stage; neither has a default
or remembers the previous choice. Invalid choices keep the prompt open.
The CPlane stage offers six parallel directions; only the World stage offers
Perspective and TwoPointPerspective.

The view prompt suspends unfinished modeling input and any prior CPlane or
zoom prompt. It accepts transparent view commands: for example, `CPlane`
inside the World stage opens a construction-plane prompt, then returns to the
World choices when that prompt finishes or is canceled. Canceling SetView
resumes the suspended command and preserves its one-pick snap setting.
The [saved option probe](../oracle.md#setview-option-prompt-probe) checks
33 transitions, including cancellation, rejected choices, and nested commands.

`SetView World Top|Bottom|Front|Back|Right|Left|Perspective|TwoPointPerspective`
sets the active viewport to a standard world view. The six parallel views share
checked CPU/GPU projection, depth ordering, grid and point drafting, object picking, and indexed
point-cloud snaps. Bottom reverses Top's vertical axis; Back and Left reverse
the horizontal axes of Front and Right.

Parallel options preserve the camera target and drawing scale and set the
matching world construction-plane axes while retaining its origin. A panned
parallel view keeps its current center. Conversion from perspective uses the
frustum at the camera target depth, clamped to the existing near/far interval,
and clears any lens shift. The
[World parallel camera probe](../oracle.md#world-parallel-camera-probe)
checks all six presets from parallel, perspective, and two-point inputs at
different zooms and with shifted frusta. `World Perspective` preserves
the camera target and distance, uses a 50 mm lens and the standard world camera
direction, and keeps the current construction plane. View undo restores the
prior camera and projection; construction-plane history remains independent.
The viewport preset menu uses the same World transitions and settings.

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
and axial camera distance. A two-point source becomes ordinary perspective, and
lens shift is cleared. The camera captures the plane orientation, so later CPlane
edits do not rotate it. View history restores the captured orientation, and
perspective orbit continues from it. The
[live camera probe](../oracle.md#setview-cplane-camera-probe) confirms
that Rhino also preserves parallel frustum width through all six directions.
The [two-point and shifted camera probe](../oracle.md#cplane-two-point-and-shifted-camera-probe)
checks six directions, Plan, and CPlane View from two-point and shifted parallel
inputs. Imported off-axis camera targets and frustum centers remain independent;
CPlane View places the plane at the saved target without changing optical framing.

Both SetView forms preserve the model, selection, model undo/redo, and any
unfinished modeling prompt. [Named views](named-view.md) save and restore
camera and CPlane state and persist them in 3DM files. See
[Rhino's SetView documentation](https://docs.mcneel.com/rhino/8/help/en-us/commands/setview.htm).

### View settings

The **View options** menu has two persistent application settings, enabled by
default, matching [Rhino's View options](https://docs.mcneel.com/rhino/8/help/en-us/options/view.htm):

- **Named views set CPlane** controls CPlane restoration and World preset axes.
  When disabled, World presets keep the current CPlane. World Perspective keeps
  it with either setting.
- **Named views set projection** controls saved projection restoration and World
  preset conversion. When disabled, parallel/perspective projection is retained.
  World Top/Bottom/Front/Back/Right/Left still change direction; a two-point source
  becomes ordinary perspective. World Perspective can produce an oblique
  parallel view. World TwoPointPerspective always enables two-point projection.

These settings do not enter model undo history and do not affect SetView CPlane,
Plan, or restoration of working viewports when opening a file. The
[policy probes](../oracle.md#named-view-and-world-preset-policy-probes) check 96
World transitions and 72 named-view restores against private-Xvfb Rhino captures.

`Plan` changes the active viewport to a parallel view looking down the current
construction plane at its origin. Its camera keeps the plane axes captured at
the time of the command; later CPlane edits leave the camera alone. The plane
itself is unchanged. Projection, drawing, picking, snapping, Zoom Extents, and
UndoView/RedoView use this orientation. Rotated Plan views use the cloud's shared
tree with lazy three-dimensional subtree bounds for picking and snapping. The
command preserves unfinished modeling prompts and document undo/redo. It keeps
the drawing scale when the source view is parallel, matching the
[live Plan probe](../oracle.md#plan-camera-probe). Rhino's perspective-to-parallel
Plan conversion projects the former target plane when its axial distance is
beyond the near plane. Otherwise it retains the raw near-plane width. It clears
two-point locks and frustum shift and centers on the CPlane origin. The native
view retains the near distance through 3DM import, named views, and view history
for this conversion. Unrepresentable parallel scales return an error without
changing the camera or histories. Document-based near/far clipping and the resulting parallel camera
relocation along the depth axis remain pending. The World parallel probe checks
framing, orientation, target, and construction-plane state independently of that
camera relocation.

`NextViewport` and `PrevViewport` cycle through the four viewports, wrapping at
the ends. Ctrl/Cmd+Tab and Ctrl/Cmd+Shift+Tab run them without moving focus out
of the command field. `NextOrthoViewport` skips perspective views, while
`NextPerspectiveViewport` skips orthographic views. When no matching viewport
exists, the active view stays put. These commands change only the active
viewport; cameras, modeling prompts, selection, and model history stay intact.
See [Rhino's viewport navigation commands](https://docs.mcneel.com/rhino/8/help/en-us/commands/nextviewport.htm).
