# Smooth implementation and native evidence

[Command reference](commands/smooth.md) · [Architecture](architecture.md) · [Oracle testing](oracle.md)

The geometry crate implements synchronous control-point and mesh-vertex
averaging through `NurbsCurve::try_smoothed_in`,
`NurbsSurface::try_smoothed_in`, and `TriangleMesh::try_smoothed_in`.
These immutable methods accept `SmoothingOptions`, `SmoothingCoordinates`
(World, CPlane with an orthonormal `Frame3`, or Object), and an optional set of
selected grip indices. `try_smoothed` accepts a fixed frame directly.
The registered `Smooth` command uses atomic document replacements and the
shared object/grip selection flow. Numeric and coordinate options are local to
the prompt until the command completes; canceled choices are discarded.

## Averaging rules

Each pass calculates the neighbor means from the previous positions, then moves
each eligible point toward its mean by the factor along the enabled axes.
Weights do not enter these Euclidean means. Negative factors move away from
the means; factors above one can overshoot. The kernel defaults to factor `0.2`,
one step, all three axes enabled, and fixed boundaries. Factors must be finite
and the step count positive.

- Curves use adjacent control-polygon points. Open endpoints have one neighbor;
  closed and periodic polygons wrap at their raw endpoints using the original
  unique controls. Interior neighbors retain raw indices. Fixed boundaries
  retain open endpoints, the closing seam, or the repeated periodic prefix.
  In particular, a degree-three periodic curve retains its first three controls
  and their aliases when boundaries are fixed.
- Surfaces use the four orthogonal control-net neighbors that exist at each
  point, wrapping through closed or periodic directions. Boundary rows and
  columns follow the same seam rule as curves. Selected indices use U-major,
  V-minor grip order, independently of the surface's V-major storage.
- Meshes use welded topology edges, including triangle diagonals that are stored
  polygon edges. Coincident raw vertices share adjacency; selected raw vertices
  can move independently. Naked-edge vertices can be fixed. The adjacency is
  prepared once for all steps, and vertices without neighbors remain in place.

Adjacency and boundary eligibility are prepared from the source and retained
for every step. This matters for Object coordinates, which can separate seam
aliases and break closure or periodicity during the operation.

The output retains curve/surface degree, knots, domains and weights, and mesh
faces, colors and n-gons. Mesh edits retain collapsed polygon records. Failed
edits leave the source unchanged. Compensated sums and exact arithmetic recover
overflow, cancellation and subnormal cases; the native extreme-range behavior
has not been measured.

## Retained oracle coverage

All live captures ran in empty owned Rhino 8 documents at idle on private Xvfb.
Closed recipe schemas reject script injection and require a private settings
scheme and matching headless display. Each command recipe explicitly seeds all
Smooth options, so it does not depend on remembered preferences.

- `smooth_fixed.json`: 96 native recipes cover 16 curve, surface and mesh
  families, World/CPlane axes, multiple steps, negative/overshoot/zero factors,
  disabled axes, selected grips, parent-plus-grip selection, all grips, and
  command-first selection.
- `smooth_selected.json`: 32 more recipes exercise fixed boundaries and repeated
  steps with partial selections on closed/periodic curves and surfaces and
  unwelded meshes.
- `smooth_object.json`: 80 native recipes cover separate Object X/Y/Z axes,
  all axes, and three steps on the same 16 families.
- `smooth_object_selected.json`: 96 more recipes cover Object X and three steps
  with partial grips, parent-plus-grip selection, and all grips on those families.
- Rust replays compare the full control coordinates, weights, knots, degrees,
  domains and retained mesh faces in all 304 cases. The absolute comparison
  limit is `2e-12` in the captured units.
- Python checks verify native command-end, post-macro, Undo and Redo snapshots,
  primitive promotion, selection and parent/grip precedence. Application replays
  exercise all 304 through selection, options, Enter, post-macro Cancel, Undo,
  and Redo, comparing geometry definitions, names, selection and grip display.
  In 36 selected Object recipes, separating closed/periodic aliases increases
  the displayed grip count and clears grip selection at command end. Grips
  remain enabled, and Undo restores the original controls and selection.

Seven further [public SDK grip witnesses](grip-alias-provenance.json) distinguish
editable closed-seam grouping from exact `ExtractPt` control extraction. Native
grips remain grouped across small endpoint differences and after display is
rebuilt; the read-only extraction API retains distinct endpoint controls.

The source and observation hashes, capture schemes, and replay scope are in
[`smooth-provenance.json`](smooth-provenance.json).

```sh
cargo test -p viboceros-geometry --release smoothing::tests
python3 -m unittest tools.rhino_oracle.test_smooth
```

## Object coordinates

`smooth_frames.json` retains 18 independent public SDK witnesses for single-pass
Object X smoothing. `smooth_uvn.json` adds 76 witnesses for separate X/Y/Z and
three steps. Sixty-four predictions match native commands within `4.45e-16`;
twelve alternatives using canonical aliases or rebuilding closure/periodicity
each step disagree. These captures support the implemented rules:

- Curve tangents are evaluated at each control's Greville parameter on the
  progressively edited curve. X follows the tangent, Y the binormal, and Z
  the in-plane normal. Degree-one curves use World axes. The kernel evaluates
  the nearest nonempty endpoint span's rational continuation when periodic
  Greville parameters lie outside the domain; it does not wrap or clamp them.
  The SDK witnesses use `TangentAt` and `CurvatureAt` for that continuation.
  Native Object edits can separate repeated periodic control aliases.
- Surface directions are evaluated on the progressively edited surface in
  U-major order. Orthogonalizing U against V with `V cross N` matches the native
  X results. Using U directly or traversing in V-major order does not.
- Mesh frames use the original vertex normals for every step and the public plane
  construction from a normal. Recomputing normals after each vertex does not
  match these captures.

Curve and surface means use the positions at the start of each pass, while
frames use geometry progressively edited earlier in the pass. The witnesses
use only public
[curve UVN directions](https://mcneel.github.io/rhinocommon-api-docs/api/RhinoCommon/html/M_Rhino_Geometry_NurbsCurve_UVNDirectionsAt.htm),
[surface UVN directions](https://mcneel.github.io/rhinocommon-api-docs/api/RhinoCommon/html/M_Rhino_Geometry_NurbsSurface_UVNDirectionsAt.htm),
and [mesh smoothing](https://developer.rhino3d.com/api/rhinocommon/rhino.geometry.mesh/smooth?overload=2).
The user-facing options are described in the
[official Smooth help](https://docs.mcneel.com/rhino/8/help/en-us/commands/smooth.htm).

## Document and workflow evidence

`smooth_workflow.json` retains 26 additional native cases for option getters,
preferences, cancellation, admission, object metadata, curve promotion, and
circularly trimmed surfaces. Three cases send a real Escape to the newly owned
Rhino window on the private display. The captures distinguish keyboard Escape
from scripted `_Cancel` and macro interruption: Escape cancels geometry and
pending preferences, while `_Cancel` at the main options getter accepts the
edit. Cancel at a numeric or coordinate subprompt cancels it. Completed options
survive Undo.

Single-face B-reps edit the underlying control net. Rectangular boundaries use
exact isocurves and rebuild seam/pole topology. Other boundaries retain UV trims
and qualify bounded fitted spatial images with a continuous certificate,
allowing up to 4096 controls per image. Every returned boundary has
a continuous normalized-parameter correspondence bound at one quarter of model
tolerance; shared edges receive independent proofs for each trim use.
Exactly constant images require a zero-error certificate before edge removal.
Assembled B-reps must pass ordinary boundary validation; component tolerances
are not enlarged to hide disagreement. The circular-trim capture compares
underlying control definitions and UV curves at `2e-12`, and 33 samples per
spatial edge, using bidirectional nearest-point witnesses at the document
tolerance `1e-7`. Native refitting can change parameterization. Those native
witnesses remain sampled comparisons; the new continuous certificate concerns
the local edited surface and its retained UV trim, not agreement with the native
refit. Spatial edge control equality is not claimed.

The application replays all 26 cases, including metadata, admission, getters,
cancellation, remembered settings, followup invocations, and retained history.
Together with the 304 geometry cases, this makes 330 application replays.

See [workflow provenance](smooth-workflow-provenance.json) for the private
settings scheme, capture source hashes, and replay scope.

## Remaining work

The retained families do not establish singular-frame or extreme-range native
behavior, complete general trimmed topology, SubD, edit-point and subobject
workflows, or performance parity. Full Rhino parity is unproven.
