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
| `CPlane World Top\|Bottom\|Front\|Back\|Right\|Left` | Restore a world preset at the world origin. |
| `CPlane 3Point origin x-point y-point` | Set X toward the second point; the third determines the positive XY half-plane. |
| `CPlane Elevation distance` | Move along the plane's normal by a signed distance. |
| `CPlane Through point` | Move only along the normal until the plane passes through the point. |
| `CPlane Rotate axis-start axis-end degrees` | Rotate the origin and axes about a world-space axis. |
| `CPlane Undo` / `CPlane Redo` | Navigate viewport-local plane history. |

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
point; Rotate accepts two axis points followed by a typed angle. Enter at the
origin prompt retains the starting plane origin. Degenerate axes, collinear
three-point definitions, nonfinite values, and unrepresentable results remain
correctable errors; they do not partially change the plane.

Plane prompts are separate from model prompts: start `Polyline`, accept two
points, run `CPlane 3Point`, define the frame, then continue the same polyline.
Its earlier points and relative-input base remain intact; subsequent local
coordinates use the new plane. Escape cancels only the innermost CPlane prompt.
The plane being edited is the viewport where the CPlane prompt started, even
when reference picks come from other viewports. Previously latched primitive
and transform construction frames retain their documented rules.

Plane history retains up to 50 entries per viewport. Successful no-op edits
also create history entries, as observed in Rhino. Editing after undo clears
the plane redo branch; model Undo/Redo remains separate. Shift+Home and
Shift+End navigate plane history when a text editor is not focused; in text
editors these keys retain their text-selection behavior. The view-preset menu
explicitly resets the chosen viewport's plane, including when reselecting the
same preset. Camera orbit, pan, zoom, and display-mode changes do not reset it.

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
All/View/Object/Surface/Curve/Gumball options, 3Point Vertical/ZAxis, picked-angle
rotation, universal/automatic planes, named planes, CopyCPlane commands, and
saved viewport settings in 3DM remain unimplemented. General scalar point-input
constraints and converting every remaining modeling command to construction
planes are separate ongoing work. The existing view menu is not full `SetView`.
