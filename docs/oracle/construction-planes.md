# Construction-plane comparisons

[Oracle overview](../oracle.md) · [Evidence index](README.md)

The [CPlane All fixture](../../tools/rhino_oracle/fixtures/construction_plane_all.json)
records standard and oblique starting planes with four independent viewports.
Its [private-Xvfb Rhino 8.32 observation](../../tools/rhino_oracle/observations/construction_plane_all.json)
shows that `All=Yes` moves every origin to the picked world point while leaving
all plane axes and camera targets unchanged. Regenerate it with:

```sh
tools/rhino_oracle/run_headless.sh rhino tools/rhino_oracle/fixtures/construction_plane_all.json --timeout 180
```

The [CPlane View fixture](../../tools/rhino_oracle/fixtures/construction_plane_view.json)
uses the public viewport API to place an oblique CPlane in Top and Perspective
views, then runs `_CPlane _View`. Its [Rhino 8.32 observation](../../tools/rhino_oracle/observations/construction_plane_view.json)
shows that the new plane origin is the camera target, and X/Y match camera
right/up even when the old CPlane had a different origin and axes. The camera
target and direction remain unchanged. Both zero and offset targets were
captured in a private Xvfb session. Regenerate with:

```sh
tools/rhino_oracle/run_headless.sh rhino tools/rhino_oracle/fixtures/construction_plane_view.json --timeout 300
```

Compare all four recorded states with:

```sh
python3 -m tools.rhino_oracle.view_camera_probe \
  tools/rhino_oracle/fixtures/construction_plane_view.json \
  tools/rhino_oracle/observations/construction_plane_view.json
```

The largest component difference from the camera alignment rule is `5.56e-17`;
recorded camera changes are zero.

The [3Point option fixture](../../tools/rhino_oracle/fixtures/construction_plane_three_point_options.json)
records five Rhino 8.32 transitions for `Vertical` and `ZAxis`, starting from
world and oblique CPlanes. Its [private-Xvfb observation](../../tools/rhino_oracle/observations/construction_plane_three_point_options.json)
shows that each option is chosen after the 3Point origin and completes after
one direction point. Vertical projects X into the old plane and uses the old
normal for Y; ZAxis uses the OpenNURBS normal-frame rule. Replay with:

```sh
python3 -m tools.rhino_oracle replay \
  tools/rhino_oracle/fixtures/construction_plane_three_point_options.json \
  --observations tools/rhino_oracle/observations/construction_plane_three_point_options.json \
  --absolute-epsilon 1e-10 --relative-epsilon 1e-12
```

All five transitions replay, with a largest axis-component difference of
`2.22e-16`. A Rust oracle test also checks the saved frames at `1e-10`.

The [picked Rotate fixture](../../tools/rhino_oracle/fixtures/construction_plane_rotate_points.json)
records four Rhino 8.32 transitions: positive and negative quarter turns,
an offset rotation axis, and an oblique axis with an oblique starting plane.
Its [private-Xvfb observation](../../tools/rhino_oracle/observations/construction_plane_rotate_points.json)
shows that Rhino uses the signed angle between reference directions projected
perpendicular to the rotation axis. Regenerate it with:

```sh
tools/rhino_oracle/run_headless.sh rhino tools/rhino_oracle/fixtures/construction_plane_rotate_points.json --timeout 180
```

Replay the saved frames with:

```sh
python3 -m tools.rhino_oracle replay \
  tools/rhino_oracle/fixtures/construction_plane_rotate_points.json \
  --observations tools/rhino_oracle/observations/construction_plane_rotate_points.json \
  --absolute-epsilon 1e-10 --relative-epsilon 1e-12
```

All four transitions replay, with a largest component difference of `8.88e-16`.
A Rust oracle test checks the same frames at `1e-10`.

The CPlane Object fixtures for
[circles](../../tools/rhino_oracle/fixtures/construction_plane_object_circle.json),
[arcs](../../tools/rhino_oracle/fixtures/construction_plane_object_arc.json),
[ellipses](../../tools/rhino_oracle/fixtures/construction_plane_object_ellipse.json), and
[surfaces](../../tools/rhino_oracle/fixtures/construction_plane_object_surface.json)
record ten actual Rhino 8.32 `_CPlane _Object` transitions. Their saved
observations are beside the fixtures in `tools/rhino_oracle/observations/`.
All live commands ran in private Xvfb. Circle and arc origins are at the center;
ellipse origins are at the curve start, with X tangent there. A surface uses its
untrimmed UV midpoint and U tangent, including on a warped bilinear patch.
Native replay matches all ten transitions at `1e-10` absolute and `1e-12`
relative tolerance; the largest component difference is `1.33e-15`.
The following replays circles; substitute `arc`, `ellipse`, or `surface` in both
filenames to replay the other cases:

```sh
python3 -m tools.rhino_oracle replay \
  tools/rhino_oracle/fixtures/construction_plane_object_circle.json \
  --observations tools/rhino_oracle/observations/construction_plane_object_circle.json \
  --absolute-epsilon 1e-10 --relative-epsilon 1e-12
```

The [mesh face fixture](../../tools/rhino_oracle/fixtures/construction_plane_object_mesh_face.json)
adds four private-Xvfb captures: a triangle, a tilted quad, an indexed second
face, and fractional vertex coordinates. Its
[Rhino observation](../../tools/rhino_oracle/observations/construction_plane_object_mesh_face.json)
places the CPlane at the face vertex average, with Z along the face normal and
X from the normal-frame rule. The tilted-quad axes match a normal whose
components were rounded to 32-bit floats before frame construction; the native
command reproduces that rounding. Regenerate and replay with:

```sh
tools/rhino_oracle/run_headless.sh rhino tools/rhino_oracle/fixtures/construction_plane_object_mesh_face.json --timeout 180
python3 -m tools.rhino_oracle replay \
  tools/rhino_oracle/fixtures/construction_plane_object_mesh_face.json \
  --observations tools/rhino_oracle/observations/construction_plane_object_mesh_face.json \
  --absolute-epsilon 1e-10 --relative-epsilon 1e-12
```

All four mesh-face captures replay at these tolerances; the largest component
difference is `4.44e-16`.

The [B-rep face fixture](../../tools/rhino_oracle/fixtures/construction_plane_object_brep_face.json)
captures `_CPlane _Object` for all six faces of a box created by Rhino, with one
case starting from an offset Front CPlane. Its
[saved observations](../../tools/rhino_oracle/observations/construction_plane_object_brep_face.json)
were recorded in private Xvfb. The Rust probe maps Rhino's box face numbering
to the native box builder's face numbering, then compares each face's untrimmed
UV midpoint and axes. Regenerate or replay with:

```sh
tools/rhino_oracle/run_headless.sh rhino tools/rhino_oracle/fixtures/construction_plane_object_brep_face.json --timeout 180
python3 -m tools.rhino_oracle replay \
  tools/rhino_oracle/fixtures/construction_plane_object_brep_face.json \
  --observations tools/rhino_oracle/observations/construction_plane_object_brep_face.json \
  --absolute-epsilon 1e-10 --relative-epsilon 1e-12
```

All six B-rep face cases replay, with a largest axis component difference of
`1.11e-16`.

The [CPlane Surface fixture](../../tools/rhino_oracle/fixtures/construction_plane_surface.json)
records six private-Xvfb Rhino 8.32 transitions on planar and warped surfaces.
It covers the default UV midpoint/U direction, chosen origin and X points,
and world points outside the tangent plane. Its
[saved observation](../../tools/rhino_oracle/observations/construction_plane_surface.json)
is compared with the native closest-point and tangent-frame calculation:

```sh
tools/rhino_oracle/run_headless.sh rhino tools/rhino_oracle/fixtures/construction_plane_surface.json --timeout 180
python3 -m tools.rhino_oracle replay \
  tools/rhino_oracle/fixtures/construction_plane_surface.json \
  --observations tools/rhino_oracle/observations/construction_plane_surface.json \
  --absolute-epsilon 1e-10 --relative-epsilon 1e-12
```

All six Surface cases replay; the largest component difference is `4.44e-16`.

The [Surface option fixture](../../tools/rhino_oracle/fixtures/construction_plane_surface_options.json)
adds ten private-Xvfb cases for `Flip=Yes|No`, with default and picked
origins, chosen X directions, and a warped surface. Its
[saved observation](../../tools/rhino_oracle/observations/construction_plane_surface_options.json)
shows that Rhino applies Flip after a picked origin; accepting the default UV
midpoint leaves the natural orientation even when an X direction is chosen.
Replay with:

```sh
python3 -m tools.rhino_oracle replay \
  tools/rhino_oracle/fixtures/construction_plane_surface_options.json \
  --observations tools/rhino_oracle/observations/construction_plane_surface_options.json \
  --absolute-epsilon 1e-10 --relative-epsilon 1e-12
```

All ten cases replay with a largest component difference of `4.44e-16`.
The IgnoreTrims case uses an untrimmed surface and establishes the option's
no-op behavior there.

The [trimmed Surface fixture](../../tools/rhino_oracle/fixtures/construction_plane_surface_trimmed.json)
records six private-Xvfb Rhino 8.32 cases on a planar face with a square hole.
Picked origins inside the hole snap to the nearest hole boundary; `IgnoreTrims`
uses the underlying surface. Picks outside the face clamp to its outer edge
even with `IgnoreTrims`, because the supporting surface is bounded. Enter at
the origin prompt uses the untrimmed UV midpoint. The
[saved observation](../../tools/rhino_oracle/observations/construction_plane_surface_trimmed.json)
is replayed with:

```sh
python3 -m tools.rhino_oracle replay \
  tools/rhino_oracle/fixtures/construction_plane_surface_trimmed.json \
  --observations tools/rhino_oracle/observations/construction_plane_surface_trimmed.json \
  --absolute-epsilon 1e-10 --relative-epsilon 1e-12
```

All six trimmed Surface cases replay; the largest component difference is
`1.42e-14`.

The [Curve CPlane fixture](../../tools/rhino_oracle/fixtures/construction_plane_curve.json)
records 18 private-Xvfb Rhino 8.32 cases: horizontal, vertical, oblique, and
reversed lines; off-curve and endpoint picks; a different initial CPlane; the
default start station; a polyline segment and corner; horizontal and vertical
circle stations; and planar and spatial quadratic NURBS stations. The
[saved observation](../../tools/rhino_oracle/observations/construction_plane_curve.json)
is replayed with:

```sh
python3 -m tools.rhino_oracle replay \
  tools/rhino_oracle/fixtures/construction_plane_curve.json \
  --observations tools/rhino_oracle/observations/construction_plane_curve.json \
  --absolute-epsilon 1e-10 --relative-epsilon 1e-12
```

All 18 Curve cases replay; the largest component difference is `3.06e-16`.

The [CPlane copy-pick fixture](../../tools/rhino_oracle/fixtures/copy_cplane_pick.json)
records private-Xvfb clicks in the Top viewport for `CopyCPlaneToAll` and
`CopyCPlaneSettingsToAll`, with the Perspective viewport initially active.
The [saved observation](../../tools/rhino_oracle/observations/copy_cplane_pick.json)
shows that the plane command copies the frame, numeric grid settings, grid
visibility, and construction-axis visibility while preserving each target's
world-axis icon setting. The settings command copies all grid settings and
leaves target frames intact. Both make the clicked source viewport active.
The plane command's `CPlane Undo` restores the selected target's old frame and
numeric grid settings, one viewport at a time, while its visibility settings
stay copied. An app regression checks the saved viewport states and cameras.

The [curve Object fixture](../../tools/rhino_oracle/fixtures/construction_plane_object_curve.json)
records 15 private-Xvfb cases for lines, open and closed polylines, and
polynomial or rational NURBS curves. Linear curves use an axis-aligned
supporting plane when possible; nonlinear planar curves use their start tangent
and an oriented plane through their control points. The separate
[nonplanar fixture](../../tools/rhino_oracle/fixtures/construction_plane_object_nonplanar.json)
records Rhino's start frames for a nonplanar polyline and NURBS curve. The
[saved observations](../../tools/rhino_oracle/observations/construction_plane_object_curve.json)
and [nonplanar observations](../../tools/rhino_oracle/observations/construction_plane_object_nonplanar.json)
were captured without using the shared desktop. Replay the curve cases with:

```sh
python3 -m tools.rhino_oracle replay tools/rhino_oracle/fixtures/construction_plane_object_curve.json --observations tools/rhino_oracle/observations/construction_plane_object_curve.json --absolute-epsilon 1e-10 --relative-epsilon 1e-12
```

All 15 curve cases replay within `1.12e-16` component error. Both nonplanar
cases replay exactly using the same tolerances.

The [polycurve Object fixture](../../tools/rhino_oracle/fixtures/construction_plane_object_polycurve.json)
adds six private-Xvfb captures for open, reversed, tilted, closed, nonplanar,
and mixed NURBS/line joins. Its
[edge fixture](../../tools/rhino_oracle/fixtures/construction_plane_object_polycurve_edges.json)
adds one-segment and collinear joins. The corresponding
[saved frames](../../tools/rhino_oracle/observations/construction_plane_object_polycurve.json)
and [edge frames](../../tools/rhino_oracle/observations/construction_plane_object_polycurve_edges.json)
were produced by `_CPlane _Object` on a separate display. Replay the main set:

```sh
python3 -m tools.rhino_oracle replay tools/rhino_oracle/fixtures/construction_plane_object_polycurve.json --observations tools/rhino_oracle/observations/construction_plane_object_polycurve.json --absolute-epsilon 1e-10 --relative-epsilon 1e-12
```

All nine polycurve cases replay at the same tolerances, with a largest axis
component difference of `1.12e-16`.

The separate [Through All diagnostic](../../tools/rhino_oracle/fixtures/construction_plane_through_all_diagnostic.json)
and [observation](../../tools/rhino_oracle/observations/construction_plane_through_all_diagnostic.json)
capture a discrepancy in Rhino 8.32's typed command path: after a command-level
World Top reset, `_CPlane _Through _All=_Yes w7,8,9` changed only the active
Top plane to a Right-oriented plane through world X=7. Rhino's published CPlane
help says the option moves each viewport's plane along its own normal. The
Viboceros implementation follows that documented geometry rule; parity for
this typed Rhino path remains unresolved.

The [Through All paths fixture](../../tools/rhino_oracle/fixtures/construction_plane_through_all_paths.json)
and [raw observation](../../tools/rhino_oracle/observations/construction_plane_through_all_paths.json)
reproduce that discrepancy on a private Xvfb display for seven cases: every
standard active viewport, local input, an oblique active plane, and an explicit
Through All=No followed by All=Yes. Top, Front, and Perspective acquire the
Right plane through X=7; the active Right viewport instead acquires the Front
plane through Y=8. Other viewport planes remain unchanged. The All=No step
correctly preserves the active Top axes and moves its origin to Z=9. These
diagnostics do not establish Through All parity.

The [All options fixture](../../tools/rhino_oracle/fixtures/construction_plane_all_options.json)
and [raw observation](../../tools/rhino_oracle/observations/construction_plane_all_options.json)
capture twelve command workflows in a private Xvfb display. Both origin and
Through All settings persist through completion and cancellation; bare All
toggles the corresponding setting. After selecting origin All, Rhino accepts
further All changes and origin points but rejects View in that same command.
An app regression runs the options as separate prompt entries and compares all
four viewport frames exactly in nine workflows, also checking that cameras,
pending model input, and model undo stay intact. The three workflows that
actually apply Through All=Yes retain the discrepancy above and are diagnostic
captures, not passing parity assertions.

The [SynchronizeCPlanes fixture](../../tools/rhino_oracle/fixtures/synchronize_cplanes.json)
records eight Rhino 8.32 cases for world-preset and oblique source planes,
including both `SetView` settings. Its
[observation](../../tools/rhino_oracle/observations/synchronize_cplanes.json)
was captured by clicking only the Rhino window launched in a private Xvfb
display. Regenerate it with:

```sh
tools/rhino_oracle/run_headless.sh rhino tools/rhino_oracle/fixtures/synchronize_cplanes.json --timeout 240
```

The captures show that Perspective copies the source CPlane while its camera
stays fixed. Parallel cameras move only with `SetView=Yes`. For an oblique
source, Top copies the source plane, and Front and Right rotate from it. When a
standard parallel view already has an exact world-preset CPlane, Rhino uses
that preset as the rotation role; this is visible when Top starts with World
Right, Front, Bottom, Left, or Back. Applying those roles to the saved source
frames reproduces all recorded CPlane axes within `1.12e-16` component error.
