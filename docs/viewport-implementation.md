# Viewport implementation and precision

[Architecture](architecture.md) · [Viewport controls](interface.md) · [GPU tests](gpu-tests.md)

Implementation details and regression coverage for the viewport modules. These
checks cover specific numeric and interaction cases, not complete Rhino parity
or unrestricted accuracy across every model and GPU backend.

## Drafting and tracking

The app's `viewport/drafting` module resolves the drafting cursor through the
shared drafting kernel and draws construction-plane grids, accepted points,
tracking guides, snap markers, and cursor labels. Guide clipping happens before
dash tessellation, keeping distant anchors from causing unbounded allocations.
Active non-curve prompts with an accepted anchor also draw its point marker when
no preview polyline is present. This does not require viewport hover, so typing
options does not hide the accepted input. Shape-level tests cover all four view
kinds, anchor removal, completed prompts, and avoiding duplicate markers when a
preview polyline already supplies them.
The drafting kernel's grid snap uses a signed remainder and a local adjustment,
avoiding overflowing grid indices for finite coordinates and fine spacing. It
retains plane elevation, rounds halfway cases away from zero, and rejects a
genuinely unrepresentable final grid point. `SnapSize` sets the spacing per
viewport without changing the displayed grid line interval. `Grid` can change
the snap interval, line spacing, major interval, line count, and grid/axis
visibility. Visible line indices are centered near the current construction-plane
view point, bounded to 501 per axis, and clipped before egui tessellation.
Within `viboceros-drafting`, `object_snap` owns feature enumeration, projection
metrics, indexed point-cloud queries, and snap priority. The crate root re-exports
the public snap API and retains shared errors and basic tracking; `plane` owns
plane-local drafting, and `point_input` owns typed point interpretation. API
regression tests live in a separate `tests.rs` module.
The shared [leaf-feature/cache modules](composite-feature-snaps.md) keep polycurve
segment enumeration and expensive arc-length/surface-boundary queries independent
of camera projection. The viewport retains the cache across ordinary and
edge-constrained point prompts. The separate [Center-hover query](center-hover-snaps.md)
scores proximity to analytic curves and returns their off-cursor center. Direct
features suppress Center only on the same object; cross-object ranking uses
capture distance. Conservative projected bounds avoid refining distant conics.
The [Mid-only hover query](mid-hover-snaps.md) shares `object_snap/proximity` with
Center. It ranks individual segments/boundaries by curve distance and returns
their cached half-arc-length targets; multiple enabled modes retain direct Mid
capture. NURBS refinement evaluates separate sided knot spans, with common-sign
control bounds and a straight-span fast path. Surface caches retain their actual
boundary curves, and failed midpoint slots remain paired with their sources.
The [polygon Center cache](polygon-center-snaps.md) retains boundary segments,
corner averages and failed recognitions. Its source keys include curve geometry,
planar surfaces and relevant face boundary edges/orientations; UV trims and
attributes are excluded. Geometry/tolerance edits and Undo refresh entries.
Production queries share these targets across ordinary and constrained prompts.
The [circular NURBS query](circular-center-snaps.md) shares source snapshots and
sided-span proximity with Mid; arc-length integration and whole-span circular/elliptic recognition
are independently lazy. Center uses original arc/boundary proximity, never a
substituted complete supporting circle or ellipse. See [elliptical Center](elliptic-center-snaps.md).
The [snap-control adapter](object-snap-controls.md) passes the effective feature
mask through ordinary and edge-constrained prompts. An empty mask exits the
drafting query before traversing objects or refreshing caches; validation still
runs. Pending one-shot overrides belong to the active point-input scope, not to
the viewport or document undo history.
Basic XY tracking treats an overflowing distance as out of range only on that
axis, preserving valid perpendicular capture. Projected tracking accepts an
in-range anchor before computing plane-local axis candidates; an unusable cursor
offset cannot suppress an already valid anchor hit. Regression tests cover both.
Relative/projected snap queries and projected tracking reject non-finite cursor
coordinates with a shared drafting input error before evaluating geometry or
calling projection callbacks, including for empty documents. Unavailable or
non-finite candidate projections remain uncapturable rather than input errors.
Clipping uses the line's dominant screen coordinate rather than a normalized
segment parameter, so distant endpoints do not collapse a visible guide to a
single point. Exact endpoint tests cover huge horizontal, vertical, and diagonal
guides, endpoint reversal, and finite-segment versus infinite-line clipping.

## Selection dispatch

The `viewport/selection` module owns selectable-object filtering, click
dispatch and curve capture, projected primitives for window/crossing selection,
and selection-window feedback. The interaction loop delegates selection to this
module; projected curve segments are also reused for drafting previews.
The `viewport/picking` module owns face hit metrics, perspective-correct
depth interpolation, and mesh/NURBS/B-rep hit evaluation. Screen capture and
feature priority remain separate from depth ordering among overlapping face hits.

## Screen-space predicates

The shared `viewport/screen` module owns triangle containment, point-to-segment
distance, and line/rectangle clipping. Rectangle selection and drafting guides
use the same dominant-coordinate clipper; selection also supports point segments
and degenerate rectangles. An independent integer-orientation reference checks
7,203 segment/rectangle cases, alongside distant-diagonal miss regressions.
Coordinate differences use f64 intermediates so opposite finite f32 screen endpoints do not overflow;
rectangle tests cover misses, crossings, endpoint reversal, and boundary contact.
Segment click distances use separate endpoint dot tests and a perpendicular
determinant, rather than reconstructing a nearest point from a rounded parameter.
A fixed-size, allocation-free expansion preserves cancelling coordinate products.
Tests check 15,625 integer-reference distances and long-segment document capture.
The same determinant is used by triangle containment and face-depth weights.
Tests retain the area and distinguish both sides of long triangle edges in all
vertex orders; 5,000 finite-coordinate cases check determinant signs and values
against the geometry kernel's exact accumulator (within one f64 ULP).

## Face-depth interpolation

Constant-depth faces retain their exact depth rather than accumulating rounded
barycentric products. Parallel interpolated depths are bounded by vertex depths;
a scaled, renormalized fallback handles overflowing products at captured edges.
Grid tests exercise positive/negative f64-limit depths, subnormal constants, and
nearby finite vertex depths; document picking checks nearer-face choice in both
insertion orders.
Perspective interpolation clamps tolerated edge weights to nonnegative values
and scales reciprocal depths by the nearest contributing vertex. This avoids
negative edge-hit depths and reciprocal overflow for tiny positive depths;
zero-weight vertices do not influence the scale. Tests check exact vertex hits,
finite depth bounds, and an analytic harmonic mean across binary scales.

## Scene staging

The `viewport/scene` module owns visible-object display dispatch, sampled curve
and tessellated surface submission, per-primitive f64 depth staging, transparent
triangle sorting, and final GPU buffer assembly. Crease-aware corner normals,
coincident-vertex grouping, display-color resolution, GPU normal/color conversion,
and their unit tests also live with scene staging. Checked scalar coordinate
conversion lives with the camera. The interaction module invokes the scene
builder without owning its internal staging types. This extraction changes
module ownership, not rendering algorithms; application and offscreen pixel tests
cover the same production path.

## Camera coordinates

The app's `viewport/camera` module owns CPU projection/unprojection, drafting
rays, navigation updates, view depth, and GPU camera matrices. It rebases GPU
positions around the model-space camera target in f64 before f32 conversion;
parallel views additionally apply pixels-per-model-unit before GPU conversion.
This avoids subnormal screen coefficients at tiny parallel zoom scales. The
scene builder retains per-vertex depth in f64 until all submitted primitives
have contributed to the range, then encodes parallel depth into `[0.05, 0.95]`
(a singleton range uses `0.5`). Perspective positions retain their normal
camera-space projection. Cursor unprojection
shares this origin convention: perspective screen rays
and drafting-plane intersections are evaluated locally, adding the target only
when constructing the final model point. This avoids rounding an absolute camera
position before intersection. Tests cover translated and tilted drafting planes,
anchor overrides, a trillion-unit unprojection regression, and unchanged rejection
of edge-on or behind-camera planes. Viewport drawing
and hit-testing consume these shared methods; camera math remains independent
of the painter and document mutation. Its extraction retains the projection,
target-plane zoom, and multi-frame navigation regressions.
Screen-to-model conversion promotes screen coordinates to f64 before subtracting
them, avoiding f32 intermediate overflow in parallel/perspective unprojection
and drafting rays. Tests cover both screen axes, all view kinds, and a facing
construction plane while retaining near-parallel drafting-ray rejection.

## Zoom fitting

The app's `viewport/extents` module stages visible-bounds camera fitting for
the actual local GPU representation. It validates transformed corners after
choosing the target and scale, rather than rejecting large absolute coordinates.
Tests include finite points at the f64 limit and small parallel-view screen
extents at extreme depth, alongside genuinely unrepresentable-span failures.
It provides [`Zoom Extents` and `Zoom Selected`](commands/zoom.md). All-view actions reuse
one bounds query and prepare every `CameraFit` before applying any camera changes.
The fit stages a complete camera snapshot, retaining its lens and projection
locks while clearing shifts. Screen spans, aspect fitting, and depth padding
match public OpenNURBS `ON_DollyExtents` and 105 Rhino camera captures.
`viewport/clipping` then computes the document near/far interval and parallel
camera relocation before committing the fit. Degenerate screen boxes use a
one-unit square; small models do
not inherit the interactive navigation scale/distance limits.
Border settings below one remain stored but fit as one, matching Rhino 8.32.
The model-space camera target is shared by
CPU/GPU projection and drafting rays; fitting does not edit construction planes
or model history. The interface parser emits a host action rather than putting
viewport navigation into document transactions.

## Clipping after document fits

Extents/Selected and their All variants intersect the combined visible world
bounding box with the infinite view frustum. Off-screen unselected geometry
can influence clipping through that combined box; hidden objects and layers
are excluded. The depth query ignores stored near/far planes, retains shifted
frusta, and follows the public OpenNURBS camera-coordinate tolerance and
frustum corner-ray rules. Fully contained or excluded boxes have fast paths.

Near/far constraints use the attributed public `ON_Viewport::SetFrustumNearFar`
adaptation in `third_party/opennurbs_rust/viewport_clipping.rs`.
Document padding and bias are calibrated independently from Rhino 8.32 public
command/API outputs. Tests replay 201 additional clipping captures: 42 fits,
39 constrained setter calls, 72 Selected fits with context/visibility across
three display modes, and 48 bounding-box depth queries. Camera poses and
initial/redraw clip intervals use tolerance `2e-12`; refreshing metadata adds
no camera history entry. Zoom All still preflights every view atomically.

Drawing refreshes stored clipping before interaction and again when that
frame's navigation changes the camera. It also handles document edits,
visibility/layer changes, projection changes, and layout resize. The empty
visible scene fallback is a unit world-origin box. A box outside the view uses
the measured default depth interval; in parallel views this may dolly the
camera on successive redraws. Refresh never adds model/CPlane/camera history.
Two more fixtures replay 213 redraws in 78 cases, including translated empty
scenes, zoom/dolly/pan, World presets, hide/show/delete, and all three display
modes. Camera components use `2e-12`, projected points `1e-8` pixels.

Immutable display geometry now caches bounds once. Shared caches reuse them
across views; the clipping key compares camera, viewport dimensions, and the
combined eligible bounds. Selection/color edits reuse clipping. Its input
camera is retained so a parallel depth relocation can correctly update the
intersection tolerance on the next draw.

GPU faces, wires, and points now obey the stored near/far planes. Perspective
projection constrains its depth interval to those planes. Parallel scenes keep
their local depth encoding for precision and transform the stored plane depths
into fragment bounds; discarded fragments write neither color nor depth.
This preserves nearby-face ordering even at large absolute depths. Scene keys
include the complete camera snapshot, including imported axes and lens settings.

Another private-Xvfb capture records 1,200 public World-to-Clip/IsVisible point
queries across 48 standard, perspective, and two-point camera cases, with tiny
and translated models and shifted-view input. Ordinary tests check GPU XY
projection within `2e-5` in clip coordinates and visibility away from exact
plane contacts. The opt-in GPU test checks face/wire/point coverage near both
depth planes in shaded and ghosted modes, with both target formats (192 renders).
Parallel encoded depth coordinates intentionally differ from Rhino's clip Z;
the clipping region is compared. This establishes specific frustum cases,
not complete display parity or behavior at every numeric boundary.
CPU selection uses the same stored depth interval. Points and point-cloud
members outside it cannot win a click; indexed cloud searches filter candidates
before nearest ordering, preserving hidden flags and stored-index ties. Lines
test the depth at their closest screen point, without treating a new endpoint
on a clip plane as a separate click target. Face interiors interpolate depth
on the original projected triangles and reject clipped hits while retaining
source face indices.

Window selection requires all source vertices/sampled wire points inside the
depth interval. Crossing selection tests the visible clipped portions. Triangle
clipping uses fixed arrays for the resulting polygon (at most five vertices).
The kernel's scalar interpolation falls back to rational arithmetic for severe
cancellation and extreme intermediate ranges, retaining separate clip-plane
intersections on long segments. Ordinary interpolation keeps a floating-point
path; this is not a general exact clipping predicate.

A private-Xvfb fixture records 1,296 primitive/document line picking results
across 48 cameras using public Rhino pick contexts. Native click/window/crossing
tests replay every result. Independent ray tests cover face interiors crossing
both depth planes in all eight views and both shaded modes. These picking tests
do not establish every inverted-selection case.

Snapping uses its own admission rules. Thirty private-Xvfb GetPoint captures
show 14 admitted targets outside the display depth interval, including seven
behind the camera. Center and Mid-only hover test the visible source curve;
their finite target can lie outside the projection half-space. Direct point
features and source-curve proximity retain camera-plane rejection. Mixed-mode
Mid remains a direct-feature query. Snap overlays use signed perspective depth
for admitted targets; a target without a finite image keeps its label at the
pointer. Ordinary geometry projection still rejects the camera's back side.
See [snap depth evidence](oracle.md#snap-targets-outside-the-visible-depth-interval)
for source families, independent Near checks, and the limited camera scope.

Imported nonorthogonal `CameraUp` hints are retained separately from the
orthonormal rendering frame, including through clipping refresh and 3DM
encoding. Orientation changes update or replace the hint.

## Related behavior and limits

See [Zoom](commands/zoom.md) for camera limits and prompt preservation,
[construction planes](cplane.md) for drafting behavior and remaining SmartTrack
scope, and [GPU tests](gpu-tests.md) for rendering coverage and backend limits.
