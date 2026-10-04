# Rhino compatibility oracle

[Project overview](../README.md)

The [Bend oracle](bend-geometry.md) retains 76 public SDK point maps, the original
48 point-command recipes, 76 geometry commands, and two preference workflows
from private Xvfb sessions. SDK maps replay through the Rust/Python protocol;
command and UI tests compare geometry, attributes, groups, selection, Undo/Redo
and 41 option/history steps. The [command guide](commands/bend.md) documents
numeric angle memory and fitting bounds. Another 35 [cursor captures](bend-preview.md)
constrain live previews, display modes and the spine-start mouse plane.

The [Taper foundation](taper-geometry.md) retains 51 SDK point maps and six owned
native point commands on private Xvfb. The Python/Rust `taper_points` protocol
replays validity and coordinates; command snapshots constrain Copy, grouped
Rigid and Undo/Redo. Follow-on [Taper command tests](commands/taper.md) use 90
geometry recipes and a 33-step preference/history workflow, including signed
distances, radial point picks, construction planes and a fitting sweep. Native
precision diagnostics are retained in the [provenance](taper-command-provenance.json).
Another 30 [Taper cursor captures](taper-preview.md) constrain quick previews,
display modes and axis-normal mouse planes, with six earlier plane diagnostics
retained separately. All were captured using private Xvfb settings schemes.

The [Maelstrom foundation](maelstrom-geometry.md) retains 108 public SDK point
maps and eight native point commands from private Xvfb. The Python/Rust
`maelstrom_points` protocol replays validity, radial profiles and coordinates;
actual commands constrain Copy, grouped Rigid, selection and Undo/Redo. This is
geometry evidence. Its [command captures](commands/maelstrom.md) add 85 geometry
recipes and 29 preference/history steps, including the fitting floor, unchanged
Undo entries, first-radius memory, per-command Rigid defaults and immediate Copy
getter persistence. Interactive input and previews remain to implement.

The [CPlane All fixture](../tools/rhino_oracle/fixtures/construction_plane_all.json)
records standard and oblique starting planes with four independent viewports.
Its [private-Xvfb Rhino 8.32 observation](../tools/rhino_oracle/observations/construction_plane_all.json)
shows that `All=Yes` moves every origin to the picked world point while leaving
all plane axes and camera targets unchanged. Regenerate it with:

```sh
tools/rhino_oracle/run_headless.sh rhino tools/rhino_oracle/fixtures/construction_plane_all.json --timeout 180
```

The [CPlane View fixture](../tools/rhino_oracle/fixtures/construction_plane_view.json)
uses the public viewport API to place an oblique CPlane in Top and Perspective
views, then runs `_CPlane _View`. Its [Rhino 8.32 observation](../tools/rhino_oracle/observations/construction_plane_view.json)
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

The [3Point option fixture](../tools/rhino_oracle/fixtures/construction_plane_three_point_options.json)
records five Rhino 8.32 transitions for `Vertical` and `ZAxis`, starting from
world and oblique CPlanes. Its [private-Xvfb observation](../tools/rhino_oracle/observations/construction_plane_three_point_options.json)
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

The [picked Rotate fixture](../tools/rhino_oracle/fixtures/construction_plane_rotate_points.json)
records four Rhino 8.32 transitions: positive and negative quarter turns,
an offset rotation axis, and an oblique axis with an oblique starting plane.
Its [private-Xvfb observation](../tools/rhino_oracle/observations/construction_plane_rotate_points.json)
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
[circles](../tools/rhino_oracle/fixtures/construction_plane_object_circle.json),
[arcs](../tools/rhino_oracle/fixtures/construction_plane_object_arc.json),
[ellipses](../tools/rhino_oracle/fixtures/construction_plane_object_ellipse.json), and
[surfaces](../tools/rhino_oracle/fixtures/construction_plane_object_surface.json)
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

The [mesh face fixture](../tools/rhino_oracle/fixtures/construction_plane_object_mesh_face.json)
adds four private-Xvfb captures: a triangle, a tilted quad, an indexed second
face, and fractional vertex coordinates. Its
[Rhino observation](../tools/rhino_oracle/observations/construction_plane_object_mesh_face.json)
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

The [B-rep face fixture](../tools/rhino_oracle/fixtures/construction_plane_object_brep_face.json)
captures `_CPlane _Object` for all six faces of a box created by Rhino, with one
case starting from an offset Front CPlane. Its
[saved observations](../tools/rhino_oracle/observations/construction_plane_object_brep_face.json)
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

The [CPlane Surface fixture](../tools/rhino_oracle/fixtures/construction_plane_surface.json)
records six private-Xvfb Rhino 8.32 transitions on planar and warped surfaces.
It covers the default UV midpoint/U direction, chosen origin and X points,
and world points outside the tangent plane. Its
[saved observation](../tools/rhino_oracle/observations/construction_plane_surface.json)
is compared with the native closest-point and tangent-frame calculation:

```sh
tools/rhino_oracle/run_headless.sh rhino tools/rhino_oracle/fixtures/construction_plane_surface.json --timeout 180
python3 -m tools.rhino_oracle replay \
  tools/rhino_oracle/fixtures/construction_plane_surface.json \
  --observations tools/rhino_oracle/observations/construction_plane_surface.json \
  --absolute-epsilon 1e-10 --relative-epsilon 1e-12
```

All six Surface cases replay; the largest component difference is `4.44e-16`.

The [Surface option fixture](../tools/rhino_oracle/fixtures/construction_plane_surface_options.json)
adds ten private-Xvfb cases for `Flip=Yes|No`, with default and picked
origins, chosen X directions, and a warped surface. Its
[saved observation](../tools/rhino_oracle/observations/construction_plane_surface_options.json)
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

The [trimmed Surface fixture](../tools/rhino_oracle/fixtures/construction_plane_surface_trimmed.json)
records six private-Xvfb Rhino 8.32 cases on a planar face with a square hole.
Picked origins inside the hole snap to the nearest hole boundary; `IgnoreTrims`
uses the underlying surface. Picks outside the face clamp to its outer edge
even with `IgnoreTrims`, because the supporting surface is bounded. Enter at
the origin prompt uses the untrimmed UV midpoint. The
[saved observation](../tools/rhino_oracle/observations/construction_plane_surface_trimmed.json)
is replayed with:

```sh
python3 -m tools.rhino_oracle replay \
  tools/rhino_oracle/fixtures/construction_plane_surface_trimmed.json \
  --observations tools/rhino_oracle/observations/construction_plane_surface_trimmed.json \
  --absolute-epsilon 1e-10 --relative-epsilon 1e-12
```

All six trimmed Surface cases replay; the largest component difference is
`1.42e-14`.

The [Curve CPlane fixture](../tools/rhino_oracle/fixtures/construction_plane_curve.json)
records 18 private-Xvfb Rhino 8.32 cases: horizontal, vertical, oblique, and
reversed lines; off-curve and endpoint picks; a different initial CPlane; the
default start station; a polyline segment and corner; horizontal and vertical
circle stations; and planar and spatial quadratic NURBS stations. The
[saved observation](../tools/rhino_oracle/observations/construction_plane_curve.json)
is replayed with:

```sh
python3 -m tools.rhino_oracle replay \
  tools/rhino_oracle/fixtures/construction_plane_curve.json \
  --observations tools/rhino_oracle/observations/construction_plane_curve.json \
  --absolute-epsilon 1e-10 --relative-epsilon 1e-12
```

All 18 Curve cases replay; the largest component difference is `3.06e-16`.

The [CPlane copy-pick fixture](../tools/rhino_oracle/fixtures/copy_cplane_pick.json)
records private-Xvfb clicks in the Top viewport for `CopyCPlaneToAll` and
`CopyCPlaneSettingsToAll`, with the Perspective viewport initially active.
The [saved observation](../tools/rhino_oracle/observations/copy_cplane_pick.json)
shows that the plane command copies the frame, numeric grid settings, grid
visibility, and construction-axis visibility while preserving each target's
world-axis icon setting. The settings command copies all grid settings and
leaves target frames intact. Both make the clicked source viewport active.
The plane command's `CPlane Undo` restores the selected target's old frame and
numeric grid settings, one viewport at a time, while its visibility settings
stay copied. An app regression checks the saved viewport states and cameras.

The [curve Object fixture](../tools/rhino_oracle/fixtures/construction_plane_object_curve.json)
records 15 private-Xvfb cases for lines, open and closed polylines, and
polynomial or rational NURBS curves. Linear curves use an axis-aligned
supporting plane when possible; nonlinear planar curves use their start tangent
and an oriented plane through their control points. The separate
[nonplanar fixture](../tools/rhino_oracle/fixtures/construction_plane_object_nonplanar.json)
records Rhino's start frames for a nonplanar polyline and NURBS curve. The
[saved observations](../tools/rhino_oracle/observations/construction_plane_object_curve.json)
and [nonplanar observations](../tools/rhino_oracle/observations/construction_plane_object_nonplanar.json)
were captured without using the shared desktop. Replay the curve cases with:

```sh
python3 -m tools.rhino_oracle replay tools/rhino_oracle/fixtures/construction_plane_object_curve.json --observations tools/rhino_oracle/observations/construction_plane_object_curve.json --absolute-epsilon 1e-10 --relative-epsilon 1e-12
```

All 15 curve cases replay within `1.12e-16` component error. Both nonplanar
cases replay exactly using the same tolerances.

The [polycurve Object fixture](../tools/rhino_oracle/fixtures/construction_plane_object_polycurve.json)
adds six private-Xvfb captures for open, reversed, tilted, closed, nonplanar,
and mixed NURBS/line joins. Its
[edge fixture](../tools/rhino_oracle/fixtures/construction_plane_object_polycurve_edges.json)
adds one-segment and collinear joins. The corresponding
[saved frames](../tools/rhino_oracle/observations/construction_plane_object_polycurve.json)
and [edge frames](../tools/rhino_oracle/observations/construction_plane_object_polycurve_edges.json)
were produced by `_CPlane _Object` on a separate display. Replay the main set:

```sh
python3 -m tools.rhino_oracle replay tools/rhino_oracle/fixtures/construction_plane_object_polycurve.json --observations tools/rhino_oracle/observations/construction_plane_object_polycurve.json --absolute-epsilon 1e-10 --relative-epsilon 1e-12
```

All nine polycurve cases replay at the same tolerances, with a largest axis
component difference of `1.12e-16`.

The separate [Through All diagnostic](../tools/rhino_oracle/fixtures/construction_plane_through_all_diagnostic.json)
and [observation](../tools/rhino_oracle/observations/construction_plane_through_all_diagnostic.json)
capture a discrepancy in Rhino 8.32's typed command path: after a command-level
World Top reset, `_CPlane _Through _All=_Yes w7,8,9` changed only the active
Top plane to a Right-oriented plane through world X=7. Rhino's published CPlane
help says the option moves each viewport's plane along its own normal. The
Viboceros implementation follows that documented geometry rule; parity for
this typed Rhino path remains unresolved.

The [Through All paths fixture](../tools/rhino_oracle/fixtures/construction_plane_through_all_paths.json)
and [raw observation](../tools/rhino_oracle/observations/construction_plane_through_all_paths.json)
reproduce that discrepancy on a private Xvfb display for seven cases: every
standard active viewport, local input, an oblique active plane, and an explicit
Through All=No followed by All=Yes. Top, Front, and Perspective acquire the
Right plane through X=7; the active Right viewport instead acquires the Front
plane through Y=8. Other viewport planes remain unchanged. The All=No step
correctly preserves the active Top axes and moves its origin to Z=9. These
diagnostics do not establish Through All parity.

The [All options fixture](../tools/rhino_oracle/fixtures/construction_plane_all_options.json)
and [raw observation](../tools/rhino_oracle/observations/construction_plane_all_options.json)
capture twelve command workflows in a private Xvfb display. Both origin and
Through All settings persist through completion and cancellation; bare All
toggles the corresponding setting. After selecting origin All, Rhino accepts
further All changes and origin points but rejects View in that same command.
An app regression runs the options as separate prompt entries and compares all
four viewport frames exactly in nine workflows, also checking that cameras,
pending model input, and model undo stay intact. The three workflows that
actually apply Through All=Yes retain the discrepancy above and are diagnostic
captures, not passing parity assertions.

The [SynchronizeCPlanes fixture](../tools/rhino_oracle/fixtures/synchronize_cplanes.json)
records eight Rhino 8.32 cases for world-preset and oblique source planes,
including both `SetView` settings. Its
[observation](../tools/rhino_oracle/observations/synchronize_cplanes.json)
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

The [SetPt transform fixture](../tools/rhino_oracle/fixtures/plane_transforms_setpt.json)
compares seven world/CPlane axis combinations, including copy mode, with a
[Rhino 8 observation](../tools/rhino_oracle/observations/plane_transforms_setpt.json)
captured on a separate Xvfb display. The maximum point-coordinate difference
is `2.28e-15` at `1e-9` absolute and `1e-12` relative tolerance. Replay it with:

```sh
python3 -m tools.rhino_oracle replay tools/rhino_oracle/fixtures/plane_transforms_setpt.json --observations tools/rhino_oracle/observations/plane_transforms_setpt.json --absolute-epsilon 1e-9 --relative-epsilon 1e-12
```

The [two-face UV reference](surface-face-uv-rhino-reference.json) records four
RhinoCommon projections onto explicitly indexed B-rep underlying surfaces.
The matching [fixture](../tools/rhino_oracle/fixtures/surface-face-uv-api.json)
ran on a separate Xvfb display; a native regression compares UV parameters,
projected points, and distances to `1e-10`. Python replay matches all four
queries, with maximum absolute error `4.44e-16`:

```sh
python3 -m tools.rhino_oracle replay tools/rhino_oracle/fixtures/surface-face-uv-api.json --observations docs/surface-face-uv-rhino-reference.json --absolute-epsilon 1e-10 --relative-epsilon 1e-10
```

This is a geometry API comparison, not an interactive `EvaluateUVPt` click capture.
The [curved-face fixture](../tools/rhino_oracle/fixtures/surface-face-uv-curved-api.json)
adds a rational cylinder and a quadratic surface with separate non-unit UV
domains. Its six queries match the [saved Rhino result](surface-face-uv-curved-rhino-reference.json)
within `1e-10`; the largest absolute replay difference is `3.13e-12`. Both
fixtures also exercise the document-level face-qualified UV query.
Regenerate the curved observation with:

```sh
tools/rhino_oracle/run_headless.sh rhino tools/rhino_oracle/fixtures/surface-face-uv-curved-api.json --timeout 300
```

[Diagnostic replay and Python API](oracle-replay.md) retain per-operation native
errors while comparing every successful record against saved Rhino observations.
On Linux, the CLI automatically runs live `rhino` and `compare` probes under
the dedicated `run_headless.sh` Xvfb display. Direct Python API calls to
`OracleClient.run_rhino` on Linux require an Xvfb session established by this
wrapper. Replay and native-only modes do not launch Rhino.

For experiments that change application preferences or remembered command
defaults, use a private settings scheme:

```python
client = OracleClient(settings_scheme="VibocerosOracleCopy_20261002")
observed = client.run_rhino(request)
```

Run this Python code through `tools/rhino_oracle/run_headless.sh exec` as usual.
Scheme names must start with `VibocerosOracle`, followed by 1–64 ASCII letters,
digits, underscores, or hyphens. The client passes a single `/scheme=` launcher
argument. Rhino stores the scheme's options separately, as documented in
[McNeel's startup options](https://docs.mcneel.com/rhino/8/help/en-us/information/startingrhino.htm).
The [50-step Copy preference workflow](commands/remember-copy-options.md#native-verification)
uses this launch path to measure command starts, completed choices, cancellation,
and re-enabling in Rhino 8.32. Copy settings probes require a private scheme.
Run native captures sequentially within a Wine prefix.

The [Match fixture](../tools/rhino_oracle/fixtures/curve_match_geometry.json)
contains 33 live Rhino `CreateMatchCurve` cases for single-span line, polynomial,
and rational curves across end orientation, continuity, and preserved opposite
end options. Replay against its [saved observation](../tools/rhino_oracle/observations/curve_match_geometry.json)
with `python3 -m tools.rhino_oracle replay tools/rhino_oracle/fixtures/curve_match_geometry.json --observations tools/rhino_oracle/observations/curve_match_geometry.json`.
All 33 outputs agree within the default 1e-10 absolute tolerance. The
[average-tangency fixture](../tools/rhino_oracle/fixtures/curve_match_average_tangency.json)
adds eight live cases, including rational handles and reversed selected ends;
all eight match their [Rhino observations](../tools/rhino_oracle/observations/curve_match_average_tangency.json)
within the same tolerance. The
[average-curvature fixture](../tools/rhino_oracle/fixtures/curve_match_average_curvature.json)
adds six matching [Rhino observations](../tools/rhino_oracle/observations/curve_match_average_curvature.json)
for opposite-end preservation of None or Position. The
[average-position fixture](../tools/rhino_oracle/fixtures/curve_match_average_position.json)
adds ten matching [Rhino observations](../tools/rhino_oracle/observations/curve_match_average_position.json)
for trimmed and reversed ends, including rational controls. One
[boundary diagnostic](../tools/rhino_oracle/fixtures/curve_match_average_position_boundary.json)
retains a difference: Rhino's `CreateMatchCurve` sometimes trims a straight
cubic to a `2^-22` parameter tail when the midpoint projects onto the far
endpoint. Both public `ClosestPoint` and the native closest-point search return
that endpoint exactly. Additional private-Xvfb probes found the tail at lengths
1 through 90 in several cases, but not at lengths 12 and 100. The
[average multi-span curvature fixture](../tools/rhino_oracle/fixtures/curve_match_average_multispan_curvature.json)
adds 13 matching [Rhino observations](../tools/rhino_oracle/observations/curve_match_average_multispan_curvature.json)
for multi-span sources and references, rational weights, uneven knots, reversed
ends, and preserved far tangents. Its
[boundary fixture](../tools/rhino_oracle/fixtures/curve_match_average_multispan_curvature_boundary.json)
and its two [Rhino observations](../tools/rhino_oracle/observations/curve_match_average_multispan_curvature_boundary.json)
now match after uniform Greville preparation of short source curves. The
[multi-span tangency fixture](../tools/rhino_oracle/fixtures/curve_match_multispan_tangency.json)
adds ten matching [Rhino observations](../tools/rhino_oracle/observations/curve_match_multispan_tangency.json)
for cubic and quadratic two-span sources, opposite-end preservation, and
reversed picks. The
[multi-span position fixture](../tools/rhino_oracle/fixtures/curve_match_multispan_position.json)
adds seven matching [Rhino observations](../tools/rhino_oracle/observations/curve_match_multispan_position.json).
A more precise NURBS closest-point stopping rule resolves the interior trim
parameter at the default model tolerance. The
[multi-span curvature fixture](../tools/rhino_oracle/fixtures/curve_match_multispan_curvature.json)
adds three matching [Rhino observations](../tools/rhino_oracle/observations/curve_match_multispan_curvature.json)
for preserved opposite-end options None, Position, and Tangency. Its
[extended fixture](../tools/rhino_oracle/fixtures/curve_match_multispan_curvature_extended.json)
adds 15 matching [observations](../tools/rhino_oracle/observations/curve_match_multispan_curvature_extended.json)
for uneven knots, rational weights, and reversed selected ends. The
[uneven rational boundary case](../tools/rhino_oracle/fixtures/curve_match_multispan_curvature_boundary.json)
now matches its [Rhino observation](../tools/rhino_oracle/observations/curve_match_multispan_curvature_boundary.json).
The [12-case transition fixture](../tools/rhino_oracle/fixtures/curve_match_multispan_curvature_b1_rule.json)
and [live observation](../tools/rhino_oracle/observations/curve_match_multispan_curvature_b1_rule.json)
vary the first rational weight, first interior knot, and original second-control
projection. In these samples, Rhino chooses the curvature-derived tangential
control when it differs from the original projection by at most 10% of their
mean magnitude;
all 12 native replays match within `1e-10`. These probes ran on a separate Xvfb
display. The [five-control probe](../tools/rhino_oracle/fixtures/curve_match_rhino_only.json)
now matches after interpolation at affine Greville parameters into a uniform,
nonrational six-control curve. The
[six-case Rhino probe](../tools/rhino_oracle/fixtures/curve_match_five_control_far_g2_probe.json)
and [observations](../tools/rhino_oracle/observations/curve_match_five_control_far_g2_probe.json)
cover both selected ends, uneven knots, a changed middle control, and rational
sources; all six native replays agree within `1e-10`. The
[two-case average boundary](../tools/rhino_oracle/fixtures/curve_match_average_multispan_curvature_boundary.json)
also now matches within `1e-10`. The
[four-case quadratic arc fixture](../tools/rhino_oracle/fixtures/curve_match_five_control_quadratic_end.json)
and [observations](../tools/rhino_oracle/observations/curve_match_five_control_quadratic_end.json)
cover both selected ends with one-sided and average matching; all four native
replays agree within `1e-10`. The
[six-case quadratic NURBS fixture](../tools/rhino_oracle/fixtures/curve_match_five_control_quadratic_general.json)
and [observations](../tools/rhino_oracle/observations/curve_match_five_control_quadratic_general.json)
cover both selected ends, rational weights, and uneven knots; all six match
within `1e-10`. The
[four-control quadratic fixture](../tools/rhino_oracle/fixtures/curve_match_four_control_quadratic_g2.json)
and [observations](../tools/rhino_oracle/observations/curve_match_four_control_quadratic_g2.json)
exercise two added controls, both selected ends, and rational weights. These live
captures used a separate Xvfb display.
The [Rebuild comparison](../tools/rhino_oracle/fixtures/curve_match_five_control_rebuild_probe.json)
and [observations](../tools/rhino_oracle/observations/curve_match_five_control_rebuild_probe.json)
show Rhino's public `NurbsCurve.Rebuild(6, 3, ...)` does not produce the same
far controls as `CreateMatchCurve`. The
[translated rational sweep](../tools/rhino_oracle/fixtures/curve_match_five_control_rational_translation_sweep.json)
and [observations](../tools/rhino_oracle/observations/curve_match_five_control_rational_translation_sweep.json)
cover 13 X translations from `-15` to `+15`; all match within `1e-10` after
private-Xvfb capture. The source's homogeneous XYZ control-distance ratio determines
the curvature control offset; Rhino uses the absolute squared difference when
the transverse curvature offset exceeds that radius.
The [five-case supported fixture](../tools/rhino_oracle/fixtures/curve_match_multispan_preserve_far_curvature.json)
and [Rhino observation](../tools/rhino_oracle/observations/curve_match_multispan_preserve_far_curvature.json)
cover six- and seven-control cubic sources, both selected ends, a rational
source, and an average match between two six-control curves. Their far three
controls and knot vectors remain unchanged; native replay agrees to `1e-10`.
These captures also ran on a separate Xvfb display.
Replay the supported case with:

```sh
python3 -m tools.rhino_oracle replay tools/rhino_oracle/fixtures/curve_match_multispan_preserve_far_curvature.json --observations tools/rhino_oracle/observations/curve_match_multispan_preserve_far_curvature.json
```

Replay the transition fixture with:

```sh
python3 -m tools.rhino_oracle replay tools/rhino_oracle/fixtures/curve_match_multispan_curvature_b1_rule.json --observations tools/rhino_oracle/observations/curve_match_multispan_curvature_b1_rule.json
```

The `viewport_arrangement_probe` operation records model viewport bounds,
titles, cameras, projection, floating state, and active view after a bounded
sequence of `NewViewport`, `CloseViewport`, `3View`, `4View`, and viewport split
commands. Run its [fixture](../tools/rhino_oracle/fixtures/viewport_arrangement.json)
with `tools/rhino_oracle/run_headless.sh rhino tools/rhino_oracle/fixtures/viewport_arrangement.json --timeout 300`.
It uses public [RhinoView properties](https://mcneel.github.io/rhinocommon-api-docs/api/RhinoCommon/html/Properties_T_Rhino_Display_RhinoView.htm).
The [default-view](viewport-arrangement-rhino-reference.json) and
[Front-active](viewport-arrangement-front-rhino-reference.json) Rhino 8.32
responses on 2026-09-25 confirm a centered, overlapping Top view and activation
of the previously active view when it closes. A
[repeated-NewViewport run](viewport-arrangement-repeated-rhino-reference.json)
confirms that successive views stack at the same centered rectangle and close
back through the prior active views. A
[Shaded-source run](viewport-arrangement-shaded-rhino-reference.json) confirms
that the new Top view starts in Wireframe even when the source is Shaded.
The Wine session required a normal
launcher `stop` before these probes could start.
The [4View projection run](viewport-arrangement-projection-rhino-reference.json)
checks both first- and third-angle layouts, their camera directions and active
Perspective view, and a Front-active reset. The worker sends Rhino's command-line
option as `Projection=<angle>` followed by Enter.
The [repeated 4View run](viewport-arrangement-fourview-repeat-rhino-reference.json)
checks a split four-view arrangement, a changed Top camera, a maximized Top
view, and a shifted Perspective camera. In these cases the first plain `4View`
call restored standard views and activated Perspective. Shaded Perspective
survived either projection arrangement, while Right-to-Left and Bottom-to-Top
replacements started in Wireframe.
The [grid-spacing run](viewport-arrangement-fourview-grid-rhino-reference.json)
sets distinctive grid and snap spacing through each viewport's public
construction-plane API before plain and explicit `4View` calls. It confirms
that unchanged Top keeps its spacing, first-angle Left takes Right's spacing,
and returning to third-angle resets Right's spacing. Each operation first
establishes a standard four-view layout with unit grid and snap spacing.
Changing Top to Bottom or Front to Back before plain `4View` gives the restored
Top or Front and Right the changed viewport's spacing. A changed Right-to-Left
view restores Right's spacing without changing the other grids.

The [orientation audit](orientation-audit.md) separates public document insertion
and replacement from actual `Flip` command behavior, retaining full definitions
and selection states. This diagnostic is not an identical-source native replay.
The separate [solid-orientation query](solid-orientation.md) compares 49 shared
3dm inputs without document insertion: all geometry records and 47 orientations
now match, with two coincident-shell orientations unresolved. Its
[planar follow-up](planar-solid-orientation.md) retains 68 further cases and eight
fresh-session repeats, exposing ten translation-sensitive Rhino classifications
despite identical geometric definitions after exact inverse translation.
The [Join scale audit](join-orientation.md) adds full-definition command records
at unit and volume-overflow scales, separating fixed native execution errors
from Rhino's large-coordinate orientation and normalization discrepancies.
The [shared-source document admission probe](document-brep-admission.md) separates
insertion and replacement using identical caller-owned inputs, with full
definitions, identity, attributes, groups, selection, and immutability checks.
It now resolves 40 normalization differences (56/64 full matches); a separate
16-case actual file-import capture has 14 full matches. Coincident-shell
orientation remains unresolved in both audits, with all numeric fields equal.

The [conic-center audit](conic-center-audit.md) compares 32 full-coordinate public
API records and 62 calibrated point-prompt records. It keeps API recognition,
interactive snap policy, and their remaining discrepancies separate.

The [unconstrained point-snap probe](point-snaps.md) records full 3D targets,
snap kinds and source ownership through public `GetPoint` APIs. Its 101 retained
line/mesh cases resolve the previous Mesh Near coordinate discrepancies while
preserving three corner-edge selection differences. Source/camera-only rational
references and native replay remain separate from the observed picked points.
The [mesh-competition follow-up and point replay API](mesh-snap-order.md) expose
broader wire-selection differences with 84 retained captures, including repeated
vertex-order checks and public picking diagnostics. `projected_object_snap`
accepts calibrated native inputs; `python3 -m tools.rhino_oracle.point_snap_replay`
compares target/kind/source and returns exit status 1 for real parity differences.

The `join_command` probe has 181 raw mesh records covering both
`JoinDisjointMeshes` choices, precision thresholds, ordered selection, winding,
attributes, groups, and identity replacement. It checks identical binary64
vertices and face indices before and after inserting each Rhino source;
no mesh output normalization is used. A further 140 paired Join/JoinCopy
workflow records cover curves and meshes, selection, no-ops, and source retention.
Curve records include native type, domains, sampled positions, NURBS definitions,
and raw polycurve local domains plus parent intervals; no affine-domain
normalization is applied during recording. Another 284 cases cover closed-chain
seams and early completion. The probe snapshots the named command's public
EndCommand event, requires exactly one completion, and always detaches its
handler. Callback errors fail the probe even if the host swallows exceptions.
The native adapter stops individual selection at the same completion boundary.
See [joining evidence and limits](commands/join.md) and the
[event transcripts and measurement correction](join-cycles.md).
Another [544 encoding and seam records](join-encodings.md) check rational weights,
higher-degree controls, polycurve leaves, and their local/parent parameter maps.
An additional [48 endpoint-search records](join-endpoint-search.md) check nearby
connections in the presence of distant unrelated sources, with absolute-only
comparison and exact preservation of the distant source records.
The [one-pass seeded-join audit](seeded-join.md) replays all 1,197 Join records and
refreshes the 140 workflow cases against an owned private-Xvfb Rhino session.

Border duplication has 86 matching live command records and offline replay
checks, including edge-table permutations. See [border validation](borders.md)
for the resolved polysurface seam discrepancies and identical-source 3DM protocol.

The [Cap command audit](commands/cap.md#oracle-evidence) adds 140 matching
shared-input B-rep records, including four resolved kinked-boundary segmentation
gaps. Two additional raw discrepancies cover negative rational weights and a
large knot origin; the latter also retains analytic area/volume witnesses.
Its schema compares exact spatial edge definitions and oriented incidence,
not newly generated cap UV parameterizations. The probe verifies that Rhino
document insertion preserves the shared source before invoking the command.
The [compound follow-up](cap-compound-orientation.md) adds 100 actual command
records: 96 match after correcting 28 total-volume orientation errors, while
four coincident-shell topology discrepancies remain explicit.

The [B-rep edge assembly audit](brep-edge-joining.md) compares explicitly paired
native boundaries with public automatic RhinoCommon `JoinBreps` on identical
shared 3DM inputs. It records full surfaces and UV trim definitions in addition
to spatial topology, tolerances, and mass properties. Raw orientation and
gap-rebuilding policy differences are retained separately. This geometry-only
probe does not establish interactive Join behavior or native automatic matching.

The separate [surface Join command audit](commands/join.md#surfaces-and-polysurfaces)
adds 92 actual Join/JoinCopy records with shared per-source 3DM artifacts.
Its original 26-case discrepancy archive is retained, but four partial-overlap
and sixteen gap cases now fully match; four duplicate-wall cases differ only in zero-volume
face senses. The [follow-up boundary audit](join-boundary-matching.md) adds 160
records, now 148 fully matching and 12 explicit zero-volume orientation differences.
The [gap-rebuilding audit](join-gap-rebuilding.md) adds 108 cases and resolves
20 earlier differences. The [selection-distance audit](join-selection-distance.md)
adds 150 cases and resolves two more earlier differences. The
[corner-only follow-up](join-corner-clustering.md) adds 84 cases, separating
endpoint movement from edge mating. Across all five audits,
471 of 594 cases fully match; the remaining 123 explicitly cover zero-volume
orientation, cutoff endpoints, order-dependent clusters, and area integration.
The command probe includes spatial and UV geometry, component
tolerances, oriented topology, object identity/selection, attributes, and groups.
Copies with a repeated source retain every disconnected piece. Neither failed
comparisons nor domain differences are normalized out of the saved observations.
Subsequent curved, projective and [short-overlap audits](join-short-overlaps.md)
bring the cumulative surface-command record to 804 cases: 633 full matches,
32 native errors and 139 other differences. The short-overlap work also corrects
native source construction to use the fixture tolerance, independently of a
command's document-tolerance override, so both engines operate on the same
short-feature inputs.
The [partial-boundary certificate audit](join-trim-certificates.md) resolves
eighteen of those differences and adds eighty pre-split curved records, retaining
all newly exposed seam-cleanup, outer-knot and area differences. That audit's total
is 651 full matches, 32 native errors and 201 other differences in 884 cases.
The [tensor-isocurve audit](join-isocurve-certificates.md) adds 33 comparable
records, with area/outer-knot differences retained: 651 full matches, 32 native
errors and 234 other differences across 917 cases. Another 31 translated cases
have native-only evidence, explicitly excluded from comparison counts. Its shared
`surface_face` sources can use `trim_bounds: [[u0,u1],[v0,v1]]` while retaining the
complete underlying surface. This shared-input recipe uses four unit-domain UV
trim curves by default, preserving the exact inputs of the retained archives.
`trim_domains: [[t0,t1], ...]` explicitly supplies the four scalar intervals in
South/East/North/West order, before edge subdivision. This is independent of UV
control coordinates and spatial edge domains. The kernel's natural-face
constructor now follows surface-axis intervals instead; the harness builds its
declared input curves, without rewriting expected output domains or observations.

Shared `box` sources likewise retain the UV definitions of the native-written
3DM inputs used in the saved archives. Two walls use their original cyclic UV
corner order. The source adapter constructs these faces before edge splitting,
reordering, reversal, or command execution. The kernel box constructor follows
Rhino's natural wall axes, as checked by the six `CPlane Object` face captures;
changing that constructor must not silently change a saved replay's inputs.

The [redundant-edge cleanup audit](join-edge-cleanup.md) adds 32 angular-tolerance
records using the same sixteen shared source artifacts at four document angles.
The cumulative total is 679 full matches, 32 native errors and 238 other
differences in 949 cases. Twelve previous cases become matches and none regress;
small-angle representation-dependent Rhino topology remains an explicit difference.

The `align` object-layout probe compares actual bounding-box, curve and line/plane alignment commands,
including retained IDs, source samples/domains, groups, layer assignment and
pre/postselection cleanup. Its shared `object_layout` module supplies fixture
ownership and recording for Align and Distribute, not the alignment algorithm.
[Alignment evidence](commands/align.md#oracle-evidence) includes bounding-box,
line/plane and [best-fit plane](plane-fit.md) commands. It retains raw curved-bound,
mesh-storage and nonunique-normal discrepancies alongside successful comparisons.
All Rhino command probes run on an owned private Xvfb display.
For `ToCurve`, `curve` is an unselected curve's index in `sources` and `selected`
must explicitly omit it. Target IDs are bound only after owned source creation;
arbitrary command text is never accepted as a target. The native adapter uses
the corresponding document UUID. The 39-case curve fixture includes all seven
native target families and retains a separate incomplete-prompt diagnostic.

The `split_edge_command` operation uses the shared owned edge-edit fixture and
history recorder, with a separate strict point-input grammar. Its
[21-case record](split-edge-provenance.json) exercises actual mouse-selected
SplitEdge commands, including full-batch duplicate failure and unchanged
endpoint-only replacements with real Undo/Redo. Native replay compares all
ordered geometry/attribute/history fields without component renumbering; see
[SplitEdge](commands/split-edge.md) for interaction and search limitations.
An additional [19 distance-constraint records](split-edge-distances.md) use
bounded interleaved typed distance/point inputs and real mouse clicks. They retain
curved Rhino inversion residuals and compare those cases at an explicitly separate
bound, without weakening the original observations or omitting numeric fields.
The later [17 snap observations](split-edge-snaps.md) retain offset feature
clicks and unsnapped controls. Fifteen model-space snap locations replay complete
geometry/history; two uncalibrated screen-only controls are explicitly unsupported.
Nonuniform curve and edge witnesses distinguish arc-length Mid from parameter Mid.
The [composite/surface snap audit](composite-feature-snaps.md) adds eleven matching
End/Mid captures, one retained uncaptured-center discrepancy and five Center
hover diagnostics. Those historical uncalibrated discrepancies remain explicit;
they are not relabeled as passing model-target or pixel replays.
The subsequent [22 calibrated Center-hover observations](center-hover-snaps.md)
record the camera at the actual point prompt. Fourteen independently computed
native captures drive complete geometry/history replay; eight empty-center misses
compare admission only. Strict optional `record_viewport`, `persistent_snaps` and
separate `pick.aim` fields keep camera input and declared model targets distinct.
Two rejected point-target hypotheses remain unchanged in the requests.
The [four two-pick one-shot records](object-snap-controls.md) add eight calibrated
captures and complete history comparisons, including restoration of persistent
modes after the first pick. Native menu/prompt lifecycle tests are separate from
these camera-calibrated core capture replays.
The [37 Mid-hover records](mid-hover-snaps.md) add 26 calibrated captures with
complete geometry/history replay and 11 mixed-mode admission-only misses. One
unsnapped control fails to split and has no Undo/Redo; that limitation is retained.
These cases cover segment/boundary hover, opposite-seam conic targets and
curve-distance ranking across competing objects and polyline segments.
The [44 polygon Center records](polygon-center-snaps.md) add 34 complete calibrated
capture/history matches and ten admission-only misses. Cases distinguish corner
averages from area/bounds centers, retain collinear/repeated curve corners,
exclude internal subdivisions of straight surface edges, and include nonplanar
curve captures alongside warped-surface misses.
The [44 circular NURBS records](circular-center-snaps.md) add 34 complete capture
replays and four admission-only misses. The
[elliptical follow-up](elliptic-center-snaps.md) resolves four more captures;
two negative-weight recognition differences remain. Both outside-arc controls
fail to split; the other 42 records include actual Undo/Redo.

The shared `serde_json` dependency explicitly enables round-trip float parsing.
Standalone oracle/document builds must not depend on app or test dependencies
to enable numerical fidelity through Cargo feature unification. A package-only
regression checks saved binary32 coordinates and 10,000 binary64 round trips.

The `surface_wires` probe compares natural surface-to-B-rep topology and wire
geometry against Rhino `CreateFromSurface`/`GetWireframe`. The
[paired surface-wire record](surface-wire-frames.md) contains 12 operations;
ten pass and two translated density-3 cases remain discrepancies. Its bounded
correspondence diagnostic requires unique nearest six-sample matches to form
a bijection, then compares unchanged coordinates and topology fields.

The `trimmed_surface_isocurves` probe accepts a `TrimmedBrepFixture` and 1–64
native `[u,v]` parameter pairs. It returns samples in face, query, U/V, segment,
point order. Both engines normalize each extracted curve's domain *after*
extraction, so differing output parameter origins do not hide geometric errors.
The [paired isocurve fixtures and recorded comparison](brep-isocurves.md)
cover local and translated paraboloid disks/annuli: local cases pass, while
large-offset cases remain explicit Rhino discrepancies.

The `mesh_face_normals` probe returns one normal per stored triangle or quad.
Its [fixture](../tools/rhino_oracle/fixtures/mesh_face_normals.json) and
[Rhino 8.32 record](../tools/rhino_oracle/observations/mesh_face_normals.json)
cover mixed face types, warped quads, cyclic/reversed winding, and a translated,
uniformly scaled copy. The native regression checks independent analytic unit
directions to `4 * f64::EPSILON`, then checks exact equality with Rhino after
conversion to its `Vector3f` face-normal storage representation. Native geometry
and probe results retain double precision; the default `1e-10` comparison
threshold is tighter than this Rhino collection's storage precision.
Extreme-scale native tests are separate from these ordinary-scale Rhino records.

The [edge-split fixture](../tools/rhino_oracle/fixtures/mesh_split_edge.json)
has a [27-case Rhino 8.32 record](../tools/rhino_oracle/observations/mesh_split_edge.json).
A native replay checks exact acceptance, face indices, vertex coordinates, and
ordering for all cases, including endpoint/outside parameters, surviving fans,
orientation conflicts, and mixed triangle/quad non-manifold incidences.
For the three-face mixed incidence, full welding produces eight vertices;
partial or no endpoint welding produces 24, with eight replacement triangles
in all three cases. These dyadic-coordinate fixtures need no comparison epsilon.

The [edge-collapse fixture](../tools/rhino_oracle/fixtures/mesh_collapse_edge.json)
has a [15-case Rhino 8.32 record](../tools/rhino_oracle/observations/mesh_collapse_edge.json).
The shared mesh-edit replay checks exact acceptance, coordinates, face indices,
and ordering. Cases include empty results, triangle/quad reduction, seams,
non-manifold edges, unused vertices, and disconnected coincident endpoint fans.
The added coincident-peer case confirms that peers outside the selected edge
move to its midpoint without merging their separate raw vertices. These ordinary
coordinate records do not establish parity for subnormal midpoint rounding.

The [edge-weld fixture](../tools/rhino_oracle/fixtures/mesh_weld_edge.json) has a
[nine-case Rhino record](../tools/rhino_oracle/observations/mesh_weld_edge.json).
It exercises the actual `WeldEdge` command with selected edge subobjects,
including reversed non-manifold face order, partial seams, empty/naked selections,
unused vertices, and disjoint seams. Replay compares acceptance, removed-vertex
counts, ordered face coordinates, and vertex-to-face sharing groups exactly.
Unlike split/collapse records, this representation does not compare raw index
numbering or identify which coincident source index survives.

The [vertex-weld fixture](../tools/rhino_oracle/fixtures/mesh_weld_vertex.json) has
a [ten-case Rhino 8.32 record](../tools/rhino_oracle/observations/mesh_weld_vertex.json).
It exercises the actual `WeldVertices` command using selected topology vertices:
both seam endpoints, reversed selection order, empty/naked/already-welded
selections, a closed fan, vertex-only contact, non-manifold incidence, and two
incident seams. It uses the same face-coordinate and sharing-group representation
as edge welding, with exact replay and the same raw-index comparison limitation.

The [vertex-unweld fixture](../tools/rhino_oracle/fixtures/mesh_unweld_vertex.json)
also has a [ten-case Rhino 8.32 record](../tools/rhino_oracle/observations/mesh_unweld_vertex.json).
It covers empty/naked/already-unwelded selections, a closed fan, duplicate and
reversed selections, cube corner/all-vertex edits, a non-manifold fan, and a
fully separated triangle. Replay checks acceptance, added-vertex counts, ordered
face coordinates, and sharing groups exactly, with the same raw-index limitation.

The [angle-unweld fixture](../tools/rhino_oracle/fixtures/mesh_unweld.json) has a
[six-case Rhino 8.32 record](../tools/rhino_oracle/observations/mesh_unweld.json).
Replay checks exact raw vertex coordinates, face indices, ordering, and added
vertex counts for zero/positive flat thresholds, equal/above right-angle
thresholds, already-unwelded faces, and cube creases. Separate non-manifold probes
exposed a [face-order-dependent mismatch](mesh-unweld-nonmanifold.md), now corrected
by radial face traversal and singleton-group ordering. All 69 non-manifold cases
are enabled parity regressions, including 24 four-face permutations and six
source-vertex reorderings; the earlier two ignored tests are enabled again.
The Rhino-only `mesh_radial_topology` probe records public edge ordering and
incidence separately; its twelve cases match the native radial sorter and isolated
the original discrepancy to subsequent grouping/rebuilding.

The [edge-unweld fixture](../tools/rhino_oracle/fixtures/mesh_unweld_edge.json) has
a [19-case Rhino 8.32 record](../tools/rhino_oracle/observations/mesh_unweld_edge.json).
This probe invokes `Mesh.UnweldEdge`, not the interactive command. Besides radial
fans and closed meshes, it covers partial sharing at one or both endpoints of a
non-manifold edge. Rhino separates an endpoint only when all incident edge faces
use one raw vertex there, preserving other existing sharing groups. Disconnected
coincident contacts and unused coincident vertices do not prevent separation.
Reversed face-order and swapped-endpoint probes confirm that partial sharing is
preserved even when the first two incident faces use different raw indices.
These cases exposed and corrected native over-separation. Replay compares
acceptance, added-vertex counts, face coordinates, and sharing groups exactly;
raw-index numbering and stored normal parity are not established by this record.

The [short-curve selection diagnostic](short-curve-selection-measurement.json)
embeds four requests and responses (40 line lengths). `short_curve_selection`
creates owned line objects, runs the actual `SelShortCrv` command, and records
selected source indices, restoring the original selection and deleting only
its temporary lines. This is a Rhino-only probe; command tests replay its
measurements. The measured relative `1e-6` allowance is more specific than the
[help page's “less than” description](https://docs.mcneel.com/rhino/8/help/en-us/commands/selection_commands.htm#SelShortCrv).

The [circle follow-up](short-curve-circle-measurement.json) records 15 analytic
circles and 15 rational NURBS circles. `curve_kind` explicitly chooses the source
representation; optional `inspect` records `GetLength`, `IsShort(limit)`, and
`IsShort(limit × 1.000001)` without changing the command under test.
All analytic and NURBS cases now replay successfully. The original mismatch was:
for nominal length `0.999999`, Rhino reports length `0.9999990022994623`, yet
both shortness queries return false and the command leaves it unselected.
The previous native length-based predicate selected it. A separate
[adaptive shortness predicate](curve-shortness.md) now reproduces the observed
selection without changing NURBS geometry or accurate length measurement.
Rhino documents [IsShort](https://mcneel.github.io/rhinocommon-api-docs/api/RhinoCommon/html/M_Rhino_Geometry_Curve_IsShort.htm)
as a faster alternative to calculating length.

The [representation follow-up](short-curve-representation-measurement.json)
adds 66 classifications: midpoint knot refinement at levels 0, 1, 2, and 4;
degree elevation from 2 through 5; and a polynomial quadratic arch with exact
nominal length derived from its parabola. These preserve the underlying locus
while changing the spans or degree. One refinement changes the circle's
near-limit selection, whereas degree elevation alone does not. A fixed
three-point Gauss–Legendre estimate explains the coarse-circle rejection but
fails on the arch, where it underestimates length and would accept cases that
Rhino rejects. It is therefore not implemented as a replacement predicate.
The records also include control counts and polygon lengths, separately from
Rhino's integrated lengths and shortness results. Adaptive three-point
integration with early rejection, rather than a single fixed estimate,
now passes all 66 classifications. Broader curve families, extreme shortness
thresholds, and polycurve-specific boundary behavior still need oracle coverage.

The versioned Python oracle API runs identical JSON geometry and document-state
batches in a native release build of Viboceros and Rhino 8, recursively checks
results, and reports per-operation timings.

`control_point_prompt_rhino_only.json` is a diagnostic exception: it exercises
Rhino's Curve point prompt and records resulting control points, rather than
running the geometry constructor. Run it with `run_headless.sh rhino
tools/rhino_oracle/fixtures/control_point_prompt_rhino_only.json --timeout 300`;
the native oracle does not implement this operation. Its [recorded response](control-point-prompt-measurement.json)
checks adjacent-point rejection; see [typed point input](point-input.md).
The [threshold follow-up](control-point-threshold-measurement.json) embeds three
additional requests and their responses (30 cases), establishing a fixed,
coordinate-wise `2^-32` comparison for the tested near-origin controls.
The app test `recorded_rhino_curve_threshold_sequences_match_interactive_completion`
replays all 30 stored requests with their document tolerances and checks accepted
controls, output degree, closure, and control locations against Rhino's responses.
JSON uses round-trip float parsing so adjacent binary64 boundary values remain distinct.

`interpolation_point_prompt_rhino_only.json` is a separate eight-case diagnostic
using the same coordinate whitelist and private construction-plane wrapper for
InterpCrv (open cubic, chord knots). Explicit command rejections become
`command_succeeded: false` records; unexpected failures still fail the batch.
Its [recorded response](interpolation-point-prompt-measurement.json) exposed an
open-curve point-tolerance discrepancy that is now corrected. An app test replays
the six successful cases and compares control points within `1e-9`; two Rhino
solver rejections remain diagnostic. Run it with `run_headless.sh rhino`, not
the native comparison runner.

`interpolation_closure_prompt_rhino_only.json` checks ordinary three-point
closure using `_Close` and `_Sharp _Close`, in that order, in a fresh private
Rhino session. The [recorded response](interpolation-closure-prompt-measurement.json)
replays through app completion and matches both sets of control points within
`1e-9`. This is a baseline, not a near-seam tolerance measurement or a test of
arbitrary persistent option state. In this Rhino build, `_Sharp` alone toggles
the option and leaves the point prompt active. Exploratory `_Sharp=_No _Close`
and `_Sharp=_Yes _Close` both produced the sharp curve; those spellings are not
used by the retained fixture. Closed prompt probes are restricted to InterpCrv.

The [closure tolerance follow-up](interpolation-closure-tolerance-measurement.json)
embeds two requests and their responses: a `0.001` interior or seam-adjacent
offset, smooth or sharp closure, at absolute tolerances `0.01` and `1e-9`.
Both batches use the same operation order in fresh private Rhino sessions.
All eight results replay through app completion with control-point error at
most `1e-9`. Command interpolation now retains these distinct points instead
of rejecting or merging them using model tolerance. This does not measure
the exact duplicate/near-zero seam threshold or automatic closing behavior.
The separate [auto-close audit](interpolation-auto-close-measurement.json)
embeds four requests and responses (18 cases) that establish an inclusive
Euclidean `1.490116119385e-8` prompt-closing threshold near the world origin.
The `PointOnly` diagnostic ending sends no Enter or closure option: its exact
seam case confirms completion from the point itself. Use that ending only to
probe suspected automatic completion; non-closing input may wait until the
client timeout. App regressions replay every measured geometry and distinguish
automatic completion from the cases that still require Enter.

The [translated follow-up](interpolation-translated-auto-close-measurement.json)
records `periodic` separately from `closed` for 13 cases. At X=`±1e6`, curves
with endpoint gaps of `2e-8` are closed under Rhino's coordinate-relative
topology test but are not periodic; a two-point return also produces a closed,
non-periodic result. App replay must not infer periodicity from closed state.
New InterpCrv prompt measurements include both properties; older responses
still check their recorded control geometry without inventing periodicity data.
A [two-point no-Enter probe](interpolation-two-point-auto-close-measurement.json)
confirms automatic completion with non-periodic output. Consequently neither
`closed` nor `periodic` alone establishes when the point prompt finished; the
`PointOnly` ending directly tests completion. The app now auto-closes two-point
returns with a non-periodic cubic instead of requiring a third collected point.

The [degree-one follow-up](interpolation-degree-one-auto-close-measurement.json)
records six Enter-completed sequences, all replayed through preview/completion
with control errors at most `1e-9`. Separate `PointOnly` probes for two and three
collected points followed by `w1e-8,0,0` both waited until the 100-second client
timeout. The owned private windows were inspected while live and still showed
the next-point prompt after accepting that input; these are prompt observations,
not successful geometry responses. Separate [exact-return probes](interpolation-degree-one-exact-close-measurement.json)
do complete without Enter for both point counts. Degree-one completion at a
nearby endpoint therefore differs from completion at exact equality, even when
its eventual endpoint is reconciled to the start.
A further no-Enter batch timed out during its first `1e-10` offset; its subsequent
`2^-32` case was not reached and must not be treated as measured.

The [degree-one reconciliation boundary audit](interpolation-degree-one-seam-boundary-measurement.json)
contains nine successful Enter-completed cases: equality and the next float
around `1.490116119385e-8`, a longer diagonal, two model tolerances, and three
translated endpoints at X=`1e6`. It confirms the current distance-based
reconciliation rule within control-point error `1e-9`. Replay also compares
the cached preview to completion for every still-active open prompt; this
does not establish a tighter automatic-completion threshold.

Standard geometry/command batches apply the request's absolute, relative, and angular tolerances to
Rhino's active document and restores its previous settings on success or failure.
This matters for command macros, which read document settings rather than an API
tolerance argument. See [Rhino's document tolerance API](https://developer.rhino3d.com/api/rhinocommon/rhino.rhinodoc/modelabsolutetolerance).
Older command comparisons made before this synchronization need revalidation.

`mesh_split_picking.json` uses the owned-window idle-click mechanism with three
disjoint meshes, then invokes the actual `SplitDisjointMesh` command. Sixteen
cases cover ordinary and overlapping groups, ordered bridge memberships,
hidden/locked objects, hidden/locked layers, and connected meshes mixed with
splittable peers. The native oracle builds the same documents and runs the
registered command. All 16 cases replay against the
[saved observations](../tools/rhino_oracle/observations/mesh_split_picking.json).
Run a read-only replay with:

```sh
python3 -m tools.rhino_oracle replay tools/rhino_oracle/fixtures/mesh_split_picking.json --observations tools/rhino_oracle/observations/mesh_split_picking.json
```

The checked-in `tools/rhino_oracle/observations/mesh_split_picking.json` records
Rhino 8.32.26160.13001 on 2026-09-12: source identity, selection, object mode,
group memberships, layer modes, vertices and face counts, including untouched peers.
A native command regression also compares those 16 recorded output sets exactly;
see the [verified fields and remaining limits](commands/meshes.md).
Deleted sources are removed from cleanup tracking; surviving original and newly
created mesh IDs are explicitly tracked and cleaned up in the private document.

`mesh_explode_picking.json` uses the same 16 mesh setups but invokes `Explode`.
All 16 cases also replay against its
[saved observations](../tools/rhino_oracle/observations/mesh_explode_picking.json).
Its observations record retained restricted decomposed sources as
**unselected**, unlike SplitDisjointMesh.
Connected sources remain selected in both commands. Python tests compare their
output records, allowing only the command success field name and decomposed
original-source selection difference. One mixed locked-connected case per
command also records selection and identity retention after actual Undo/Redo.
Those history selections are checked separately, including the unchanged peer.
The shared native test adapter in `mesh_decomposition_tests.rs` compares both
commands against their respective recorded output sets; run
`cargo test -p viboceros-command mesh_decomposition_tests`.
See the [verified native Explode behavior and limits](commands/editing.md).

`group_picking.json` is a dedicated, untimed three-line fixture: an idle-event
worker returns control to Rhino's normal UI loop, then the host clicks projected
line locations in the newly owned window and acknowledges each click atomically.
No Enter or selection command substitutes for a mouse pick. Its 52 exact checks
cover ordered group selection, hidden/locked peers and layers, and subsequent
Move commands. Run it as a separate batch with `run_headless.sh` (one iteration);
it cannot be mixed with synchronous geometry operations. See [groups](groups.md).
The idle worker guards against event-loop re-entry and finalizes only once.
Cleanup attempts every owned resource even after a failure, publishes cleanup
errors, and leaves process termination to the owned-window client fallback if
Rhino cannot exit. Fault-injection tests cover callback detachment, setup,
disposable attributes/layers, cleanup, logging, and exit failures.

`selection_recall.json` checks 84 SelPrev sequences with explicit/remembered
options, changed groups, and deleted objects. `selection_recall_picking.json`
uses the dedicated idle worker for 52 recalls after real group picking. These
136 exact comparisons distinguish recall from ordinary group expansion and
verify visibility/lock filtering. See [selection recall](selection-recall.md).
`last_selection.json` adds 32 exact SelLast sequences after idle Move, checking
group/mode filtering, remembered options, and empty-layer creation. The separate
16-case `last_selection_history.json` and 54-case `deletion_recall.json` now
match complete traces, including immediate Undo/Redo selection. These use the
same owned-window idle worker. `undo_selection.json` adds 10 isolated command
traces for Move, Delete, Explode, DeleteFaces, and ExtractMeshFaces; it checks
whole-object selection, source existence, object/output counts, and mesh face
counts after commands. See [history selection](history-selection.md).
Repeated picks at identical coordinates do not wait for a new mouse-motion event;
input failures report the case/window and progress without acknowledging a click.

`point_input.json` compares 19 [typed-coordinate sequences](point-input.md)
against Rhino's actual Polyline prompt in world, Front, Right, shifted, and
oblique construction planes. It checks unrounded resulting vertices, not a
second copy of the parser. The probe restores the plane and selection and deletes
only its own output objects; command execution is untimed.
`point_input_diagnostics.json` retains a large-scale exact-quadrant discrepancy,
not a passing Rhino reference at `1e-12`.

`plane_primitives.json` checks 57 Circle/Polygon/Rectangle/MeshPlane/Box/MeshBox
command results on construction planes. Complete boundary geometry is checked
independently of Box/MeshBox layout; ten raw layout probes remain in
`plane_primitives_representation.json`. See [comparison details](construction-planes.md).
The [two-point circle fixture](../tools/rhino_oracle/fixtures/circle_two_point.json)
checks six actual `_Circle _2Point` results against
[Rhino 8 observations](circle-two-point-rhino-reference.json), including
reversed diameter endpoints, oblique diameter, and tilted CPlane.
The [Arc Center angle fixture](../tools/rhino_oracle/fixtures/arc_center_angle.json)
compares six actual `_Arc _Center` commands with
[Rhino 8 sampled curves](arc-center-angle-rhino-reference.json), including a
major arc, capped sweep, negative sweep, and rotated construction plane. Native
samples and domains agree within `1e-10`.
The [bare Arc fixture](../tools/rhino_oracle/fixtures/arc_default.json) confirms
Rhino's center-first default through three typed angles and two actual endpoint
clicks. The [StartPoint ThroughPoint fixture](../tools/rhino_oracle/fixtures/arc_start_through_point.json)
checks the explicit three-point path, including a major and tilted arc.
[Default](arc-default-rhino-reference.json) and
[through-point](arc-start-through-point-rhino-reference.json) sampled curves and
domains match native results within `1e-10` when endpoint sweep is explicit.
The [Arc Center Length fixture](../tools/rhino_oracle/fixtures/arc_center_length.json)
compares five signed-length `_Arc _Center` commands against
[live Rhino 8 results](arc-center-length-rhino-reference.json), including a
rotated construction plane and both signs of length beyond one circumference.
Native curve samples and domains agree within `1e-10`.
The [Arc Center endpoint fixture](../tools/rhino_oracle/fixtures/arc_center_endpoint.json)
automates eight actual Rhino viewport clicks at Point osnaps with a public
`_Arc _Center` prompt and compares their complete sampled curves and domains
to explicitly directed native arcs within `1e-10`, including a construction
plane with its normal reversed. The
[single south pick](../tools/rhino_oracle/fixtures/arc_center_endpoint_south_alone.json)
produces a clockwise 90° arc, while the same pick after another operation in
the batch produces a counterclockwise 270° arc. Their
[batch](arc-center-endpoint-rhino-reference.json) and
[single-pick](arc-center-endpoint-south-alone-rhino-reference.json) records
show that interaction history affects the sweep; cursor travel appears to be
the deciding factor.
The [Arc Center Midpoint fixture](../tools/rhino_oracle/fixtures/arc_midpoint.json)
compares 17 typed angle/length commands and actual endpoint clicks against
[Rhino 8 sampled curves](arc-midpoint-rhino-reference.json). It covers
positive and negative sweeps, the 360° boundary, off-radius endpoint picks,
and a reversed construction-plane normal. Native curves and domains agree
within `1e-10` for explicitly directed endpoint picks. A separate same-radial
endpoint probe was rejected by Rhino and is rejected by the native command.
The [Arc StartPoint Direction fixture](../tools/rhino_oracle/fixtures/arc_start_direction.json)
compares five typed Rhino direction constructions with
[live sampled curves](arc-start-direction-rhino-reference.json): both turn
directions, a major arc, an oriented construction plane, and an off-plane
endpoint. Native samples and domains agree within `1e-10`.
The [Arc StartPoint Center fixture](../tools/rhino_oracle/fixtures/arc_start_center.json)
compares ten actual Rhino commands and viewport endpoint picks with
[live sampled curves](arc-start-center-rhino-reference.json), including signed
angles and lengths, major and quarter arcs, and reversed construction-plane
normal. Native curves and domains agree within `1e-10` when endpoint sweeps
are made explicit.
The [three-point circle fixture](../tools/rhino_oracle/fixtures/circle_three_point.json)
checks four actual `_Circle _3Point` results against
[Rhino 8 observations](circle-three-point-rhino-reference.json), including
reversed point order and off-CPlane geometry. Complete sampled curve records
and domains agree within `1e-10`; command execution is untimed.
The [three-point Radius fixture](../tools/rhino_oracle/fixtures/circle_three_point_radius.json)
checks four numeric-radius orientations against
[Rhino 8 records](circle-three-point-radius-rhino-reference.json). The
[picked Radius fixture](../tools/rhino_oracle/fixtures/circle_three_point_radius_picks.json)
checks on-plane and off-plane radius locations against
[live results](circle-three-point-radius-picks-rhino-reference.json).
The [circle size fixture](../tools/rhino_oracle/fixtures/circle_size_options.json)
checks Diameter, Circumference, and Area against
[Rhino 8 numeric results](circle-size-options-rhino-reference.json). The
[picked size fixture](../tools/rhino_oracle/fixtures/circle_size_picks.json)
checks Rhino's distinct point behavior for the same three options against
[live results](circle-size-picks-rhino-reference.json).
The [Vertical fixture](../tools/rhino_oracle/fixtures/circle_vertical.json)
checks world, off-plane, and rotated-CPlane direction picks against
[Rhino records](circle-vertical-rhino-reference.json). A separate
[numeric-radius fixture](../tools/rhino_oracle/fixtures/circle_vertical_numeric.json)
uses a direction point beyond the fixed radius and matches its
[Rhino result](circle-vertical-numeric-rhino-reference.json).
The [Orientation fixture](../tools/rhino_oracle/fixtures/circle_orientation.json)
checks five normal directions and construction planes against
[Rhino records](circle-orientation-rhino-reference.json). The
[radius-point fixture](../tools/rhino_oracle/fixtures/circle_orientation_picks.json)
checks on-plane and projected off-plane picks against
[live results](circle-orientation-picks-rhino-reference.json). The
[picked-size fixture](../tools/rhino_oracle/fixtures/circle_orientation_size_picks.json)
checks off-plane Diameter, Circumference, and Area input against
[live results](circle-orientation-size-picks-rhino-reference.json).

`plane_transforms.json` checks 140 actual transform commands using four affinely
independent point witnesses per operation, including copied/original identity
and selection. Four parallel-reference Shear cases remain explicit diagnostics
in `plane_transform_diagnostics.json`; see [plane transforms](plane-transforms.md).

`interface_commands.json` checks 208 untimed display/snapping transitions across
eight initial states against Rhino's actual commands. The native probe shares
the GUI's parser and state reducer. The worker restores application settings
and all viewport modes; see [interface controls](commands/interface.md).
`drafting_aids.json` adds 22 live Ortho, Planar, CPlane Z, and Ortho angle
transitions. Its [retained Rhino response](../tools/rhino_oracle/observations/drafting_aids.json)
replays against the native reducer with a maximum angular difference of
`7.1e-15` degrees. RhinoCommon reports the angle in radians; the probe records
degrees. Cursor positions still need a separate live comparison.

`construction_planes.json` checks 129 actual plane edits/history transitions.
`construction_plane_input.json` checks 24 transparent CPlane commands inside
Polyline prompts, including local and relative coordinate continuation.
See [construction-plane editing](cplane.md) for accuracy and remaining options.

`plane_arrays.json` checks 64 actual rectangular/linear/polar array commands,
including CPlane axes, tight curve extents, signed Fill lengths, zero-spacing
cells, selection, and groups. Initial scripted 3D Fill-height discrepancies
remain in `plane_array_diagnostics.json`; see [plane arrays](plane-arrays.md).
`curve_bounds.json` checks 16 timed tight-curve-box queries. A negative-gauge
Rhino discrepancy remains in `curve_bounds_diagnostics.json`, separate from
passing references; see [bounds policy and measurements](curve-bounds.md).
`surface_bounds.json` adds 25 passing timed surface-box queries.
`surface_bounds_diagnostics.json` retains four inaccurate Rhino boxes, with
untimed 41×41 `sample_grid` witnesses recorded separately as `sample_bounds`.
`surface_array_bounds.json` compares 32 actual surface/mixed-source arrays;
24 pass and eight retain placement discrepancies at `1e-8`.
See [surface bounds](surface-bounds.md) for coverage and limitations.
`parameter_curve_bounds.json` compares 20 exact surface images against
independently supplied spatial reference curves; a signed-rational Rhino box
discrepancy remains in `parameter_curve_bounds_diagnostics.json`.
`trim_boundary_bounds.json` adds six exact B-rep boundary comparisons.
These untimed probes retain direct surface-image samples and do not imply
complete trimmed-face bounds; see [trim-boundary bounds](trim-boundary-bounds.md).
`trimmed_brep_bounds.json` separately compares 11 complete B-rep boxes, with
five retained box/trim-evaluation diagnostics in
`trimmed_brep_bounds_diagnostics.json`; see [trimmed-face bounds](trimmed-face-bounds.md).
`trimmed_brep_array_bounds.json` exercises 32 actual arrays of single- and
multi-face trimmed B-reps, recording every face domain and trim-image samples.
`bounding_box.json` checks 57 actual BoundingBox commands, including output
corners/topology, groups, reports, and World/CPlane orientation.
`bounding_box_mixed.json` compares six mixed valid/point selections with actual
Rhino partial output and failure reporting; all six match. The remaining 20 of
the 26 `bounding_box_diagnostics.json` cases retain curved-bound and thin-geometry
differences. See [bounding boxes](commands/bounding-box.md), including
why raw Rhino script success flags are not used for report-only commands.
`distribute.json` adds 188 actual distribution commands with signed spacing,
World/CPlane directions, bound tie ordering, retained source identities, and
top-group membership rules. Six analytically checked curved-bound placement
differences remain in `distribute_diagnostics.json`; see
[Distribute](commands/distribute.md). Its batch preselection guard avoids
leaving Rhino in an interactive object-selection prompt.

`group_memberships.json` contains 56 comparisons of ordered object memberships and the reverse
group-member table after every fixture step. It exercises membership edits,
deletion, nested/partial `Ungroup` and `UngroupAll`, copied-group allocation,
decomposition, and older-group distribution. IDs are compared as retained-source
flags; domains, sampled geometry, and selection are checked separately.
Only automatic `Group`-plus-digits names are canonicalized, by numeric order:
the reused Rhino document reserves those names across cleaned-up operations.
Explicit names and incorrectly styled copy names remain literal. This does not
establish absolute deleted-name allocation history; see [groups](groups.md).
The former ConvertToBeziers diagnostic now passes and is part of the regular
group fixture; ordinary Delete also retains empty definitions. The separate
`bezier_conversion.json` compares complete output NURBS control nets, source
identity, domains, sampled geometry, output creation order, current/source layers,
colors, names, groups, and selection after explicit deletion choices; see
[Bézier conversion](commands/beziers.md). A separate point-cloud-cycle probe uses
the same preselection workflow as the native command; the previous in-command
SelID workflow did not.

`single_span_conversion.json` adds 78 actual surface conversions covering
directional strips, Both patches, no-ops, trims, unclamped/periodic structure,
mixed sources, and U/V toggles. `conversion_sessions.json` adds eight self-seeded
sequences (39 steps) checking independent remembered deletion/direction choices,
partial option changes, no-op acceptance, and undo. They share the full conversion
recorder and owned-object cleanup. See [single spans](commands/single-spans.md)
for the explicit omitted-outer-knot codec and [option memory](command-options.md)
for lifetime and bootstrap limitations. These command probes are untimed.

`nurbs_conversion.json` compares 78 actual `ToNURBS` conversions, including meshes
and already-NURBS no-ops. It additionally records curve representation kind and
full source curve/surface definitions so identity-preserving replacement cannot look like a
no-op. B-reps include topology and full underlying surface nets. Six
`nurbs_conversion_sessions.json` sequences add 39 steps; unlike single-span
conversion, `ToNURBS` no-ops do not accept new choices. See
[ToNURBS](commands/to-nurbs.md) for parameters, chronological renewal, and limits.

`mesh_nurbs_conversion.json` adds 50 actual preselected `MeshToNURB` comparisons,
distinct from the older `mesh_to_nurb.json` geometry-API probes. Three
`mesh_nurbs_conversion_sessions.json` sequences add 21 option-memory steps.
Requested Rhino options are first seeded on a separate owned temporary mesh
because preselection hides the command's option prompt. Results include full
B-rep topology and surface nets, source/output metadata, selection, group tables,
and creation order; no result selection is normalized.
`mesh_nurbs_postselection.json` adds 60 prompted-selection cases; three
`mesh_nurbs_postselection_sessions.json` sequences add 18 steps. `postselect`
uses the actual option/picking prompt with ordered `SelID` inputs; `cancel`
escapes it, including an empty picked list. Optional `initial_selection` contains
only non-mesh sources, so it cannot accidentally trigger preselected conversion.
Cancellation cannot request undo. Full source/output records capture cleared
selection, pick-dependent creation order, and choices retained after cancellation.
See
[MeshToNURB](commands/mesh-to-nurb.md) for coverage and selection-path limits.

`nurbs_postselection.json` adds 87 cases and three
`nurbs_postselection_sessions.json` sequences add 31 steps. ToNURBS macros complete
ordered object selection before entering the separate conversion options and nested
MeshOptions prompt. `cancel` can cancel preselected or prompted confirmation;
`cancel_at_selection` additionally requires prompted cancellation and permits an
empty picked list. No-op input cannot reach confirmation, and cancelled commands
cannot seed option memory. A separate script builder is tested for precise phase
ordering. Native probes exercise explicit ordered copies/renewal and preserve
complete source/output records without normalizing selection or creation order.

`bezier_postselection.json` adds 61 cases and two
`bezier_postselection_sessions.json` sequences add 24 steps. The macro completes
ordered selection before answering the separate Yes/No deletion question; the
answer finishes the command without an extra Enter. Cancellation at selection
or at the deletion question accepts no choice. Initial selection must be
ineligible, and cancelled commands cannot seed a deterministic session. The
same complete conversion records check source retention/deletion, fresh output
attributes, empty groups, cleared picks, and pick-dependent creation order.

`single_span_postselection.json` adds 108 cases and three
`single_span_postselection_sessions.json` sequences add 34 steps. Macros finish
selection before entering the options stage and its Direction chooser. Cancelled
options are remembered, but selection-stage presets are not. This includes
toggle-only follow-ups and no-op input. A cancelled options stage can seed a
session; cancelled selection cannot. Disjoint grouped subset probes avoid
confounding group behavior with overlapping-surface hit ambiguity.

With Rhino installed through the configured Wine/FEX launcher, run the core fixture:

```sh
python3 -m tools.rhino_oracle compare \
  tools/rhino_oracle/fixtures/core.json \
  --absolute-epsilon 2e-12 --relative-epsilon 1e-12
```

For trimmed surfaces with non-affine parameterization:

```sh
tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/surface_split_nonaffine_trimmed.json \
  --absolute-epsilon 1e-9 --relative-epsilon 1e-10
```

That fixture sets `sample_trim_geometry=true`: each non-isoparametric trim is
compared at 65 equal-arc-length stations in UV, including both endpoints, with
both UV coordinates and surface-evaluated 3D positions recorded. This compares
independently fitted curves whose knot counts and parameter speeds differ.
Topology, edge domains, underlying surfaces, attributes, groups, and selection
are still compared. The default retains complete trim control/knot comparisons.
Sampling is bounded evidence of geometric agreement, not a continuous error proof.

The `trimmed_mass_properties.json` fixture checks [nonplanar trimmed-face area
and signed volume](mass-properties.md). Its `trimmed_surface_mass_properties`
operation supplies exact spatial and UV loop curves, an underlying surface, and
an optional cap surface sharing those boundaries. Rhino's public B-rep topology
API builds the same input geometry as the native probe, avoiding changes to the
input from a separate trim-fitting operation. Numerical API calls are timed;
the native probe also checks `Area` and `Volume` command results and document state.
The [35 trim-domain frame cases](mass-properties.md#trim-domain-regression-audit)
vary trim parameters without changing UV coordinates or spatial edges. Their
records include complete trim definitions, extracted outside the timed section,
so implicit input normalization cannot masquerade as large-domain coverage.

The `curve_area.json` fixture checks eight [enclosed curve-area cases](curve-area.md)
against analytic references and Rhino's public API. The retained strict comparison
has two conic discrepancies; it is not an all-passing parity claim.

The `non_manifold_selection.json` fixture checks the actual
[`SelNonManifold` command](non-manifold-selection.md) on mesh and B-rep objects,
with separate unselected controls and additive-selection cases.

The `polycurve.json` fixture exercises [exact piecewise curves](polycurves.md).
It preserves and compares segment definitions and domains, then tests reversal,
trimming, splitting, length-based reparameterization, derivatives, and division.
The `curve_division_contract.json` fixture checks open/closed division endpoint
rules separately, including the `include_ends=false` case.
`polycurve_document.json` exercises actual extraction and explode commands,
duplicate comparison, and 3DM round-trip segment definitions; the native side also
checks undo. See [polycurve documentation](polycurves.md) for compatibility boundaries.

`edge_surface.json` and `edge_surface_command.json` compare [Coons edge surfaces](edge-surfaces.md)
through both the geometry API and document command, including full NURBS
coefficients, direct samples and singular-side topology. The zero-weight-control
case uses exact comparison-degree elevation while retaining original-surface
samples. `edge_surface_3dm_interchange.json` separately checks both readers of the
same native-written files, without changing the serialized basis.

`point_grid.json` checks [point-grid surface construction](point-grid-surfaces.md),
including native domains, full coefficients, active samples, and actual
construction-site samples (including closed-direction continuation).
`point_grid_command.json` checks every output face and retained point-cloud
state. Higher degrees and same-file 3DM checks have separate fixtures.
`point_grid_high_degree_diagnostics.json` deliberately retains ill-conditioned
closed-grid residuals and a folded-shell orientation mismatch; it is not a
passing full-record reference. See the point-grid page for limits and measured
differences.

`polycurve_native.json` checks native analytic evaluation and segment classes
through reversal, trim/split, and transforms. `polycurve_analytic_editing.json`
checks analytic endpoint policy inside composites. These fixtures include both
native derivative samples and NURBS definitions: rational conversion alone cannot
verify angular parameterization. Exact nonuniform transforms explicitly prepare
Rhino's polycurve with `MakeDeformable`; direct API shear has different behavior.
`polycurve_native_document.json` compares extraction, recursive Explode, duplicate
checks, and 3DM round trips on native mixed composites in both directions.

`curve_native_parameters.json` covers the shared [native parameter contract](curve-parameters.md)
for all seven curve families. It compares domains, first/second derivatives,
tangents, equal-length division parameters, reversal, transforms, and NURBS definitions.
`curve_native_editing.json` extends those records with native trims, cyclic subcurves,
splits, seam relocation, periodic curves, and unequal rational weights. See
[domain-editing validation](curve-domain-editing.md) for the separate geometry and
Rhino length-inversion comparison limits.

`nurbs_rational_jets.json` adds 23 degree-one and higher rational derivative,
weight-scale, and seam comparisons. `nurbs_translated_jets.json` is a separate
diagnostic exposing Rhino's large-coordinate cancellation, not a passing reference
at the ordinary epsilon. Its `curve_native` operations set `differential_only=true`
to omit length/division calls, isolating derivative evaluation from Rhino's failed
translated length inversion. See [rational numerical validation](nurbs-numerics.md)
for analytic checks, observed errors, and remaining kernel limits.

`curve_sided_evaluation.json` compares 45 native left/right limit cases via
`sided_parameters`, including composite child knots and stationary endpoints.
See [one-sided evaluation](curve-sided-evaluation.md) for the exact-limit contract,
Rhino probe construction, and native-only full-order knot tests.

`curve_frames.json` and `curve_frames_multispan.json` compare 22 sets of
[rotation-minimizing frames](curve-frames.md), keeping points/tangents separate
from the relative-rotation accuracy checks. `curve_frames_diagnostics.json`
isolates seven known Rhino availability, large-coordinate, and corner-query
differences; it is not a passing reference. `curve_array.json` now records
unrounded endpoints. `curve_array_corner_diagnostics.json` is a separate,
untimed actual-command probe exposing Rhino's query-dependent corner twist.

`sweep1.json`, `sweep1_command.json`, `sweep1_multisection.json`, and
`sweep1_weights.json` check 49 [one-rail sweep cases](sweep1.md)
through Rhino's public refitted API and actual unrefitted/refitted commands.
Each compares 135 unrounded closest-point results, including 54 off-surface
queries. Command probes compare output geometry, not full document state, and
are untimed. The actual-command macro uses Rhino's script options
`Style`, `ShapeBlending`, and `RefitRail`, not dialog property labels. It rejects
new `Unknown command` history even if `RunScript` reports success.
`sweep1_curved_blend.json`, `sweep1_diagnostics.json`,
`sweep1_basis_diagnostics.json`, and `sweep1_weights_diagnostics.json` retain
13 discrepancies; these are not passing references at `1e-6`.
Some are construction differences, others closest-point
differences, including two independently verified nonminimal Rhino answers.
Weight probes distinguish raw relative scales, Euclidean versus homogeneous
placement, and normalization after placement. Signed-control sweeps additionally
check a sufficient positive-denominator bound; no comparison epsilon was widened.

All `loft.json` and `loft_command.json` cases now enable `sample_geometry`:
289 unrounded normalized-UV grid points supplement each output face's full
coefficients. `loft_end_weights.json` adds two normalization checks;
`loft_end_weights_diagnostics.json` retains three public Rhino endpoint-drift
cases, not passing references at absolute `1e-9`, relative `1e-12`. Native
analytic checks preserve the original ruled profiles. See [Loft](loft.md) and
[endpoint-weight numerics](nurbs-numerics.md#endpoint-weight-normalization).

`surface_jets.json` checks 20 [rational surface differential cases](surface-evaluation.md),
including second partials, parameter-domain changes, continuation, and exact
quadrant limits. `surface_translated_jets.json` separately diagnoses large-coordinate
cancellation and is not a passing reference at the ordinary epsilon.

`surface_curvature.json` checks 27 [surface-curvature API cases](curvature.md),
including one-sided knot limits and singular-pole availability. Spatial shape
operators avoid arbitrary principal-direction signs and umbilic axes.
`surface_curvature_umbilic.json` isolates Rhino's roughly `7.45e-9` repeated
eigenvalue split on a sphere and uses its own `1e-8` absolute comparison limit.
`curvature_command.json` checks 17 actual command cases, including permanent
markers, source geometry and attributes, and retained selection. These command
records verify that a measurement was reported; the API records separately
check unrounded numerical results.

`curve_surface_morph.json` separates [direct point-map and curve-fit validation](curve-morphing.md).
Its eight direct maps match at absolute `2e-12`; fitted outputs use `1e-5` because
Rhino's observed fit error exceeds the requested `1e-9`. Native fits are separately
asserted against their direct maps at the requested tolerance. `surface_orient.json`
now compares unrounded curve samples and document state, not rounded control nets.
`surface_surface_morph.json` adds five [surface-fitting cases](surface-morphing.md)
with 1,089 offset-grid direct-map and fitted samples per case; native fit errors
are independently checked at each case's requested tolerance.
`brep_surface_morph.json` adds four [trimmed B-rep morph cases](brep-morphing.md),
checking shared topology, direct maps, and fitted geometry separately. Edge
samples use closest-point correspondence because Rhino changes their parameter
speeds; native fits additionally retain original-parameter accuracy checks.
`brep_mesh_boundaries.json` checks [open and closed shared-boundary meshing](brep-meshing.md)
on five independently refined box-face subsets. It compares boundary geometry
and incidence properties, not identical mesh element counts, and separately
validates native `Mesh` command results.

`curve_3dm_interchange.json` checks Rhino's reading of actual Viboceros-written
files, including decomposed full-order knots and all visibility/locking states.
Unlike independent-construction probes, `compare` supplies shared private artifact
paths and cleans them after both readers finish. See [curve interchange](curve-3dm-interchange.md).
`brep_3dm_interchange.json` uses the same shared-artifact workflow for eight
[nonlinearly morphed B-reps](brep-3dm-interchange.md), comparing complete NURBS
definitions, topology, tolerances, geometry samples and post-import mesh topology.
`rational_3dm_range.json` checks four extreme-coordinate/weight cases using zero
absolute epsilon and relative `1e-12`; see [numeric range validation](rational-3dm-range.md).

`curve_parameter_map.json` adds both native/rational parameter maps to those records.
`curve_native_cutting.json` tests cutting-object Split and Trim on all curve families,
including wrapped outputs, seam hits, and projected cuts. `curve_native_extrusion.json`
checks profile-domain preservation. See [native cutting validation](curve-cutting.md)
for the 75 cases, numeric limits, and the separate ill-conditioned legacy Trim
tangent comparison. Curve-cut command records sort by native domain, not rounded
world-space endpoint coordinates.

`curve_join_close.json` compares 39 mixed joining and closure cases, including
full NURBS definitions, retained intervals, representation, and length. It tests
the batch `JoinCurves` API separately from actual `Join`/`CloseCrv` commands.
Join command records also compare source identity, names, and overlapping groups;
document results are sorted by their unique source names. The native command
path executes the real command registry. See [curve editing](curve-editing.md).

The [curved face Intersect fixture](../tools/rhino_oracle/fixtures/curved_brep_face_intersect_command.json)
compares three live cylinder/plane surface-B-rep and B-rep/B-rep commands. All
three [saved observations](../tools/rhino_oracle/observations/curved_brep_face_intersect_command.json)
match after canonicalizing the direction of straight intersection curves;
Rhino chooses branch directions from input order. The
[circle section fixture](../tools/rhino_oracle/fixtures/curved_brep_face_circle_intersect_command.json)
adds fourteen matching [Rhino observations](../tools/rhino_oracle/observations/curved_brep_face_circle_intersect_command.json)
for surface order, reversed selection, plane normals, rotated axes, and section
heights. Full cylinder sections now use Rhino's signed `2π` domain and winding.
The [closed cylinder fixture](../tools/rhino_oracle/fixtures/cylinder_brep_intersect_command.json)
and [saved observations](../tools/rhino_oracle/observations/cylinder_brep_intersect_command.json)
also probe a full-domain curved wall inside a three-face solid. Rhino gives its
closed circular section a `4π` domain. The
[steep and oblique section fixture](../tools/rhino_oracle/fixtures/curved_cylinder_brep_intersect_command.json)
has three more [saved Rhino results](../tools/rhino_oracle/observations/curved_cylinder_brep_intersect_command.json).
The kernel joins exact rational wall arcs to cap segments for a steep cut and
keeps an untrimmed oblique cut as an exact rational ellipse. Rhino's Intersect
command returns fitted cubic curves for these cuts, with sampled boundary
deviation up to `3.5e-6`; the native curves' sampled deviation is below `1e-14`.
The [section geometry audit](../tools/rhino_oracle/audit_cylinder_sections.py)
checks planarity, solid boundary, bounds, and closure independent of curve
degree and parameterization. Full NURBS definitions differ in these cases.
The B-rep intersection path also clips sections against curved face trim loops.
Height and angular isocurve trims on cylinder walls have exact kernel tests.
An exact non-isocurve split test covers a tilted ellipse trim, transverse
quarter-circle sections, and a single-point contact at the cut endpoint;
general curved trim loops still need broader oracle coverage.
The [trimmed wall Intersect fixture](../tools/rhino_oracle/fixtures/trimmed_cylinder_face_intersect_command.json)
compares four [Rhino observations](../tools/rhino_oracle/observations/trimmed_cylinder_face_intersect_command.json)
for lower and upper wall pieces, a disjoint plane, and a planar B-rep cutter.
All four match full curve definitions, including Rhino's `4π` domain for a
closed section of a trimmed cylindrical face.
The [angular wall fixture](../tools/rhino_oracle/fixtures/angular_cylinder_face_intersect_command.json)
matches three more [Rhino observations](../tools/rhino_oracle/observations/angular_cylinder_face_intersect_command.json)
for both half-cylinder faces and a B-rep cutter. Rhino doubles the parameter
interval of each exact semicircular section, so the two halves use `0..2π`
and `2π..4π`.
The [trimmed planar face fixture](../tools/rhino_oracle/fixtures/curved_surface_trimmed_plane_intersect_command.json)
checks the opposite arrangement: a cylindrical surface against either half
of a split planar B-rep face. Both [Xvfb-captured Rhino results](../tools/rhino_oracle/observations/curved_surface_trimmed_plane_intersect_command.json)
match the complete native NURBS definitions within `1e-10`.
The [coincident trimmed planar face fixture](../tools/rhino_oracle/fixtures/coincident_trimmed_planar_face_intersect_command.json)
checks a planar surface against a split planar B-rep face, with the surface
either enclosing or cutting through the face. Both [Xvfb-captured Rhino results](../tools/rhino_oracle/observations/coincident_trimmed_planar_face_intersect_command.json)
match the native closed perimeters exactly after canonicalizing linear curve
orientation and parameterization (`2/2` cases, zero coordinate difference).
The [coincident trimmed B-rep face fixture](../tools/rhino_oracle/fixtures/coincident_trimmed_brep_faces_intersect_command.json)
uses the same two regions as B-rep/B-rep command inputs. Both [Xvfb-captured Rhino results](../tools/rhino_oracle/observations/coincident_trimmed_brep_faces_intersect_command.json)
also match the native closed perimeters exactly after the same linear curve
canonicalization (`2/2` cases, zero coordinate difference).
The [coincident box fixture](../tools/rhino_oracle/fixtures/coincident_box_brep_intersect_command.json)
compares identical boxes and coaxial boxes with overlapping heights. The
[edge audit](../tools/rhino_oracle/audit_coincident_box_edges.py) verifies that
both engines return every one of the twelve overlap-box edges exactly once,
with zero coordinate error in both cases. Rhino joins the coaxial case into
five curves; Viboceros joins the same edges into four.
The [bent planar strip fixture](../tools/rhino_oracle/fixtures/coincident_bent_strip_surface_intersection_command.json)
checks an enclosing rectangle and a partial rectangle against a quadratic
planar strip, repeats the partial overlap with surface/B-rep and B-rep/B-rep
command inputs, and intersects two bent strips in both input orders and with
shifted parameter domains. All ten [Xvfb-captured Rhino results](../tools/rhino_oracle/observations/coincident_bent_strip_surface_intersection_command.json)
match the complete native degree-two curves, including control points, knots,
and domains, within `1.8e-15`. The [transposed strip fixture](../tools/rhino_oracle/fixtures/coincident_transposed_bent_strip_surface_intersection_command.json)
repeats seven surface cases with the curved direction in V; its
[Xvfb observations](../tools/rhino_oracle/observations/coincident_transposed_bent_strip_surface_intersection_command.json)
also match the native curves exactly within floating-point tolerance.
The [rational strip fixture](../tools/rhino_oracle/fixtures/coincident_rational_bent_strip_surface_intersection_command.json)
and its [transposed companion](../tools/rhino_oracle/fixtures/coincident_transposed_rational_bent_strip_surface_intersection_command.json)
add eight weighted planar-strip overlaps. Their
[U-direction](../tools/rhino_oracle/observations/coincident_rational_bent_strip_surface_intersection_command.json)
and [V-direction](../tools/rhino_oracle/observations/coincident_transposed_rational_bent_strip_surface_intersection_command.json)
Xvfb observations match all native control points, weights, knots, and domains.
The [mixed-axis strip fixture](../tools/rhino_oracle/fixtures/coincident_mixed_axis_bent_strip_surface_intersection_command.json)
intersects U-curved strips with V-curved strips in both orders, with polynomial
and rational weights, shifted parameter domains, and rotated geometry. All
twelve [Xvfb observations](../tools/rhino_oracle/observations/coincident_mixed_axis_bent_strip_surface_intersection_command.json)
match the full native curves.
The [nonproportional rational strip fixture](../tools/rhino_oracle/fixtures/coincident_nonproportional_rational_strip_surface_intersection_command.json)
and its [orientation variants](../tools/rhino_oracle/fixtures/coincident_nonproportional_rational_strip_orientation_command.json)
cover twelve overlaps whose two weight rows are not proportional, including
transposed and mixed-axis inputs. The
[U-direction observations](../tools/rhino_oracle/observations/coincident_nonproportional_rational_strip_surface_intersection_command.json)
and [orientation observations](../tools/rhino_oracle/observations/coincident_nonproportional_rational_strip_orientation_command.json)
match all native curves. A separate
[refined-strip fixture](../tools/rhino_oracle/fixtures/coincident_refined_nonproportional_strip_surface_intersection_command.json)
checks degree elevation and an inserted knot in four more cases; its
[Xvfb results](../tools/rhino_oracle/observations/coincident_refined_nonproportional_strip_surface_intersection_command.json)
match within `7.2e-15`.
The [bilinear plane fixture](../tools/rhino_oracle/fixtures/bilinear_plane_intersect_command.json)
adds five saddle-patch `Intersect` cases: two conic branches, a clipped branch,
two plane-contained rulings, two isolated corner contacts, and rationally
weighted conics. The [saved Rhino results](../tools/rhino_oracle/observations/bilinear_plane_intersect_command.json)
fit cubics to the conics, while the native kernel keeps exact rational
quadratics. The [locus audit](../tools/rhino_oracle/audit_bilinear_plane_sections.py)
checks the resulting geometry independently of curve degree and parameterization.
Its maximum sampled surface deviation is `2.63e-6` for Rhino's fitted curves
and below `7e-16` for the native exact curves. The straight rulings and corner
points have the same locus; only the corner-point case matches full output
definitions at the default `1e-10` comparison tolerance.

```sh
python3 -m tools.rhino_oracle.audit_bilinear_plane_sections \
  tools/rhino_oracle/fixtures/bilinear_plane_intersect_command.json \
  tools/rhino_oracle/observations/bilinear_plane_intersect_command.json
```

```sh
python3 tools/rhino_oracle/audit_cylinder_sections.py \
  tools/rhino_oracle/fixtures/curved_cylinder_brep_intersect_command.json \
  tools/rhino_oracle/observations/curved_cylinder_brep_intersect_command.json \
  --max-error 5e-6
```

To keep Wine/Rhino completely off the active desktop, use the isolated Xvfb
runner (requires `Xvfb`, `xvfb-run`, and `i3`):

```sh
tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/parabola_three_point.json \
  --absolute-epsilon 2e-12 --relative-epsilon 1e-12

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/helix.json \
  --absolute-epsilon 2e-5 --relative-epsilon 1e-12

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/spiral.json \
  --absolute-epsilon 3e-5 --relative-epsilon 1e-12

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/swept_spiral.json \
  --absolute-epsilon 2e-7 --relative-epsilon 1e-12

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/catenary.json \
  --absolute-epsilon 2e-8 --relative-epsilon 1e-12

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/curve_through.json \
  --absolute-epsilon 2e-12 --relative-epsilon 1e-12

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/curve_tween.json \
  --absolute-epsilon 5e-8 --relative-epsilon 1e-12

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/curve_tween_short_samples.json \
  --absolute-epsilon 2e-12 --relative-epsilon 1e-12

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/curve_fit.json \
  --absolute-epsilon 2e-12 --relative-epsilon 1e-12

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/curve_rebuild.json

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/curve_make_uniform.json

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/surface_make_uniform.json

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/make_uniform_commands.json

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/curve_insert_knot.json

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/surface_insert_knot.json

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/insert_control_point.json

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/curve_change_seam.json

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/surface_change_seam.json

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/reparameterize.json

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/reparameterize_automatic.json \
  --relative-epsilon 1e-8

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/surface_extend.json

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/surface_extend_length.json

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/surface_extend_line.json

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/curve_extend_line.json \
  --absolute-epsilon 5e-14 --relative-epsilon 1e-12

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/curve_extend_command.json \
  --absolute-epsilon 2e-8 --relative-epsilon 1e-11

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/curve_extend_arc.json \
  --absolute-epsilon 2e-11 --relative-epsilon 2e-12

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/curve_extend_arc_command.json \
  --absolute-epsilon 2e-8 --relative-epsilon 1e-11

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/curve_extend_boundary_command.json

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/surface_shrink.json \
  --absolute-epsilon 3e-8 --relative-epsilon 1e-10

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/curve_subcurve.json

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/curve_split.json

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/curve_split_command.json

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/surface_split_isocurve_command.json

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/surface_split_cutting_command.json

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/curve_intersect_command.json \
  --absolute-epsilon 1e-9 --relative-epsilon 1e-11

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/curve_surface_intersect_command.json \
  --absolute-epsilon 1e-9 --relative-epsilon 1e-11

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/curve_brep_intersect_command.json \
  --absolute-epsilon 1e-9 --relative-epsilon 1e-11

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/surface_surface_intersect_command.json \
  --absolute-epsilon 1e-9 --relative-epsilon 1e-11

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/surface_brep_intersect_command.json \
  --absolute-epsilon 1e-9 --relative-epsilon 1e-11

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/brep_brep_intersect_command.json \
  --absolute-epsilon 1e-9 --relative-epsilon 1e-11

# Full-domain curved B-rep faces against surfaces or another face.
python3 -m tools.rhino_oracle replay \
  tools/rhino_oracle/fixtures/curved_brep_face_intersect_command.json \
  --observations tools/rhino_oracle/observations/curved_brep_face_intersect_command.json

python3 -m tools.rhino_oracle replay \
  tools/rhino_oracle/fixtures/curved_brep_face_circle_intersect_command.json \
  --observations tools/rhino_oracle/observations/curved_brep_face_circle_intersect_command.json

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/curve_trim_command.json \
  --absolute-epsilon 5e-6 --relative-epsilon 1e-11

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/surface_direction_edit.json

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/remove_knot.json \
  --absolute-epsilon 2e-11 --relative-epsilon 2e-12

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/remove_control_point.json \
  --absolute-epsilon 1e-10 --relative-epsilon 1e-10

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/remove_multi_knot.json

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/make_non_periodic.json

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/curve_make_periodic.json \
  --absolute-epsilon 2e-11 --relative-epsilon 2e-12

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/curve_make_periodic_degrees.json \
  --absolute-epsilon 2e-11 --relative-epsilon 2e-12

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/surface_make_periodic.json \
  --absolute-epsilon 2e-11 --relative-epsilon 2e-12

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/curve_change_degree.json \
  --absolute-epsilon 2e-11 --relative-epsilon 2e-12

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/surface_change_degree.json \
  --absolute-epsilon 2e-11 --relative-epsilon 2e-12
```

The same workflow is importable for instrumentation and tests. Run the Python
process in the dedicated Xvfb session with
`tools/rhino_oracle/run_headless.sh exec python3 your_script.py`:

```python
from tools.rhino_oracle import OracleClient, load_request

report = OracleClient().compare(load_request("tools/rhino_oracle/fixtures/core.json"))
assert report.passed
```

Set `VIBOCEROS_RHINO_LAUNCHER` to use another launcher. McNeel's documented
`/runscript` startup interface is tried first; this project's Wine path also
has an owned-window fallback that requires `wmctrl` and `xdotool` and never
targets a pre-existing Rhino process. Set `VIBOCEROS_RHINO_UI_FALLBACK=0` to
disable it. The `viboceros` and `rhino` modes run either side independently.
On Linux the driver reports a Rhino process that exits before publishing a
response after a two-second exit grace period. If no owned Rhino process ever
appears, it reports startup failure after 30 seconds; an active process keeps
the full requested probe timeout.

### SetView option prompt probe

The bounded `set_view_prompt_probe` operation runs whitelisted option macros
and records public camera/CPlane properties and Rhino command history. Its
[fixture](../tools/rhino_oracle/fixtures/set_view_prompt.json) covers 30
transitions in three workflows: all World and CPlane choices, Enter/cancel at
both stages, rejected choices, and nested CPlane/Plan commands. The
[Rhino 8.32 observation](../tools/rhino_oracle/observations/set_view_prompt.json)
was captured on a private Xvfb display:

```sh
tools/rhino_oracle/run_headless.sh rhino tools/rhino_oracle/fixtures/set_view_prompt.json --timeout 300
```

Each macro ends with a cancel to bound incomplete input. The recorded history
may contain `Unknown command: !` after an already completed command; the raw
capture is retained. `succeeded` reports macro termination, not a camera
comparison verdict. Enter exits either SetView stage without choosing a view.
At the World view stage, `CPlane World Top` runs transparently and returns to
the outstanding World choices. At the CPlane view stage, `World` is rejected.
A [supplemental fixture](../tools/rhino_oracle/fixtures/set_view_nested_cplane.json)
and [private-display observation](../tools/rhino_oracle/observations/set_view_nested_cplane.json)
check three additional nested CPlane World transitions, including Enter and an
invalid choice. Enter cancels that child prompt and returns to SetView.

The native application regression replays the tokens through the command bar's
command handler and checks all 33 camera directions, up vectors, targets,
construction-plane frames, and projection flags at component epsilon `2e-12`:

```sh
cargo test --release -p viboceros app::tests::set_view
```

This fixture omits frusta, so it does not compare optical framing or camera
location. The following camera probes cover framing independently; document
clipping and parallel camera depth relocation remain pending. The generic
oracle `compare` mode does not execute these interactive application prompts.

### Named-view and World preset policy probes

The [World policy fixture](../tools/rhino_oracle/fixtures/view_camera_world_policy.json)
and [Rhino 8.32 capture](../tools/rhino_oracle/observations/view_camera_world_policy.json)
cover all four combinations of **Named views set CPlane** and **Named views set
projection**, with eight World choices from parallel, perspective, and two-point
inputs: 96 transitions. The [named-view fixture](../tools/rhino_oracle/fixtures/named_view_policy.json)
and [capture](../tools/rhino_oracle/observations/named_view_policy.json) cover 72
restores: four policies, every saved/current projection pair, and centered versus
rescaled, vertically shifted saved frusta. All live captures use private Xvfb:

```sh
tools/rhino_oracle/run_headless.sh rhino tools/rhino_oracle/fixtures/view_camera_world_policy.json --timeout 300
tools/rhino_oracle/run_headless.sh rhino tools/rhino_oracle/fixtures/named_view_policy.json --timeout 300
cargo test --release -p viboceros named_view_policy
```

The bounded `named_view_policy_probe` creates disposable named views using
public RhinoCommon APIs. Cleanup deletes only its own records and independently
restores camera, target, CPlane, name, and both application settings, including
after failure. Source initialization forces both settings on before setting the
requested source projection: Rhino's public `SetProjection` also honors the
projection policy. The tested settings are applied afterward.

With projection restoration disabled, named views keep the destination's binary
parallel/perspective family but copy the saved pose and raw frustum, including
near/far distances. Saved two-point locks return only in a perspective destination.
World presets instead use their existing scale conversion rules; World
TwoPointPerspective always forces two-point projection. CPlane restoration is
independent, and World Perspective always keeps the CPlane.

Native replays check camera and CPlane components at `2e-12` and captured screen
coordinates at `1e-8` pixels, plus independent camera/CPlane histories, failed
conversion atomicity, and 3DM named-view encoding. Precise screen queries retain
lens shift in `f64`. World comparisons exclude document-driven near/far changes
and parallel camera depth relocation; named-view restoration compares all six
saved frustum dimensions and camera location. The generic oracle `compare`
mode does not execute these application-level operations.

### SetView CPlane camera probe

The bounded `view_camera_probe` operation uses public RhinoCommon viewport
properties to record camera direction, up vector, location, target, projection,
and construction plane after all six `SetView CPlane` directions in parallel and
perspective viewports. It restores the original view projection, target, name,
CPlane, and model aid settings. Run the Rhino side with:

```sh
tools/rhino_oracle/run_headless.sh rhino tools/rhino_oracle/fixtures/view_camera_cplane.json --timeout 300
```

The [saved Rhino 8.32 observation](../tools/rhino_oracle/observations/view_camera_cplane.json)
was captured on a separate Xvfb display. Compare its camera and CPlane values
with the current Viboceros camera rule:

```sh
python3 -m tools.rhino_oracle.view_camera_probe tools/rhino_oracle/fixtures/view_camera_cplane.json tools/rhino_oracle/observations/view_camera_cplane.json
```

The comparator checks the six orientations, CPlane axes and origin, camera
target, projection, whether each parallel SetView preserves frustum width, and
whether each perspective SetView preserves the camera distance measured
immediately before the command. All 12 states pass at componentwise epsilon
`1e-9`; the recorded parallel frustum-width change is exactly zero, and the
largest observed perspective distance change is `1.5e-14` model units. Rhino's
startup distance in this capture is about `102.226`, while Viboceros currently
starts at `50`; the comparison tests the SetView transition from the existing
camera, not identical startup framing.
Parallel camera location is diagnostic: Viboceros retains a camera distance for
3DM interchange, but Rhino can relocate parallel cameras when recomputing
document clipping. The camera probe has no native oracle operation yet, so the
generic `compare` mode is unavailable for this fixture. The independent Rust
viewport tests cover all six directions in both projections and preservation of
nondefault parallel zoom and perspective camera distance.

### CPlane two-point and shifted camera probe

The [full camera fixture](../tools/rhino_oracle/fixtures/view_camera_cplane_two_point.json)
and [Rhino 8.32 observation](../tools/rhino_oracle/observations/view_camera_cplane_two_point.json)
record 64 transitions: all six SetView CPlane directions, Plan, and CPlane View,
from ordinary parallel/perspective, two-point, and parallel views restored from
two-point named views. Two-point and restored parallel inputs include centered,
positive, and negative vertical frustum shifts. Capture and replay with:

```sh
tools/rhino_oracle/run_headless.sh rhino tools/rhino_oracle/fixtures/view_camera_cplane_two_point.json --timeout 300
cargo test --release -p viboceros cplane_two_point
```

All live captures use private Xvfb. The `full_camera` flag records camera pose,
target, CPlane, projection, and frustum before and after each command, plus
projected world points. `parallel_from_two_point` initializes a shifted parallel
source by restoring a disposable two-point named view with projection restoration
disabled. Cleanup deletes only owned named records and independently restores
projection, target, CPlane, name, and application settings.

SetView CPlane retains the binary parallel/perspective family, clears two-point
locks and frustum shifts, and uses the CPlane origin as target. Plan expands a
perspective near frustum by `target_distance / near` when that ratio exceeds one;
otherwise it keeps the raw width. CPlane View instead retains projection and
framing and uses the nominal camera target for its origin, independently of the
frustum center. Several inputs have targets off the camera axis. Native import
retains those offsets instead of moving the camera or replacing the saved target.

Native replay checks camera/CPlane components at `2e-12`, optical framing at
`2e-12`, and screen coordinates at `1e-8` pixels. GPU checks allow f32 rounding;
drafting rays round-trip through view-normal planes within `1e-4` model units.
Side views are edge-on to the original CPlane, where parallel drafting rays have
no unique plane intersection. Tests also check independent histories, Plan scale
limits, explicit target replacement, and 128 captured input/output cameras through
actual 3DM files. Document-driven clipping and parallel camera depth relocation
remain excluded. The generic Python comparator directs full captures to the
native tests.

### Plan camera probe

The same bounded probe records `Plan` from parallel and perspective views with
an oblique CPlane. It measures
[frustum width](https://developer.rhino3d.com/api/rhinocommon/rhino.docobjects.viewportinfo/frustumwidth)
and [world-to-screen scale](https://developer.rhino3d.com/api/rhinocommon/rhino.display.rhinoviewport/getworldtoscreenscale)
before and after the command. The [saved Rhino 8.32 observation](../tools/rhino_oracle/observations/view_camera_plan.json)
was captured in a separate Xvfb display. Replay its focused comparison with:

```sh
python3 -m tools.rhino_oracle.view_camera_probe tools/rhino_oracle/fixtures/view_camera_plan.json tools/rhino_oracle/observations/view_camera_plan.json
```

Both cases match the expected camera orientation, target, CPlane, and parallel
projection. The parallel source retains its frustum width and screen scale
exactly; the native `Plan` command now retains its parallel drawing scale too.
The perspective source also retains raw frustum width in Rhino, but its screen
scale at the CPlane origin changes by a factor of `0.8483105677` during the
projection switch. Its
[frustum near distance](https://developer.rhino3d.com/api/rhinocommon/rhino.docobjects.viewportinfo/frustumnear)
also remains unchanged at `120.5053553` model units. The ratio equals the recorded camera distance
`102.2259664` divided by that near distance, to floating-point precision.
The native viewport now retains the near and far frustum distances through 3DM
views and view history. Its Plan conversion uses the measured ratio; a focused
Rust test checks the saved Rhino width and scale. The Python comparator still
marks native zoom parity as unchecked because the generic oracle `compare` mode
has no native camera operation for either fixture.
The full camera probe above also covers inputs with the target beyond the near
plane, where Plan scales the raw width to the target depth.

### World parallel camera probe

The [World parallel fixture](../tools/rhino_oracle/fixtures/view_camera_world_parallel.json)
records all six `SetView World` parallel presets with a translated target and
oblique construction plane. It covers parallel, perspective, and two-point
inputs at their current zoom and after `Zoom Factor 2.5`, plus positive and
negative vertical shifts of a two-point frustum: 48 recorded transitions.
Capture on a private display with:

```sh
tools/rhino_oracle/run_headless.sh rhino tools/rhino_oracle/fixtures/view_camera_world_parallel.json --timeout 300
```

The [saved Rhino 8.32 observation](../tools/rhino_oracle/observations/view_camera_world_parallel.json)
includes the complete camera and frustum before and after each command, plus
projected world points. The native regression imports each measured input and
executes the corresponding World preset:

```sh
cargo test --release -p viboceros viewport::tests::world_parallel
```

Camera direction, up, target, CPlane origin/axes, and the four framing dimensions
agree within `2e-12`. Precise CPU projection agrees with Rhino's screen
coordinates within `1e-8` pixels. GPU comparisons account for `f32` rounding,
including points far outside a highly zoomed viewport; drafting picks round-trip
within `1e-4` model units. The tests also cover panned parallel input, clearing
two-point locks, view history, and 3DM named-view conversion. Parallel scale is
retained in `f64` through import, camera operations, snapping, picking, scene
cache keys, and export.

The transitions preserve the camera target and CPlane origin. Parallel inputs
retain frustum width. Perspective inputs use the frustum at the target depth,
clamped to the old near/far interval. Lens shift is cleared. Document-based
clipping can also move Rhino's parallel camera along the view direction without
changing framing; matching those depth relocations and near/far updates remains
pending. The test excludes those fields and does not claim full camera-record
parity. The Python CPlane comparator and generic oracle `compare` do not execute
these native World transitions.

### World perspective and two-point camera probe

The [two-point fixture](../tools/rhino_oracle/fixtures/view_camera_two_point.json)
records `SetView World Perspective` and `SetView World TwoPointPerspective`
from both parallel and perspective inputs with a translated target and an
oblique construction plane. Six additional operations deliver bounded right
drags in the owned viewport: horizontal, vertical, diagonal, shorter, and
negative vertical motion. Each drag uses the private Xvfb display and cancels
its disposable `EvaluatePt` prompt afterward. Capture with:

```sh
tools/rhino_oracle/run_headless.sh rhino tools/rhino_oracle/fixtures/view_camera_two_point.json --timeout 240
```

The [saved Rhino 8.32 observation](../tools/rhino_oracle/observations/view_camera_two_point.json)
contains the input and output camera state, complete frusta, projection flags,
construction planes, viewport sizes, and projected world points. The native
viewport regression imports each measured input and checks the actual camera
transition and navigation, plus undo/redo and named-view restoration:

```sh
cargo test --release -p viboceros viewport::tests::two_point
```

Camera and plane components and frustum dimensions normalized by the near
distance use tolerance `2e-12`. Projected points agree with Rhino within
`1e-4` pixels; the CPU/GPU comparison uses `1e-3` pixels. Drafting picks also
round-trip through shifted perspective rays. The IO tests check projection
locks and shifted frusta in both named and working 3DM views.
Rhino recomputes document-based near/far clipping after these commands;
Viboceros currently retains its existing distances, so clipping parity is
excluded from this comparison. The generic oracle `compare` operation and the
Python CPlane comparator do not run these native camera transitions.

## Zoom fitting cameras

The [Zoom fitting request](../tools/rhino_oracle/fixtures/zoom_extents_camera.json)
and [Rhino 8.32 capture](../tools/rhino_oracle/observations/zoom_extents_camera.json)
contain 85 public `_Zoom _Extents`, `_Zoom _Selected`, and
`RhinoViewport.ZoomBoundingBox` fits, all recorded in private Xvfb. The bounded
helper refuses a nonempty document, adds only its own point objects, and restores
camera, target, CPlane, title, and application settings independently on failure.
It deletes only its owned objects. Captures include all six standard parallel
directions, perspective and two-point projection, varying lens width, vertical
shift beyond the screen center, borders above and below one, translated and thin
boxes, isolated points, tiny models, and a depth-aligned line.

```sh
tools/rhino_oracle/run_headless.sh rhino tools/rhino_oracle/fixtures/zoom_extents_camera.json --timeout 300
cargo test --release -p viboceros zoom_extents
```

Native replay checks optical framing, targets, axes and projection at `2e-12`,
CPU screen coordinates at `1e-8` pixels, GPU agreement within f32 rounding, and
selection/model/CPlane history plus view undo/redo. Parallel and perspective
locations match the captures; explicit bounding-box fits also match all six
frustum values. Command fits include document-driven parallel camera depth
relocation. This camera fixture is exercised by
native viewport tests, not the generic geometry `compare` command.
The separate [border request](../tools/rhino_oracle/fixtures/zoom_extents_borders.json)
and [capture](../tools/rhino_oracle/observations/zoom_extents_borders.json) add
20 private-Xvfb cases with factors 0.1, 0.8, 1, 1.5, and 10. Both direct public
settings writes and `SetZoomExtentsBorder` retain values below one, but actual
Extents fits use one. Command cases begin with a distinct 1.3 setting and record
the resulting settings and command history, so a command that silently ignores
the requested value cannot pass this check.

### Document clipping and infinite-frustum bounds

Three additional private-Xvfb requests/captures exercise the same bounded helper:

| Fixture | Cases | Coverage |
| --- | ---: | --- |
| [viewport_clipping](../tools/rhino_oracle/fixtures/viewport_clipping.json) / [capture](../tools/rhino_oracle/observations/viewport_clipping.json) | 81 | 42 document fits and 39 public constrained `ViewportInfo.SetFrustumNearFar` calls, including rejected reversed intervals |
| [viewport_clipping_context](../tools/rhino_oracle/fixtures/viewport_clipping_context.json) / [capture](../tools/rhino_oracle/observations/viewport_clipping_context.json) | 72 | Selected fits with unselected visible/hidden boxes, in Wireframe/Shaded/Ghosted |
| [viewport_box_depth](../tools/rhino_oracle/fixtures/viewport_box_depth.json) / [capture](../tools/rhino_oracle/observations/viewport_box_depth.json) | 48 | Public `ViewportInfo.GetBoundingBoxDepth` before fitting, with shifted parallel/perspective/two-point frusta and inside/outside/crossing boxes |

All captures use Rhino 8.32.26160.13001. The helper adds only owned point
objects to an empty document, shows owned hidden points before deleting them,
and restores camera, target, CPlane, title, display mode, and application
settings independently. Public clipping constraints operate on a disposable
`ViewportInfo` copy. Depth queries ignore current near/far planes, as documented
by [McNeel](https://mcneel.github.io/rhinocommon-api-docs/api/RhinoCommon/html/M_Rhino_DocObjects_ViewportInfo_GetBoundingBoxDepth.htm).

```sh
tools/rhino_oracle/run_headless.sh rhino tools/rhino_oracle/fixtures/viewport_clipping.json --timeout 300
cargo test --release -p viboceros viewport::tests::zoom_extents
```

Native replay checks all six frustum values at relative tolerance `2e-12` and
camera components at absolute tolerance `2e-12`. Initial and redraw clipping
are checked separately: a parallel fit can relocate its camera, changing the
box-intersection tolerance used on the next redraw. Visible geometry contributes
to a combined world bounding box before frustum intersection; clipping each
object separately does not match these captures. Tests also retain screen
framing, construction planes, selection, and model/view history.

Viboceros stages this clipping calculation for Extents/Selected and their All
variants, and refreshes it during drawing after navigation and scene edits.
GPU depth encoding retains a local range but rendering obeys stored clip planes.
These fixtures run native
viewport tests, not the generic geometry `compare` operation.

### Redraw after navigation and document edits

The [redraw fixture](../tools/rhino_oracle/fixtures/viewport_clipping_redraw.json)
and [Rhino capture](../tools/rhino_oracle/observations/viewport_clipping_redraw.json)
record 189 redraws across 72 cases: parallel/perspective/two-point cameras,
Wireframe/Shaded/Ghosted, Zoom Factor, public dolly/pan API calls, World
presets, hidden/restored geometry, deleted geometry, and repeated redraws.
The [fallback fixture](../tools/rhino_oracle/fixtures/viewport_clipping_fallback.json)
and [capture](../tools/rhino_oracle/observations/viewport_clipping_fallback.json)
add 24 redraws from six origin/translated empty-scene cases. Run with:

```sh
tools/rhino_oracle/run_headless.sh rhino tools/rhino_oracle/fixtures/viewport_clipping_redraw.json --timeout 360
cargo test --release -p viboceros redraw_clipping
```

Each bounded action records snapshots immediately before and after public
`Views.Redraw`/`RhinoApp.Wait`. The native replay imports the measured input
camera, reconstructs the visible document, and refreshes clipping. It checks
full camera/frustum output at `2e-12` and projected points at `1e-8` pixels,
with camera history unchanged. A separate egui regression verifies the actual
drawing hook. Shared cached bounds and invalidation tests cover layout/camera
changes and retained results for selection changes.

An empty scene uses a unit box at the world origin, including when the camera
target is translated. A box outside the view uses default depths 0.005 and
1000 before projection-specific padding and constraints. This can retreat a
parallel camera on every redraw while preserving screen framing. Constrained
perspective near distances retain Rhino's final 0.99 ratio allowance after
applying the measured bias.

Public pan calls can retain a nonorthogonal CameraUp hint. Native cameras keep
that hint separately from perpendicular screen axes so clipping does not
silently change the saved camera. The following fixture checks display clipping.

### GPU frustum clipping

The [clip-transform fixture](../tools/rhino_oracle/fixtures/viewport_clip_transform.json)
and [capture](../tools/rhino_oracle/observations/viewport_clip_transform.json) record
1,200 point queries in 48 camera cases using public
[GetTransform](https://mcneel.github.io/rhinocommon-api-docs/api/RhinoCommon/html/M_Rhino_Display_RhinoViewport_GetTransform.htm)
and [IsVisible](https://mcneel.github.io/rhinocommon-api-docs/api/RhinoCommon/html/M_Rhino_Display_RhinoViewport_IsVisible.htm).
All six standard parallel directions, perspective, and two-point perspective
cover normal/tiny/translated boxes and centered/shifted source frusta. Applying
the shifted source through public `SetViewProjection` normalizes ordinary live
frusta and retains only vertical shift in two-point perspective; the measured
output camera is the reference. Query generation adds no document geometry.

```sh
tools/rhino_oracle/run_headless.sh rhino tools/rhino_oracle/fixtures/viewport_clip_transform.json --timeout 300
cargo test --release -p viboceros gpu_projection_and_clip_intervals
cargo test --release -p viboceros gpu_faces_wires_and_points_obey_saved_rhino_clip_planes -- --ignored --nocapture
```

The ordinary test checks GPU XY projection within `2e-5` in normalized clip
coordinates, perspective Z with the OpenNURBS-to-wgpu range conversion, and
visibility outside a `2e-7` band around exact plane contacts. Parallel depth
remains encoded relative to scene bounds; transformed fragment bounds enforce
the actual near/far planes. The offscreen test checks 4,032 face/wire/point
coverage samples in 192 renders, in shaded/ghosted and both target formats.
These are public-API geometric comparisons, not Rhino screenshot/color matches.

### Selection at the depth planes

The [picking fixture](../tools/rhino_oracle/fixtures/viewport_clipping_picks.json)
and [capture](../tools/rhino_oracle/observations/viewport_clipping_picks.json)
reuse those 48 camera cases with `picking_probe=true`. After projection capture,
the worker adds nine owned lines, constructs disposable public
[PickContext](https://mcneel.github.io/rhinocommon-api-docs/api/RhinoCommon/html/T_Rhino_Input_Custom_PickContext.htm)
instances with the viewport's rectangle pick transform, and updates clipping
planes. It queries each line primitive and
[ObjectTable.PickObjects](https://mcneel.github.io/rhinocommon-api-docs/api/RhinoCommon/html/M_Rhino_DocObjects_Tables_ObjectTable_PickObjects.htm)
in PointPick, WindowPick, and CrossingPick modes. All 1,296 paired results agree.
The probe performs no redraw while its temporary lines exist, verifies the
camera is unchanged, disposes contexts/references, and deletes only owned lines.

```sh
tools/rhino_oracle/run_headless.sh rhino tools/rhino_oracle/fixtures/viewport_clipping_picks.json --timeout 300
cargo test --release -p viboceros viewport::clip_tests
```

In these captures, window selection rejects a line crossing any frustum plane;
crossing selection accepts its visible part. Click selection rejects a line
when its closest screen point is beyond a depth plane, even when an artificial
clipped endpoint lies within the capture aperture. The native replay checks
every primitive/document result, with no timing claim. These cases establish
line selection behavior for the measured cameras; face picking also has an
independent ray regression, but no native Rhino mesh-pick capture here.

### Snap targets outside the visible depth interval

The [30 snap requests](../tools/rhino_oracle/fixtures/snap_depth_visibility.json)
and [Rhino capture](../tools/rhino_oracle/observations/snap_depth_visibility.json)
use real public `GetPoint` input on a private Xvfb display. The optional
`camera_pose` sets a bounded perspective location/target through public
[SetCameraLocations](https://mcneel.github.io/rhinocommon-api-docs/api/RhinoCommon/html/M_Rhino_Display_RhinoViewport_SetCameraLocations.htm).
Rhino still computes its own document clip planes on redraw. `clipping_probe`
records the camera/frustum at the actual point prompt and after the click,
plus public World-to-Clip and IsVisible queries for the aim and picked point.
Every case retains identical prompt/post-click cameras, unchanged source
records, restored snap settings, and at least 250 ms of owned motion settling.

Fourteen admitted targets are outside those depth planes; seven are behind
the camera. Arc, circle, ellipse, circular NURBS, and closed-polygon Center
targets follow visible curve proximity. Mid-only hover can likewise return
a midpoint behind the camera. Mixed Mid+Cen rejects that off-camera midpoint;
Mid+Near captures the visible Near target. Fully behind-camera lines do not
admit direct End, Mid, or Near snaps, and a crossing line's rear endpoint is
not admitted.

The additional [27 line Near requests](../tools/rhino_oracle/fixtures/snap_crossing_line_near.json)
and [capture](../tools/rhino_oracle/observations/snap_crossing_line_near.json)
use nine pointer offsets, endpoint reversal, and a fully front-facing control.
Camera-crossing Near selects the original visible endpoint in either order,
including when a visible interior point is closer. Offsets of 16 pixels miss
that endpoint. The front-facing control retains ordinary interior Near targets.
There are 22 Near results and five misses, replayed at `1e-9` through the
viewport. Native camera queries receive explicit signed depth; projection
overflow and positive-depth clipping retain mathematical visible-locus search.
The same depth API is used by the calibrated Python oracle replay.

```sh
tools/rhino_oracle/run_headless.sh rhino tools/rhino_oracle/fixtures/snap_depth_visibility.json --timeout 300
tools/rhino_oracle/run_headless.sh rhino tools/rhino_oracle/fixtures/snap_crossing_line_near.json --timeout 300
cargo test --release -p viboceros viewport::drafting::clipping_tests
cargo test --release -p viboceros-drafting object_snap
python3 -m tools.rhino_oracle.point_snap_replay tools/rhino_oracle/fixtures/snap_crossing_line_near.json tools/rhino_oracle/observations/snap_crossing_line_near.json
```

Native tests replay all 30 kind/source/point results through the application
drafting query. Feature and line Near points compare at `1e-9` model units.
Four arc Near points compare at `1e-8`; independent closed-form ray/circle
solutions check native results at `1e-12`. A separate 90-digit Decimal
calculation found Rhino's front and behind-center arc Near positions differ
from that analytic optimum by `2.83e-9` and `3.04e-9`. The looser arc comparison
accounts for those recorded oracle residuals. Overlay tests check labels for
admitted rear targets, and their pixels match the recorded screen transform
within `1e-3` pixels. This does not establish Rhino screenshot appearance,
all camera orientations/scales, user clipping planes, or source admission at
every depth boundary. None results compare snap admission, not CPlane placement.

The calibrated Python CLI replay matches all 27 additional line cases; its
largest coordinate residual is `2.78e-17`. Verification passes 760 application,
157 drafting, and 447 oracle tests, plus all 488 Python tests. Formatting and
whitespace checks pass. Workspace Clippy completes with existing warnings in
unchanged files; its strict `-D warnings` gate still fails on those warnings.

### Camera-plane centers and straight-source representation

The [72 straight-source requests](../tools/rhino_oracle/fixtures/snap_crossing_straight_sources.json)
and [Rhino 8.32 captures](../tools/rhino_oracle/observations/snap_crossing_straight_sources.json)
cover lines, two-point polylines, and rational degree-one NURBS in both endpoint
orders, with world, translated, side, and oblique cameras. All 48 Near hits use
the original visible endpoint; all 24 aperture misses remain misses. The viewport
and calibrated Python CLI replay these inputs at `1e-9` model units. The CLI's
largest coordinate difference is `1.42e-14`.

The side camera lies in World XY. A missed point there leaves ordinary GetPoint
without a usable free plane intersection. The explicit diagnostic `camera_plane`
option sets a camera-facing construction plane at the target and restores it
afterward. Snap projection to the CPlane remains disabled. Its boolean validation,
setup failures, restoration failures, and resource cleanup are tested. Every
live capture uses a private Xvfb display and at least 250 ms of owned settling.

NURBS observations now retain public degree, knots, domain, controls, and weights
as well as the original five samples. The Python replay verifies the complete
definition before invoking the native engine; control coordinates compare at
`1e-10`, while weights, knots, degree, and domain must match. Before/after records
must be identical. Polyline vertices are verified directly. Targets never enter
native query inputs. Older sample-only NURBS observations do not establish a
complete source definition for this replay.

The [15 camera-plane requests](../tools/rhino_oracle/fixtures/snap_camera_plane_targets.json)
and [capture](../tools/rhino_oracle/observations/snap_camera_plane_targets.json)
include exact-plane Center/Mid targets and neighboring float/nanounit depths.
Circle Center rejects an exact camera-plane target but admits the adjacent floats
on either side. The recorded closed ellipse nets reject the same singular center;
polygon Center and Mid can still be admitted on that plane. Partial arc/quarter
NURBS centers in these records are a few float steps in front of the plane.

Rhino's ellipse conversion rounds the adjacent center shifts away in its actual
NURBS net. The viewport replay therefore reconstructs that recorded net, rather
than assuming an ideal analytic ellipse has identical binary64 controls. A
recognized four-span quadratic with coincident diagonal midpoints uses that
stable center only when the fit differs by at most four input-coordinate ULPs.
This corrects a three-ULP fit offset without changing conic recognition or the
geometry kernel. Admitted near-singular targets keep finite overlay labels at
the pointer. Public IsVisible reports true for the exact-plane polygon/Mid points
with zero clip coordinates; these values do not establish a finite camera image.

Verification passes 761 application, 157 drafting, and 447 oracle tests, plus 490
Python tests. These records do not establish all conic representations, arbitrary
camera poses/scales, user clipping planes, or screenshot appearance. CLI misses
compare admission, not free CPlane placement.

```sh
tools/rhino_oracle/run_headless.sh rhino tools/rhino_oracle/fixtures/snap_crossing_straight_sources.json --timeout 720
tools/rhino_oracle/run_headless.sh rhino tools/rhino_oracle/fixtures/snap_camera_plane_targets.json --timeout 300
python3 -m tools.rhino_oracle.point_snap_replay tools/rhino_oracle/fixtures/snap_crossing_straight_sources.json tools/rhino_oracle/observations/snap_crossing_straight_sources.json
cargo test --release -p viboceros viewport::drafting::clipping_tests
```

## Surface trim removal

[UntrimAll](commands/untrim-all.md) has 100 saved private-Xvfb command cases in
[the fixture](../tools/rhino_oracle/fixtures/untrim_all.json) and
[Rhino capture](../tools/rhino_oracle/observations/untrim_all.json). The probe
records public command completion, full constructed and inserted geometry,
result topology and definitions, chronological object order, selection, and
attributes. Source recipes are regenerated without using observed targets.
For raw surfaces, document snapshots compare a natural B-rep wrapper; the
constructed record retains the original NURBS surface definition.

The Python replay compares all 96 non-box cases with absolute epsilon `1e-9`
and relative epsilon zero, including four rejected two-face selections and
both workflows. Four independently built
box cases compare rejection, attributes, and unchanged geometry within each
engine; their different face/edge tables are not a topology-parity claim.
Command events and history are retained as diagnostics, outside the geometric
comparison. The adapter rejects changed geometry in those box cases before
comparison and sends only the original source fixture to the native engine.
Native commands and replay are untimed.
The recorded replay's maximum absolute difference is `3.55e-15`.

```sh
python3 -m tools.rhino_oracle.untrim_replay tools/rhino_oracle/fixtures/untrim_all.json tools/rhino_oracle/observations/untrim_all.json
```

[UntrimBorder](commands/untrim-border.md) has another 100 cases in
[its source fixture](../tools/rhino_oracle/fixtures/untrim_border.json) and
[Rhino capture](../tools/rhino_oracle/observations/untrim_border.json). The same
replay compares 96 complete definitions and scopes the four independent box
factories to rejection and unchanged geometry. Inner loops retain their exact
model-space curves, UV trims, tolerances, and winding while unused outer
topology is removed. The cases include multiple holes, reversed faces, shifted
and negative domains, natural surfaces, and exterior trims. Both commands
restore native parameter intervals on new boundary edges. Kernel checks also
exercise a hole on a closed cylinder with shared seam topology and permuted
multi-edge hole boundaries.

```sh
python3 -m tools.rhino_oracle.untrim_replay tools/rhino_oracle/fixtures/untrim_border.json tools/rhino_oracle/observations/untrim_border.json
```

The command reference lists the remaining face-subobject, hatch, and crease
settings gaps. These captures do not certify those workflows.

## Interior-hole removal geometry API

[`brep_remove_holes`](geometry/remove-holes.md) records the public
`Brep.RemoveHoles` overloads on identical, owned 3dm source files. Its
[28 source recipes](../tools/rhino_oracle/fixtures/brep_remove_holes.json) and
[Rhino capture](../tools/rhino_oracle/observations/brep_remove_holes.json) compare
complete before/after topology, NURBS definitions, UV trims, and tolerances with
absolute epsilon `1e-9` and relative epsilon zero. Selecting one opening of a
joined through hole removes its wall and closes both openings. Disconnected
caps are edited independently. Empty and outer-only selections return no result;
the all-hole overload copies no-hole sources unchanged. This untimed API probe
does not establish interactive `UntrimHoles` parity. The
[component command](commands/untrim-holes.md) now matches 110 native
cases: 44 face/edge cases, eight internal Undo/repicking cases, and 42 split-edge
perimeter-limit cases, including exact retained
curves and wall B-reps, source identity and metadata, current-layer defaults,
and edits surviving Esc, plus 16 multiple-pick and external Undo/Redo cases.
Local Undo discards the last pick; external Undo/Redo restores the complete
command. Multiple distinct preselected components fail without edits. The
viewport now supports edge/face postselection, options, ambiguity, and grouped
history; component preselection and window selection remain pending.
Raw command and Undo/Redo events/history are retained as diagnostics; only those
fields are omitted from the complete geometry/metadata comparison, including
the geometry snapshots after external Undo and Redo. These probes are untimed.
The release-mode Python replay matches all 110 cases with maximum absolute
error zero, including validation of each independently constructed 3dm source.

```sh
python3 -m tools.rhino_oracle replay tools/rhino_oracle/fixtures/brep_remove_holes.json --observations tools/rhino_oracle/observations/brep_remove_holes.json --absolute-epsilon 1e-9 --relative-epsilon 0 --timeout 300
python3 -m tools.rhino_oracle.untrim_replay tools/rhino_oracle/fixtures/untrim_holes_components.json tools/rhino_oracle/observations/untrim_holes_components.json --timeout 300
python3 -m tools.rhino_oracle.untrim_replay tools/rhino_oracle/fixtures/untrim_holes_undo.json tools/rhino_oracle/observations/untrim_holes_undo.json --timeout 300
python3 -m tools.rhino_oracle.untrim_replay tools/rhino_oracle/fixtures/untrim_holes_history.json tools/rhino_oracle/observations/untrim_holes_history.json --timeout 300
```

## Picked boundary restoration

General [Untrim](commands/untrim.md) has 231 saved private-Xvfb cases:
71 preselection/mouse cases, six partial exterior variants, four opposite tube
opening picks, 24 local Undo/Enter/Escape sequences, 96 rectangular source
permutations, four split-edge variants and eight polynomial/rational surface
partial edits, 16 ignored multi-face exterior picks, and two positive controls.
The multi-face sources include explicit exact edge joins and both face orders.
All sources are exported independently before Rhino starts. Public picking
queries confirm the owned object and first target edge before real mouse input;
correctly targeted edges may still be ignored by the command.
The driver gates inputs on public command
lifecycle events and records complete per-click and per-Undo geometry.

Replay retains control nets, knots, scalar intervals, face sense, vertex records,
UV trims, metadata and the complete incidence graph, including native edge-table
order and numeric component indices. The rectangular allocation policy stores
only measured component ordering classes; geometry is constructed independently.
No edge permutation is applied during comparison. Other partial-loop allocation
classes remain unverified, as detailed in the command's coverage notes.
Captures are split into bounded owned jobs, and each finished batch is saved
before the next begins.

```sh
python3 -m tools.rhino_oracle.untrim_replay tools/rhino_oracle/fixtures/untrim_components.json tools/rhino_oracle/observations/untrim_components.json
python3 -m tools.rhino_oracle.untrim_replay tools/rhino_oracle/fixtures/untrim_history.json tools/rhino_oracle/observations/untrim_history.json
python3 -m tools.rhino_oracle.untrim_replay tools/rhino_oracle/fixtures/untrim_ordering.json tools/rhino_oracle/observations/untrim_ordering.json
python3 -m tools.rhino_oracle.untrim_replay tools/rhino_oracle/fixtures/untrim_multiface.json tools/rhino_oracle/observations/untrim_multiface.json
```

## Joined edge separation

`brep_unjoin_edges` compares the public geometry API on independently built,
identically exported sources. `unjoin_edge_command` compares actual command
preselection and owned mouse batches, including genuine external Undo/Redo.
The saved corpora contain 26 API cases, 48 preselection/mouse command cases,
and [42 sequenced selection cases](../tools/rhino_oracle/fixtures/unjoin_edge_selection.json).
The latter mix modifier clicks and rectangles with `None` and prompt `Undo`;
each accepted input records the native SDK component selection. Complete
edge/vertex tables, surface control nets, UV trims, tolerances, object metadata, source
identities, and transient component selection remain in the comparison.
Only command lifecycle events and diagnostic history are excluded.

The command's copied-edge order differs from the API's input order. Captures
measure preselected batch sizes through twelve and two-/three-edge mouse
orders, including split shared edges and multiple sources. See
[command behavior and remaining limits](commands/unjoin-edge.md) and the
[geometry kernel](geometry/unjoin-edges.md).

The capture helper exports and validates all independent source responses
before starting Rhino. Owned paths replace caller paths and are cleaned after
the job. Mouse batches use one command pause, then an owned Enter/Escape event
to finish the multi-selection prompt. Sequence markers include a bounded step
index so repeated identical gestures are delivered independently. Inputs are
acknowledged before taking mouse/Undo selection snapshots; terminal `None`
is observed after command exit. Snapshots never drive input. The whitelist
permits modifier gestures and `None`/`Undo` keys only,
and owned key/button releases run even when an earlier release fails.
The independent rectangle adapter rejects tilted or nonlinear sources.
All live captures use private Xvfb.

```sh
python3 -m unittest tools.rhino_oracle.test_unjoin_edges
python3 -m tools.rhino_oracle.unjoin_edges_replay tools/rhino_oracle/fixtures/brep_unjoin_edges.json tools/rhino_oracle/observations/brep_unjoin_edges.json
python3 -m tools.rhino_oracle.unjoin_edges_replay tools/rhino_oracle/fixtures/unjoin_edge_command.json tools/rhino_oracle/observations/unjoin_edge_command.json
python3 -m tools.rhino_oracle.unjoin_edges_replay tools/rhino_oracle/fixtures/unjoin_edge_selection.json tools/rhino_oracle/observations/unjoin_edge_selection.json
tools/rhino_oracle/run_headless.sh exec python3 -m tools.rhino_oracle.unjoin_edges_capture tools/rhino_oracle/fixtures/unjoin_edge_command.json --timeout 300
```

## Timing interpretation

The comparison report's `rhino_to_viboceros_ratio` is a ratio of raw harness
elapsed times, **not a kernel speedup**. Its `timing_note` is included in both
the Python report and the CLI's JSON output to keep that qualification with
exported comparisons. Correctness checks do not depend on timing ratios.

The generic measurement helpers run one warm-up before timing repeated calls.
Process startup and transport JSON I/O are outside those loops, but the work
inside a call varies by probe and engine. Rhino's `mesh_unweld_edge`, for
example, still extracts result geometry into Python containers and disposes
the edited duplicate on every timed iteration. Excluding JSON encoding does
**not** necessarily exclude result extraction.

For angle-based `mesh_unweld`, both current implementations time creation of
the edited result and cleanup of the previous result (including the warm-up
result on the first iteration). Rhino duplicates its source for each in-place
edit; native Unweld returns an owned mesh. Both extract only the final result
outside the loop, then release that result. Source mesh construction is also
outside both loops. Worker lifecycle tests check event order and cleanup when
editing, timer reads, or result extraction fail. Historical angle-Unweld
recordings predate this timing-boundary change and included per-iteration
result extraction and disposal; their geometry values remain valid.
A fresh private-session run on Rhino 8.32.26160.13001 with the revised worker
matched all six `mesh_unweld.json` recorded geometry values exactly (100 timed
iterations per case). This verifies unchanged output, not equivalent kernel
performance.

Rhino measurements also include its Python/RhinoCommon bridge and, on this
development host, FEX/Wine overhead. Existing recorded responses retain their
original timings; geometry replay tests do not compare those elapsed times.
Before making a kernel-performance claim, audit the specific operation's
timing boundaries, match copying/extraction/cleanup work, use release-mode
native builds and sufficient iterations, and report the host/emulation setup.

## Shrink trimmed surfaces

Both [shrink commands](commands/shrink-trimmed-surfaces.md) have 132 independent
private-Xvfb cases: 60 base, 28 history/order, and 44 additional geometry cases.
Sources use the shared B-rep recipe and are exported/validated before Rhino
reads owned 3DM artifacts. Captures record complete raw definitions and topology,
attributes, groups, selection, command events, history, Undo and Redo.

`shrink_trimmed_cases.py` regenerates each source-only matrix;
`shrink_trimmed_capture.py` bounds each owned Rhino process to eight cases.
`shrink_trimmed_replay.py` compares all geometry and numeric indices at `1e-9`
absolute epsilon and zero relative epsilon. Snapshot order alone is matched by
source identity: repeated identical inputs with fresh UUIDs demonstrate varying
native renewal order. Raw observations retain chronological order. The geometry
matrix covers rational rotated bounds, signed weights, exact multi-span crops,
joined topology, seams, singularities, and UV origins at `1e12`.

```sh
tools/rhino_oracle/run_headless.sh exec python3 -m tools.rhino_oracle.shrink_trimmed_capture tools/rhino_oracle/fixtures/shrink_trimmed_surfaces.json --timeout 300
python3 -m tools.rhino_oracle.shrink_trimmed_replay tools/rhino_oracle/fixtures/shrink_trimmed_surfaces.json tools/rhino_oracle/observations/shrink_trimmed_surfaces.json
```

Face subobjects and disabling individual shrink sides remain outside this
coverage. These captures establish the stated cases, not full Rhino parity.
