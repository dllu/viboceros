# Clipping, selection and snap comparisons

[Oracle overview](../oracle.md) · [Evidence index](README.md)

## Document clipping and infinite-frustum bounds

Three additional private-Xvfb requests/captures exercise the same bounded helper:

| Fixture | Cases | Coverage |
| --- | ---: | --- |
| [viewport_clipping](../../tools/rhino_oracle/fixtures/viewport_clipping.json) / [capture](../../tools/rhino_oracle/observations/viewport_clipping.json) | 81 | 42 document fits and 39 public constrained `ViewportInfo.SetFrustumNearFar` calls, including rejected reversed intervals |
| [viewport_clipping_context](../../tools/rhino_oracle/fixtures/viewport_clipping_context.json) / [capture](../../tools/rhino_oracle/observations/viewport_clipping_context.json) | 72 | Selected fits with unselected visible/hidden boxes, in Wireframe/Shaded/Ghosted |
| [viewport_box_depth](../../tools/rhino_oracle/fixtures/viewport_box_depth.json) / [capture](../../tools/rhino_oracle/observations/viewport_box_depth.json) | 48 | Public `ViewportInfo.GetBoundingBoxDepth` before fitting, with shifted parallel/perspective/two-point frusta and inside/outside/crossing boxes |

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

## Redraw after navigation and document edits

The [redraw fixture](../../tools/rhino_oracle/fixtures/viewport_clipping_redraw.json)
and [Rhino capture](../../tools/rhino_oracle/observations/viewport_clipping_redraw.json)
record 189 redraws across 72 cases: parallel/perspective/two-point cameras,
Wireframe/Shaded/Ghosted, Zoom Factor, public dolly/pan API calls, World
presets, hidden/restored geometry, deleted geometry, and repeated redraws.
The [fallback fixture](../../tools/rhino_oracle/fixtures/viewport_clipping_fallback.json)
and [capture](../../tools/rhino_oracle/observations/viewport_clipping_fallback.json)
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

## GPU frustum clipping

The [clip-transform fixture](../../tools/rhino_oracle/fixtures/viewport_clip_transform.json)
and [capture](../../tools/rhino_oracle/observations/viewport_clip_transform.json) record
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

## Selection at the depth planes

The [picking fixture](../../tools/rhino_oracle/fixtures/viewport_clipping_picks.json)
and [capture](../../tools/rhino_oracle/observations/viewport_clipping_picks.json)
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

## Snap targets outside the visible depth interval

The [30 snap requests](../../tools/rhino_oracle/fixtures/snap_depth_visibility.json)
and [Rhino capture](../../tools/rhino_oracle/observations/snap_depth_visibility.json)
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

The additional [27 line Near requests](../../tools/rhino_oracle/fixtures/snap_crossing_line_near.json)
and [capture](../../tools/rhino_oracle/observations/snap_crossing_line_near.json)
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

## Camera-plane centers and straight-source representation

The [72 straight-source requests](../../tools/rhino_oracle/fixtures/snap_crossing_straight_sources.json)
and [Rhino 8.32 captures](../../tools/rhino_oracle/observations/snap_crossing_straight_sources.json)
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

The [15 camera-plane requests](../../tools/rhino_oracle/fixtures/snap_camera_plane_targets.json)
and [capture](../../tools/rhino_oracle/observations/snap_camera_plane_targets.json)
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
