# Camera and view comparisons

[Oracle overview](../oracle.md) · [Evidence index](README.md)

## SetView option prompt probe

The bounded `set_view_prompt_probe` operation runs whitelisted option macros
and records public camera/CPlane properties and Rhino command history. Its
[fixture](../../tools/rhino_oracle/fixtures/set_view_prompt.json) covers 30
transitions in three workflows: all World and CPlane choices, Enter/cancel at
both stages, rejected choices, and nested CPlane/Plan commands. The
[Rhino 8.32 observation](../../tools/rhino_oracle/observations/set_view_prompt.json)
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
A [supplemental fixture](../../tools/rhino_oracle/fixtures/set_view_nested_cplane.json)
and [private-display observation](../../tools/rhino_oracle/observations/set_view_nested_cplane.json)
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

## Named-view and World preset policy probes

The [World policy fixture](../../tools/rhino_oracle/fixtures/view_camera_world_policy.json)
and [Rhino 8.32 capture](../../tools/rhino_oracle/observations/view_camera_world_policy.json)
cover all four combinations of **Named views set CPlane** and **Named views set
projection**, with eight World choices from parallel, perspective, and two-point
inputs: 96 transitions. The [named-view fixture](../../tools/rhino_oracle/fixtures/named_view_policy.json)
and [capture](../../tools/rhino_oracle/observations/named_view_policy.json) cover 72
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

## SetView CPlane camera probe

The bounded `view_camera_probe` operation uses public RhinoCommon viewport
properties to record camera direction, up vector, location, target, projection,
and construction plane after all six `SetView CPlane` directions in parallel and
perspective viewports. It restores the original view projection, target, name,
CPlane, and model aid settings. Run the Rhino side with:

```sh
tools/rhino_oracle/run_headless.sh rhino tools/rhino_oracle/fixtures/view_camera_cplane.json --timeout 300
```

The [saved Rhino 8.32 observation](../../tools/rhino_oracle/observations/view_camera_cplane.json)
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

## CPlane two-point and shifted camera probe

The [full camera fixture](../../tools/rhino_oracle/fixtures/view_camera_cplane_two_point.json)
and [Rhino 8.32 observation](../../tools/rhino_oracle/observations/view_camera_cplane_two_point.json)
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

## Plan camera probe

The same bounded probe records `Plan` from parallel and perspective views with
an oblique CPlane. It measures
[frustum width](https://developer.rhino3d.com/api/rhinocommon/rhino.docobjects.viewportinfo/frustumwidth)
and [world-to-screen scale](https://developer.rhino3d.com/api/rhinocommon/rhino.display.rhinoviewport/getworldtoscreenscale)
before and after the command. The [saved Rhino 8.32 observation](../../tools/rhino_oracle/observations/view_camera_plan.json)
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

## World parallel camera probe

The [World parallel fixture](../../tools/rhino_oracle/fixtures/view_camera_world_parallel.json)
records all six `SetView World` parallel presets with a translated target and
oblique construction plane. It covers parallel, perspective, and two-point
inputs at their current zoom and after `Zoom Factor 2.5`, plus positive and
negative vertical shifts of a two-point frustum: 48 recorded transitions.
Capture on a private display with:

```sh
tools/rhino_oracle/run_headless.sh rhino tools/rhino_oracle/fixtures/view_camera_world_parallel.json --timeout 300
```

The [saved Rhino 8.32 observation](../../tools/rhino_oracle/observations/view_camera_world_parallel.json)
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

## World perspective and two-point camera probe

The [two-point fixture](../../tools/rhino_oracle/fixtures/view_camera_two_point.json)
records `SetView World Perspective` and `SetView World TwoPointPerspective`
from both parallel and perspective inputs with a translated target and an
oblique construction plane. Six additional operations deliver bounded right
drags in the owned viewport: horizontal, vertical, diagonal, shorter, and
negative vertical motion. Each drag uses the private Xvfb display and cancels
its disposable `EvaluatePt` prompt afterward. Capture with:

```sh
tools/rhino_oracle/run_headless.sh rhino tools/rhino_oracle/fixtures/view_camera_two_point.json --timeout 240
```

The [saved Rhino 8.32 observation](../../tools/rhino_oracle/observations/view_camera_two_point.json)
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

The [Zoom fitting request](../../tools/rhino_oracle/fixtures/zoom_extents_camera.json)
and [Rhino 8.32 capture](../../tools/rhino_oracle/observations/zoom_extents_camera.json)
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
The separate [border request](../../tools/rhino_oracle/fixtures/zoom_extents_borders.json)
and [capture](../../tools/rhino_oracle/observations/zoom_extents_borders.json) add
20 private-Xvfb cases with factors 0.1, 0.8, 1, 1.5, and 10. Both direct public
settings writes and `SetZoomExtentsBorder` retain values below one, but actual
Extents fits use one. Command cases begin with a distinct 1.3 setting and record
the resulting settings and command history, so a command that silently ignores
the requested value cannot pass this check.
