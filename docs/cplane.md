# Construction-plane editing

[Interface](interface.md) · [Typed points](point-input.md) · [Primitive orientation](construction-planes.md)

Every viewport has an independent right-handed construction plane, separate
from its camera. `CPlane` changes its origin/orientation without moving the
camera, document objects, selection, or model undo history. The grid, free
mouse picks, Grid Snap, SmartTrack axes, typed coordinates, plane-aware
primitives, and plane-aware transforms use that frame.

| Command | Result |
| --- | --- |
| `CPlane point` | Move the origin, retaining all axes. |
| `CPlane All point` | Move every viewport's plane origin to the same world point, retaining each viewport's axes. |
| `CPlane View` | Put the active plane at the camera target, with X and Y along screen right and up. |
| `CPlane World Top\|Bottom\|Front\|Back\|Right\|Left` | Restore a world preset at the world origin. |
| `CPlane 3Point origin x-point y-point` | Set X toward the second point; the third determines the positive XY half-plane. |
| `CPlane 3Point origin Vertical x-point` | Project the X direction onto the old plane, then use the old normal as the new Y axis. |
| `CPlane 3Point origin ZAxis z-point` | Set the new normal toward the second point, with deterministic OpenNURBS axes. |
| `CPlane Elevation distance` | Move along the plane's normal by a signed distance. |
| `CPlane Through point` | Move only along the normal until the plane passes through the point. |
| `CPlane Through All point` | Move every viewport's plane along its own normal until it passes through the point. |
| `CPlane Rotate axis-start axis-end degrees` | Rotate the origin and axes about a world-space axis. |
| `CPlane Rotate axis-start axis-end reference-point target-point` | Rotate by the signed angle between two directions from the axis. |
| `CPlane Object [object-id [Face=index]]` | Align the active plane to a supported object, mesh face, or B-rep face. |
| `CPlane Surface [object-id [Face=index] [origin [x-point]]]` | Place a tangent plane on a surface or B-rep face, then choose its X direction. |
| `CPlane Undo` / `CPlane Redo` | Navigate viewport-local plane history. |
| `NamedCPlane Save name` / `Restore name` | Save or restore a reusable construction plane with grid and snap spacing. |
| `NamedCPlane Import path.3dm` | Import named construction planes without importing model objects. |
| `CopyCPlaneToAll [source]` | Copy a viewport's construction plane to every other viewport. |
| `CopyCPlaneSettingsToAll [source]` | Copy its grid display and snap settings to every other viewport. |
| `SynchronizeCPlanes [source] [SetView=Yes\|No]` | Orient standard Top, Front, Right, and Perspective construction planes to a source plane. |

For the copy commands, `source` is a viewport name or its one-based number;
omitting it uses the active viewport. The plane command records each target's
plane history. Both commands leave camera views and model undo unchanged.

`SynchronizeCPlanes` uses the active viewport unless `source` names a viewport
or gives its one-based number. Perspective receives the source plane; Front and
Right receive its corresponding 90-degree rotations. Top receives the source
plane for an oblique source. When a standard parallel viewport already has an
exact world-preset plane, Rhino uses that preset as the viewport's rotation
role; this also applies to Top. Renamed user views are left alone.
`SetView=No` updates only construction planes; `SetView=Yes` also points the
parallel cameras along those planes. Perspective's camera stays in place. The
default is `SetView=Yes`.

`CPlane View` reads the current viewport camera, including its effective
parallel-view pan target, and changes only that viewport's construction plane.
It preserves camera position, direction, and projection. At the bare `CPlane`
origin prompt, type `View` to apply the same option.

Points accept the regular local/world/relative coordinate syntax; prefix `w`
for world coordinates. Angles are degrees. For example:

```text
CPlane 3Point w1,2,3 w2,4,6 w-3,10,4
Circle
0
2,0
CPlane Elevation 5
CPlane Undo
```

Bare `CPlane` prompts for a new origin. `CPlane 3Point`, `CPlane Through`,
`CPlane Elevation`, and `CPlane Rotate` also have interactive prompts, mixing
typed coordinates and viewport picks. Elevation accepts a distance or height
point; Rotate accepts two axis points followed by a typed angle or two picked
reference points. Their directions are projected perpendicular to the axis.
Enter at the origin prompt retains the starting plane origin. Degenerate axes, collinear
three-point definitions, nonfinite values, and unrepresentable results remain
correctable errors; they do not partially change the plane.

At the 3Point X-axis prompt, `Vertical` changes the next pick to a projected
X direction and completes the plane. `ZAxis` changes it to a new normal and
also completes after that pick. The old CPlane's axes affect Vertical;
ZAxis uses the normal-only OpenNURBS frame rule.

`CPlane Object` uses a single preselected object, or prompts for a viewport
object pick or typed UUID. An explicit UUID also works in the full command.
For a mesh or multi-face polysurface, use `CPlane Object object-id Face=index`
or pick a face in a viewport; face indices are zero based. A single preselected
mesh or multi-face polysurface waits for a face pick.
At the bare `CPlane` origin prompt, type `Object` to enter the same selection
prompt.
Circles and arcs place the origin at their center with X toward the curve start.
Ellipses put the origin at their start point, with X along the start tangent.
Lines use their start point and an OpenNURBS-style supporting plane: a line
parallel to a world coordinate plane uses that plane, while an oblique line
uses its direction for X. Polylines and NURBS curves use their start point and
starting tangent for X. Planar curves derive Z from their oriented control
plane; nonplanar curves use the start curvature direction when available, with
a deterministic perpendicular when the start curvature vanishes.
Joined polycurves use the same start tangent and sample their full path to
find a supporting plane; nonplanar joins use their start frame.
NURBS surfaces and B-rep faces use the midpoint of the underlying U/V domains,
the U tangent for X, and the surface normal for Z. Single-face B-reps align
directly; multi-face polysurfaces require a face index or pick.
Mesh faces use the average of their three or four vertices for the origin,
their polygon normal for Z, and deterministic normal-derived X and Y axes.
The command does not change object selection or the camera.

`CPlane Surface` accepts a surface or a picked B-rep face. A typed UUID may be
followed by a zero-based `Face=index` for a polysurface. At the next prompt,
pick a point to place the origin at its closest point on the underlying surface,
or press Enter for the untrimmed UV midpoint. Pick an X direction in world
space, or press Enter for the U tangent. The X point is projected into the
surface tangent plane. The full command also accepts typed origin and X points,
for example `CPlane Surface object-id Face=2 w1,2,3 w2,2,3`.

`CPlane All` and `CPlane Through All` also accept a picked point. Typed points
use the active viewport's CPlane for local coordinates; the resolved world
point is then applied to every viewport. Each viewport records its own plane
history. Through All computes every new plane before applying any of them, so
an invalid result leaves all planes unchanged. Explicit `All=Yes` and `All=No`
forms are accepted in command macros. Origin and Through remember their own
`All` setting for the current session. A bare `All` toggles its setting;
`All=Yes` and `All=No` select it explicitly. The setting survives a canceled
point prompt.

Plane prompts are separate from model prompts: start `Polyline`, accept two
points, run `CPlane 3Point`, define the frame, then continue the same polyline.
Its earlier points and relative-input base remain intact; subsequent local
coordinates use the new plane. Escape cancels only the innermost CPlane prompt.
The plane being edited is the viewport where the CPlane prompt started, even
when reference picks come from other viewports. Previously latched primitive
and transform construction frames retain their documented rules.
The same nesting applies to live `Points` placement: Enter is handled by the
innermost plane prompt, and completing or cancelling that prompt leaves the
point session active. Plane changes do not join its document transaction.

Plane history retains up to 50 entries per viewport. Successful no-op edits
also create history entries, as observed in Rhino. Editing after undo clears
the plane redo branch; model Undo/Redo remains separate. Shift+Home and
Shift+End navigate plane history when a text editor is not focused; in text
editors these keys retain their text-selection behavior. The view-preset menu
explicitly resets the chosen viewport's plane, including when reselecting the
same preset. Camera orbit, pan, zoom, and display-mode changes do not reset it.

`NamedCPlane List`, `Update`, `Delete`, `Rename old | new`, `Duplicate source |
new`, `MoveUp`, and `MoveDown` manage the ordered named-plane list. Restore
changes the active viewport's CPlane through its plane history and applies the
saved grid line count, minor spacing, snap spacing, and thick-line interval.
Set the thick-line interval to zero to show only thin grid lines.
Show-grid and axis toggles remain viewport settings. Save, Export3dm, Open3dm,
and Import3dm persist the OpenNURBS named-CPlane table; `NamedCPlane Import`
reads that table only, converts lengths to the current document units, and
resolves duplicate names with numbered suffixes. The 3DM depth-buffer flag is
retained in the named-plane table but is not exposed as a viewport control.

Grid X/Y axes are red/green in local plane coordinates. Grid snapping retains
the picked elevation, and tracking uses the plane axes through the first
reference point with a screen-space capture radius. An edge-on construction
plane has no free ray/plane pick; camera-space object snaps still work. This
is reference-axis tracking, not Rhino's complete SmartTrack inference system.
Dashed guides are clipped before tessellation so distant reference points do
not allocate off-screen dashes.

## Validation and remaining scope

The command/parser/history implementation is independent of egui and model
transactions. The drafting module owns ray/plane intersection, plane-local grid
rounding, and projected tracking. Tests check invalid input, nested prompts,
history isolation, camera/GPU invariance, all four camera types, off-plane
anchors, edge-on object snapping, and arbitrary orthonormal frames.

`construction_planes.json` compares 129 plane transitions with actual Rhino
commands, including every world preset, oblique/translated frames, duplicate
edits, and history branching. All agree at absolute/relative `1e-8`/`1e-12`;
the maximum coordinate difference is `1.86e-9` on a frame translated by `1e7`.
`construction_plane_input.json` compares 24 actual nested Polyline/CPlane
workflows, including local and relative inputs; the maximum difference is
`1.42e-14`. These probes are untimed. The worker restores viewport planes,
application settings, and any owned geometry/selection on ordinary exits;
forced process termination cannot run cleanup.

A release-build UI smoke test also exports and checks seven objects at `1e-10`:
shifted/oblique mouse-grid picks, nested relative Polyline input, plane-aligned
Circle/Rectangle/Box geometry, and a rotated plane origin. Wireframe, Shaded,
and Ghosted retain the edited plane without changing the other viewports.

This is not the complete [Rhino CPlane command](https://docs.mcneel.com/rhino/8/help/en-us/commands/cplane.htm):
Curve/Gumball options, Surface Flip and IgnoreTrims options, trim-aware Surface
origin picks, universal/automatic planes, the named-plane panel, and
CopyCPlane commands with a picked source viewport remain
unimplemented. General scalar point-input
constraints and converting every remaining modeling command to construction
planes are separate ongoing work. The existing view menu is not full `SetView`.
The typed `Through All` macro has a documented/native discrepancy described in the
[oracle notes](oracle.md), so its exact Rhino parity remains unverified.
