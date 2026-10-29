# Geometry integration and representation

[Architecture overview](../architecture.md) · [Documentation](../README.md)

Implementation notes for this subsystem. Command pages and qualification records
describe the supported behavior and measured limits.

`Point3::midpoint` centralizes coordinate-wise standard floating-point midpoint
evaluation for box centers, curve-join endpoints, and point contacts. Scalar
midpoints in NURBS and intersection algorithms use the same standard operation.
Tests cover subnormal rounding, extreme coordinates, symmetry, and actual joined
polyline vertices under reversed source order; they do not establish Rhino
parity for subnormal geometry.
The `bounds/bezier` module handles bounded tensor subdivision and rational hulls;
curve/surface adapters select the complete native active spans.
`bounds/bezier/compose` constructs homogeneous rational surface/UV-curve
compositions. `bounds/parameter_curves` bounds their exact spatial images and
[B-rep trim boundaries](../trim-boundary-bounds.md), separately from face interiors.
`bounds/trimmed_surfaces` adds a stationary-extremum search; `bounds/trim_region`
uses rational curve hulls for conservative inside/outside/uncertain tests.
`Geometry::tight_bounds` centralizes dispatch without changing fast display bounds.
The independent [BoundingBox command](../commands/bounding-box.md) uses that query
in a geometry-local working frame, then stages oriented solids, quad meshes,
or grouped rectangle outputs before entering document mutation.
The separate [Distribute command](../commands/distribute.md) shares its
`object_bounds` working-frame query, then stages group-aware in-place
translations. Its option parser and logical-unit preflight are shared with the
application's two-point interaction; geometry code does not depend on egui.
`viboceros-command::interface` owns [display/drafting controls](../commands/interface.md).
The application adapter applies those actions without entering the modeling
command lifecycle; the compact toolbar and keyboard shortcuts share that path.
The headless oracle exercises the same reducer, independently of egui and wgpu.
`viboceros-command::construction_plane` owns validated [CPlane edits](../cplane.md)
and viewport-local history. The drafting plane module performs local grid,
ray-intersection, and projected tracking queries. Each viewport stores its frame
separately from camera state; the app's nested plane prompt does not replace its
active modeling prompt.

The current foundation supports finite 3D points, vectors, line segments,
analytic circles, circular arcs, and ellipses, validated open and closed
polylines, planes, bounding boxes, rational NURBS curves with analytic first
and second derivatives and exact knot refinement, splitting, and interval trimming,
rational NURBS surfaces with analytic first/second partial derivatives and exact tensor
splitting, knot refinement, and rectangular domain trimming,
validated shared-topology B-reps with exact rational parameter-space trims,
validated mixed triangle/quad polygon meshes, layers, groups, and bounded
undo/redo.
Native point clouds preserve point order and duplicates, cache finite bounds,
and use a balanced XY spatial index plus lazily initialized XZ/YZ indexes for
[parallel-view picking](../point-cloud-picking.md). Top/Front/Right cloud picking
queries these indexes with a separate camera origin and
local cursor offset. Distances are evaluated as `(point - origin) - offset`,
avoiding loss of cursor precision when forming an absolute world query. Grid
tests compare indexed results with local-coordinate exhaustive searches at
zero and large signed translations, including radius boundaries and ties.
Parallel-view object snapping uses the same relative-coordinate metric for every
feature and retains the indexed cloud search. Tests compare snap results with
an explicit local projection across translated point, cloud, and line scenes,
including locked targets, and check the viewport's pixel capture boundary.

B-rep area and volume live in `viboceros-geometry/src/brep/mass_properties/`
and its parent module. [Trimmed-domain integration](../mass-properties.md) uses
exact NURBS boundaries and adaptive quadrature, including nonplanar faces and holes.
[Boundary validation](../brep-validation.md) in `brep/validate` checks shared topology
and bidirectional edge/trim correspondence; matching endpoints alone do not
establish a valid face boundary.
`brep/components` exposes deterministic shared-edge face components, also used
by automatic B-rep joining. Union/find avoids constructing per-face adjacency
lists or copying geometry. Vertex-only contact and geometric coincidence do not
connect otherwise independent components; this is not a spatial containment or
solid-orientation classifier. Those policies are tracked in the
[orientation audit](../orientation-audit.md).
`brep/solid_orientation` adds a separate, conservative
[spatial query](../solid-orientation.md): exact planar-polygon first crossings and
minimum-X regular surface contacts on certified rectangular trims, with explicit
`Unknown` results for unsupported geometry or conflicting shells. Its `planar`
submodule separates exact face extraction, trim winding, and outside-ray tests.
The curved path shares exact homogeneous normal numerators with surface
evaluation; neither path normalizes document objects or uses volume sign as an
orientation predicate.
`document/object_admission` applies that query at explicit insertion/replacement
boundaries, including file imports and explicit replacement-geometry copies.
It reverses only known inward B-reps, before equality/history decisions or edits;
unknown geometry stays unchanged. Transform/morph paths remain separate, and
history restores immutable snapshots without reclassification. See the
[shared-source admission and import audit](../document-brep-admission.md).
The [morph assembler](../brep-morphing.md) in `brep/morph` retains shared topology
and exact UV trims. `brep/trim_image` supplies composed-curve correspondence to
validation and [B-rep meshing](../brep-meshing.md). `brep/tessellation` separates
independent face sampling, source-face boundary audits, and conforming
reconstruction; naked-edge provenance covers open shells as well as solids.

The [polycurve kernel](../polycurves.md) preserves independent exact segments and
parameter maps. Document geometry operations live in their own module, separate
from object state and history. Polycurves are integrated with transforms, rendering,
picking, endpoint snapping, extraction, explode, and 3DM interchange.
Representation-aware ownership, endpoint editing, and joining live in separate
geometry modules. `CloseCrv` stays in `curve_edit`; `Join` and `JoinCopy` share a separate
family dispatcher and curve-policy submodule. Curve representation and parameter
assembly live in `curve_join/assembly`. A read-only command completion hook lets
individual curve picks finish on closure, using the normal atomic execution path.
The independent `curve_join/search` module uses conservative bounding-box pairs
to find nearby endpoints without tolerance-scaled coordinate rounding; see
[endpoint-search evidence](../join-endpoint-search.md).
The `curve_join/seeded` module instead implements individual picks as a single
source pass, sharing the narrow-phase predicate and rank without constructing
the batch candidate graph; see [seeded connectivity](../seeded-join.md).
Seeded joining records the seed's mapped result parameter, so representation
consolidation does not force commands to guess its new seam from geometry.
Mesh concatenation, matching,
and component orientation live in the kernel's `mesh/append` and `mesh/join`
modules; [mesh joining](../commands/join.md) documents their explicit precision
policy and the command's selection-order and group behavior.
`mesh/polygon` shares normalized boundary projection, constrained Delaunay
triangulation and winding predicates across insertion, hole filling and planar
caps. A traversal of the triangulation toggles inside/outside across each
constrained edge, preserving narrow strips between holes without scanning every
boundary for every triangle. Triangle counts and area still enforce one outer
loop with disjoint, unnested holes. `mesh/hole_fill` owns boundary traversal,
cap admission and channel
transfer. Insertion retains its own cell pairing and quad origin policy;
the shared polygon geometry does not resolve the remaining
[warped insertion layouts](../geometry/mesh-insert-sources.md).
The separate `brep/join_edges` module provides
[certified explicit boundary assembly](../brep-edge-joining.md), with exact-binary
curve predicates separated from vertex unions, edge remapping, and face
orientation. It composes with shared-edge subdivision and preserves all source
surfaces and UV curves. Its `automatic` submodule separates broad-phase search,
straight partial-overlap planning, mutual unique candidate selection, subdivision
provenance, and connected extraction. The separate
[spatial rebuilding policy](../join-gap-rebuilding.md)
uses exact weighted means and independent lifted-boundary uncertainty bounds.
The small attributed OpenNURBS chord-adjustment adaptation lives in
`third_party/opennurbs_rust`; rational Bernstein bounds live in `certificate/refine`.
Certified original-source contacts are separate from accepted mates.
The `join/breps` adapter owns batch/seeded selection
policy, [selection-dependent distances](../join-selection-distance.md), and
reconsiders original accepted boundaries; shared piece copying preserves
all disconnected outputs even when several inherit one source's attributes/groups.
See [curve joining and closure](../curve-editing.md) for tested policies and limits.
All seven curve families share [native parameter evaluation](../curve-parameters.md),
including analytic derivatives and parameter-bearing arc-length samples. Circular
support frames are distinct from complete circles and their native domains.
The `curve/arc_length` sampler separates span integration, repeated-query tables,
inversion, and kink detection from the curve dispatch API. Its `spans` submodule
owns native span construction and initial integration, with regression tests in
a separate sibling module. NURBS, polyline, and polycurve sampling use checked
internal frames while retaining native output parameters. Linear spans retain
source indices for direct local-distance point and tangent evaluation.
Native trim, split, closest-point dispatch, and cyclic edits live in `curve_trim`;
seam, subcurve, and reparameterization commands share the `curve_domain` module.
The command `curve_reference` module resolves ordinary curves and surface edges;
[SubCrv edge input](../subcurve-edge-input.md) retains parent topology and attributes
and guards interactive references with immutable snapshots.
The `curve_parameter_map` geometry module supplies exact span-aware correspondence
with rational representations; the `curve_cut` command module uses it for
[cutting-object splits and trims](../curve-cutting.md).
NURBS differential evaluation and homogeneous weight matching have separate
`nurbs/evaluate` and `nurbs/weights` modules. See [rational numerical policy](../nurbs-numerics.md)
for local-coordinate evaluation, degree-one acceleration, and scale-safe seam joins.
`nurbs/exact` supplies shared exact homogeneous recurrences for guarded curve and
surface evaluation. Curve [range-loss and failure recovery](../curve-rational-range.md)
keeps tangent direction separate from representability of derivative magnitude.
Surface [regular normal queries](../surface-normals.md) likewise separate direction
from derivative magnitude, with outward-rounded filtering and exact recovery.
`nurbs/arc_length` owns accuracy-controlled full-curve length integration.
`nurbs/integration_frame` supplies checked dimensionless parameter preparation
shared by length, sampling, shortness, and enclosed curve-area queries.
`nurbs/sampling` provides [lossless fractional point sampling](../curve-parameter-sampling.md)
and exact span limits without changing stored native domains. The viewport's
shared segment visitor uses it for drawing, click picking, and window selection.
Its [exceptional fractional recovery](../curve-fractional-recovery.md) preserves
unrepresentable intermediate stations and shares exact recurrences with native
mixed-weight curve jets.
`nurbs/weights/end_weights` owns projective endpoint normalization and
piecewise-Bezier end-weight changes, including near-equal and extreme gauges.
Loft and Sweep share its geometry-preserving normalizer; explicit common-profile
knot policies and Rhino's measured near-equal-weight drift remain separate.
The shared [one-sided evaluator](../curve-sided-evaluation.md) propagates side choices
through composite leaves and supplies exact kink checks and stationary tangents.
The [curve-frame module](../curve-frames.md) separates span-aware adaptive tangent
transport from frame construction. `ArrayCrv`, swept spirals, and `Sweep1` share it;
corner-side policy and remaining Rhino corner differences are explicit.
The `nurbs_surface/evaluate` module shares `ParameterSide` and nonempty knot-span
selection with curves. Its [surface jets](../surface-evaluation.md) use local rational
coordinates and expose exact U/V limits and boundary-span continuation.
Its separate [exact-rational evaluator](../surface-rational-range.md) handles detected
range loss, mixed-sign active weights, variable-weight continuation, and failed
floating-point evaluation. The [pole audit](../surface-pole-recovery.md) documents
why a finite rounded denominator is insufficient; ordinary same-sign nets stay fast.
Shared [exact dyadic recurrences](../exact-dyadic-evaluation.md) avoid repeated GCDs
when the original homogeneous coefficients and exact factors permit it.
Closest-point refinement owns a [last-span query cache](../surface-query-cache.md)
for prepared controls and lazy exact derivative nets. It does not change the
surface, cache evaluated stations, or relax pole detection.
Its [candidate module](../surface-candidate-selection.md) ranks original-surface
points with conservative distance bounds and exact tie/overlap predicates,
including grid selection and rounded boundary-curve proposals.
The [curve Rebuild command](../commands/rebuild-curves.md) shares readonly output
preparation between scripts, typed previews and acceptance. Curve and surface
options use separate typed branches of the same preview scene and stale-source
guards. Output-policy edits reuse prepared geometry; new geometric options rebuild
it, and failed preparation blocks acceptance while permitting correction.

The [curve closest-point module](../curve-closest-point.md) shares immutable distance
keys with surfaces. It separates bounded candidate selection from range-safe
Newton/tangent refinement; translated curves only propose parameters, whose
original model-space images decide the result.
Both frames own a [curve query cache](../curve-query-cache.md): immutable active-span
controls, lazy floating/exact derivative nets, and reusable floating scratch.
No evaluated point or pole status is cached, and late failures do not change
the span's normal floating-point dispatch.
The separate [curvature module](../curvature.md) forms a scale-aware orthonormal
shape operator from those jets. Geometry evaluation is independent of the
command's closest-point selection, reports, and permanent markers.
The [curve fitter](../curve-morphing.md) in `morph/curve_fit` and
[surface fitter](../surface-morphing.md) in `morph/surface_fit` are separate from
point-map construction. They share sided cubic banded interpolation in
`spline_collocation`, retain source knot limits, and explicitly fail when
sampled fitting tolerance cannot be reached within their resource budgets.
Both fitters also check bounded rational composition candidates:
[curves](../curve-rational-fitting.md) and [surfaces](../surface-rational-fitting.md).
`nurbs2/evaluate` provides stable, sided UV-trim evaluation independently of
model-space curves, including exact constant parameter coordinates. Its
[exact-rational fallback](../uv-rational-range.md) shares `nurbs/exact` recurrences
for points and native first derivatives. `brep/parameter_normalization` separately
owns independent UV-axis normalization and width-relative interval tolerances
for topology predicates, containment, and triangulation.
`brep/parameter_frame` prepares [lossless local UV frames](../brep-parameter-frames.md)
for validation, containment, meshing, and mass integration, avoiding native UV
rounding between trim and surface evaluation without altering stored geometry.
Shared surface preparation lives in `nurbs_surface/parameter_frame`;
`brep/rectangular_surface` constructs independently parameterized spatial edges
and `nurbs_surface/wires` generates local-domain borders and density wires.
See [surface construction and wire frames](../surface-wire-frames.md).
`brep/isocurves` uses the same frames for trim intersections and density stations;
it restores extracted curve domains only losslessly and keeps display wires
locally parameterized. The `extract_isocurve` command module owns parsing,
selection, and atomic document updates. See [isocurve precision](../brep-isocurves.md).

The [loft kernel](../loft.md) and [one-rail sweep](../sweep1.md) share exact degree/knot
matching and the explicit common-basis end-weight policy in `section_basis`.
Loft normalizes each profile's input scale independently; Sweep1 preserves
relative profile scales and applies end-weight normalization only after
retained-basis section placement. `sweep/weights` separately checks denominator
positivity by bounded scalar subdivision, without changing the surface basis.
The sweep separates frame placement and arc-length blending from fitting and
from the command's document operations. `sweep/basis` chooses the rail basis,
adds section interpolation stations, and solves homogeneous profile trajectories;
`sweep/fit` separately approximates the continuous transport/blend model.
The shared `curve_fit` kernel refits rails before section interpolation. Cubic
curve fits, sweep collocation, and morph fits reuse banded `spline_collocation`.
Ordered tensor lofts are independent of command
orientation and crease splitting. `brep/surface_grid` supplies exact shared
topology for tensor partitions, including closed seams and singular sides.
The [edge-surface constructor](../edge-surfaces.md) separately handles boundary
ordering, pairwise parameter compatibility and homogeneous Coons blending.
Unlike Loft's common-basis policy, three/four-edge endpoint-weight normalization
precedes pair matching. Exact degree elevation retains finite patches whose
original Coons basis contains a projective control at infinity.

The [point-grid module](../point-grid-surfaces.md) separates tensor interpolation,
open/periodic basis policy, and polynomial shell-orientation integration.
Command adapters preserve Rhino's input order while the kernel retains U-fast
storage. Complete face records distinguish API tensor construction from
command-level shared crease topology.
