# Architecture and implementation status

[Project overview](../README.md)

The dependency direction runs from mathematical primitives through document
state and commands to the user interface. Geometry does not depend on the UI,
file formats, or command parsing.

Vector cross products use compensated two-product determinants. Extreme-range
recovery is component-local: ordinary components are retained instead of being
recomputed from globally normalized vectors. Regression tests
cover cancellation, exact parallelism, disparate magnitudes, axis permutations,
and genuine result overflow. Each determinant falls back to the exact dot-product
accumulator for overflow recovery or when product bits could be lost below the
subnormal range. Binary-scaled integer determinants check representable
differences of both overflowing and near-underflow normal products.
Direct vector dot products compensate both multiplication and summation
rounding; tests cover exact orthogonality, near-cancelling integer products,
and small terms between large opposite terms. Dot products use an allocation-free
fixed-size integer accumulator when individual products overflow or could lose
bits below the subnormal quantum (even when the rounded products are normal),
or when the running sum overflows. The accumulator spans the full binary64
product range and rounds only the final sum (nearest, ties to even);
tests compare it with hardware multiplication and fused multiply-add across
10,000 deterministic full-range inputs, plus cancellation and rounding boundaries.

| Module | Responsibility |
| --- | --- |
| `viboceros-geometry` | Validated primitives, NURBS, intersections, B-rep topology, tessellation, and mass properties; nalgebra and faer provide linear algebra. |
| `viboceros-document` | Objects, attributes, layers, groups, selection, transactions, and bounded undo/redo. |
| `viboceros-drafting` | Snapping and tracking calculations. |
| `viboceros-io` | STL, OpenNURBS 3DM bindings, and initial STEP interchange. |
| `viboceros-command` | Document commands plus a separate, document-independent interface-command parser and state reducer. |
| `src/` | egui application and wgpu viewport rendering. |
| `viboceros-oracle`, `tools/rhino_oracle/` | Matching native and public Rhino API probes and a Python comparison client. |
| `third_party/` | Pinned OpenNURBS source and its license. |

Typed coordinates are resolved by `viboceros-drafting/point_input`, independent
of UI and document edits. `app/point_input` shares the interactive command's
point-validation and transaction path with mouse picks; see [point input](point-input.md).
The command crate's `interchange` module owns the six STL/STEP/3DM import/export
commands, document-to-file attribute/group adapters, and export mesh aggregation.
Parsing and file replacement remain in `viboceros-io`; registry transactions
remain responsible for rolling back failed imports. Existing mesh aggregation
uses numerical validation, independently of surface tessellation tolerance.
`CommandContext` carries an explicit construction plane separately from document
history. The [plane-primitives module](construction-planes.md) handles orientation,
projection, and native parameter policy for six primitive commands.
The separate [plane-transforms module](plane-transforms.md) handles reference
interpretation and affine maps for five transforms, sharing document mutation
and copy/group transactions with the other transform commands.
The document's independent `groups` module owns ordered object memberships and
their reverse member index. Lightweight membership history records preserve
order without cloning geometry; see [group invariants](groups.md).
The independent [arrays module](plane-arrays.md) owns rectangular, linear, and
polar layout. Its geometry extents use tolerance-controlled
[curve](curve-bounds.md), [surface](surface-bounds.md), and
[trimmed-face bounds](trimmed-face-bounds.md), separately from
document/control-net bounds. The geometry `bezier` module owns local homogeneous
span extraction shared by [Bézier conversion](commands/beziers.md), directional
[single-span conversion](commands/single-spans.md), and bounds. Each conversion
command has its own module and typed [option memory](command-options.md), outside
document history. Shared oracle recording lives in `conversion`; self-seeded
stateful probes live in `conversion_session`, separate from geometry algorithms.
The separate [ToNURBS module](commands/to-nurbs.md) reuses exact curve representation
and mesh-to-B-rep construction. Its replacement order is handled by the document's
`object_order` module, with linear-time permutation replay and lightweight history
containing only moved IDs and original indices. Geometry, memberships, and
selection action order are independent of chronological object order.
[`MeshToNURB`](commands/mesh-to-nurb.md) has a separate command module and option
memory. Geometry connectivity and component extraction live in `mesh/components`;
disjoint splitting and mesh explosion share a single reusable vertex-remap
scratch array, avoiding per-component initialization of a source-sized array.
The command-owned `object_selection` module supplies typed filters and boolean
options to the separate [app object-prompt adapter](object-selection.md).
`selection_commands` owns basic, action-history, attribute, and duplicate
selectors together with their argument parsing and remembered selection options.
The independent `geometry_selection` command module owns document-wide type,
topology, planarity, and short-curve selectors. It preflights eligible matches
before applying additive selection and leaves model history unchanged.
The geometry [shortness module](curve-shortness.md) keeps representation-dependent
selection integration separate from accuracy-controlled arc-length measurement.
The read-only [`measurements` command module](commands/measurements.md) owns
Length, Area, and Volume, with shared streaming exact finite-value aggregation and
separate nonnegative/signed policies. Its tests live beside the implementation.
`curve/area` dispatches enclosed curve area to analytic formulas or the existing
exact planar-face boundary integrator; temporary topology stays outside the document.
Viewport hit-testing applies the filter before hit priority; clicks/windows use
the same selection adapter. Prompt choices live outside model history, while
finishing uses the registry's normal transaction lifecycle with explicit
postselection dispatch. Typed workflows distinguish options during selection from
ToNURBS's separate confirmation and MeshOptions submenu. The command determines
selection-dependent menus; the app owns their phase and pre/postselection origin.
Confirmation disables object picking without enabling point drafting. Nested CPlane
input and display controls retain the current phase. Explicit ordered document
renewal and ordered copies support pick-dependent creation order; inverse renewal
uses adjacent cycle swaps with the same single scratch permutation as forward replay.
The `bounds/bezier` module handles bounded tensor subdivision and rational hulls;
curve/surface adapters select the complete native active spans.
`bounds/bezier/compose` constructs homogeneous rational surface/UV-curve
compositions. `bounds/parameter_curves` bounds their exact spatial images and
[B-rep trim boundaries](trim-boundary-bounds.md), separately from face interiors.
`bounds/trimmed_surfaces` adds a stationary-extremum search; `bounds/trim_region`
uses rational curve hulls for conservative inside/outside/uncertain tests.
`Geometry::tight_bounds` centralizes dispatch without changing fast display bounds.
The independent [BoundingBox command](commands/bounding-box.md) uses that query
in a geometry-local working frame, then stages oriented solids, quad meshes,
or grouped rectangle outputs before entering document mutation.
The separate [Distribute command](commands/distribute.md) shares its
`object_bounds` working-frame query, then stages group-aware in-place
translations. Its option parser and logical-unit preflight are shared with the
application's two-point interaction; geometry code does not depend on egui.
`viboceros-command::interface` owns [display/drafting controls](commands/interface.md).
The application adapter applies those actions without entering the modeling
command lifecycle; the compact toolbar and keyboard shortcuts share that path.
The headless oracle exercises the same reducer, independently of egui and wgpu.
`viboceros-command::construction_plane` owns validated [CPlane edits](cplane.md)
and viewport-local history. The drafting plane module performs local grid,
ray-intersection, and projected tracking queries. Each viewport stores its frame
separately from camera state; the app's nested plane prompt does not replace its
active modeling prompt.

## Current foundation

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
and use a balanced XY spatial index for snapping and picking.

Implementation is incomplete. General surface/surface
intersections, editable STEP B-reps,
and much of Rhino's command set remain to be implemented. See the
[command reference](commands/README.md) and [file-format documentation](file-formats.md)
for capability boundaries. Rhino Render is outside the project scope.

B-rep area and volume live in `viboceros-geometry/src/brep/mass_properties/`
and its parent module. [Trimmed-domain integration](mass-properties.md) uses
exact NURBS boundaries and adaptive quadrature, including nonplanar faces and holes.
[Boundary validation](brep-validation.md) in `brep/validate` checks shared topology
and bidirectional edge/trim correspondence; matching endpoints alone do not
establish a valid face boundary.
The [morph assembler](brep-morphing.md) in `brep/morph` retains shared topology
and exact UV trims. `brep/trim_image` supplies composed-curve correspondence to
validation and [B-rep meshing](brep-meshing.md). `brep/tessellation` separates
independent face sampling, source-face boundary audits, and conforming
reconstruction; naked-edge provenance covers open shells as well as solids.

The [polycurve kernel](polycurves.md) preserves independent exact segments and
parameter maps. Document geometry operations live in their own module, separate
from object state and history. Polycurves are integrated with transforms, rendering,
picking, endpoint snapping, extraction, explode, and 3DM interchange.
Representation-aware ownership, endpoint editing, and joining live in separate
geometry modules; `Join` and `CloseCrv` share a dedicated command module.
See [curve joining and closure](curve-editing.md) for tested policies and limits.
All seven curve families share [native parameter evaluation](curve-parameters.md),
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
The `curve_parameter_map` geometry module supplies exact span-aware correspondence
with rational representations; the `curve_cut` command module uses it for
[cutting-object splits and trims](curve-cutting.md).
NURBS differential evaluation and homogeneous weight matching have separate
`nurbs/evaluate` and `nurbs/weights` modules. See [rational numerical policy](nurbs-numerics.md)
for local-coordinate evaluation, degree-one acceleration, and scale-safe seam joins.
`nurbs/arc_length` owns accuracy-controlled full-curve length integration.
`nurbs/integration_frame` supplies checked dimensionless parameter preparation
shared by length, sampling, shortness, and enclosed curve-area queries.
`nurbs/weights/end_weights` owns projective endpoint normalization and
piecewise-Bezier end-weight changes, including near-equal and extreme gauges.
Loft and Sweep share its geometry-preserving normalizer; explicit common-profile
knot policies and Rhino's measured near-equal-weight drift remain separate.
The shared [one-sided evaluator](curve-sided-evaluation.md) propagates side choices
through composite leaves and supplies exact kink checks and stationary tangents.
The [curve-frame module](curve-frames.md) separates span-aware adaptive tangent
transport from frame construction. `ArrayCrv`, swept spirals, and `Sweep1` share it;
corner-side policy and remaining Rhino corner differences are explicit.
The `nurbs_surface/evaluate` module shares `ParameterSide` and nonempty knot-span
selection with curves. Its [surface jets](surface-evaluation.md) use local rational
coordinates and expose exact U/V limits and boundary-span continuation.
The separate [curvature module](curvature.md) forms a scale-aware orthonormal
shape operator from those jets. Geometry evaluation is independent of the
command's closest-point selection, reports, and permanent markers.
The [curve fitter](curve-morphing.md) in `morph/curve_fit` and
[surface fitter](surface-morphing.md) in `morph/surface_fit` are separate from
point-map construction. They share sided cubic banded interpolation in
`spline_collocation`, retain source knot limits, and explicitly fail when
sampled fitting tolerance cannot be reached within their resource budgets.
Both fitters also check bounded rational composition candidates:
[curves](curve-rational-fitting.md) and [surfaces](surface-rational-fitting.md).
`nurbs2/evaluate` provides stable, sided UV-trim evaluation independently of
model-space curves, including exact constant parameter coordinates.

The [loft kernel](loft.md) and [one-rail sweep](sweep1.md) share exact degree/knot
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
The [edge-surface constructor](edge-surfaces.md) separately handles boundary
ordering, pairwise parameter compatibility and homogeneous Coons blending.
Unlike Loft's common-basis policy, three/four-edge endpoint-weight normalization
precedes pair matching. Exact degree elevation retains finite patches whose
original Coons basis contains a projective control at infinity.

The [point-grid module](point-grid-surfaces.md) separates tensor interpolation,
open/periodic basis policy, and polynomial shell-orientation integration.
Command adapters preserve Rhino's input order while the kernel retains U-fast
storage. Complete face records distinguish API tensor construction from
command-level shared crease topology.

Tests cover numerical operations, topology, document state, commands, import/export,
and UI interactions. Passing the current suite is evidence for those cases,
not a proof of full Rhino compatibility. The [oracle](oracle.md) checks public
Rhino outputs and records timing independently of startup and fixture setup.
The dedicated `brep_interchange` oracle module checks [morphed 3DM exports](brep-3dm-interchange.md)
through both readers, keeping source fitting, serialization and meshing checks separate.
