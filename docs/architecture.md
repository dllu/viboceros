# Architecture and implementation status

[Project overview](../README.md)

The dependency direction runs from mathematical primitives through document
state and commands to the user interface. Geometry does not depend on the UI,
file formats, or command parsing.

| Module | Responsibility |
| --- | --- |
| `viboceros-geometry` | Validated primitives, NURBS, intersections, B-rep topology, tessellation, and mass properties; nalgebra and faer provide linear algebra. |
| `viboceros-document` | Objects, attributes, layers, groups, selection, transactions, and bounded undo/redo. |
| `viboceros-drafting` | Snapping and tracking calculations. |
| `viboceros-io` | STL, OpenNURBS 3DM bindings, and initial STEP interchange. |
| `viboceros-command` | Command registration, argument parsing, and document operations. |
| `src/` | egui application and wgpu viewport rendering. |
| `viboceros-oracle`, `tools/rhino_oracle/` | Matching native and public Rhino API probes and a Python comparison client. |
| `third_party/` | Pinned OpenNURBS source and its license. |

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
Native trim, split, closest-point dispatch, and cyclic edits live in `curve_trim`;
seam, subcurve, and reparameterization commands share the `curve_domain` module.
The `curve_parameter_map` geometry module supplies exact span-aware correspondence
with rational representations; the `curve_cut` command module uses it for
[cutting-object splits and trims](curve-cutting.md).
NURBS differential evaluation and homogeneous weight matching have separate
`nurbs/evaluate` and `nurbs/weights` modules. See [rational numerical policy](nurbs-numerics.md)
for local-coordinate evaluation, degree-one acceleration, and scale-safe seam joins.
The shared [one-sided evaluator](curve-sided-evaluation.md) propagates side choices
through composite leaves and supplies exact kink checks and stationary tangents.
The `nurbs_surface/evaluate` module shares `ParameterSide` and nonempty knot-span
selection with curves. Its [surface jets](surface-evaluation.md) use local rational
coordinates and expose exact U/V limits and boundary-span continuation.
The [curve fitter](curve-morphing.md) in `morph/curve_fit` and
[surface fitter](surface-morphing.md) in `morph/surface_fit` are separate from
point-map construction. They share sided cubic banded interpolation in
`morph/interpolation`, retain source knot limits, and explicitly fail when
sampled fitting tolerance cannot be reached within their resource budgets.
The surface fitter also checks a bounded [rational composition candidate](surface-rational-fitting.md).
`nurbs2/evaluate` provides stable, sided UV-trim evaluation independently of
model-space curves, including exact constant parameter coordinates.

Tests cover numerical operations, topology, document state, commands, import/export,
and UI interactions. Passing the current suite is evidence for those cases,
not a proof of full Rhino compatibility. The [oracle](oracle.md) checks public
Rhino outputs and records timing independently of startup and fixture setup.
The dedicated `brep_interchange` oracle module checks [morphed 3DM exports](brep-3dm-interchange.md)
through both readers, keeping source fitting, serialization and meshing checks separate.
