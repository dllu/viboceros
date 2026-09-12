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
genuinely unrepresentable final grid point. The current UI still uses unit spacing.
Within `viboceros-drafting`, `object_snap` owns feature enumeration, projection
metrics, indexed point-cloud queries, and snap priority. The crate root re-exports
the public snap API and retains shared errors and basic tracking; `plane` owns
plane-local drafting, and `point_input` owns typed point interpretation. API
regression tests live in a separate `tests.rs` module.
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
The model-space camera target is shared by
CPU/GPU projection and drafting rays; fitting does not edit construction planes
or model history. The interface parser emits a host action rather than putting
viewport navigation into document transactions.

## Related behavior and limits

See [Zoom](commands/zoom.md) for camera limits and prompt preservation,
[construction planes](cplane.md) for drafting behavior and remaining SmartTrack
scope, and [GPU tests](gpu-tests.md) for rendering coverage and backend limits.
