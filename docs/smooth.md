# Smooth kernel and native evidence

[Architecture](architecture.md) · [Oracle testing](oracle.md)

The geometry crate implements synchronous control-point and mesh-vertex
averaging through `NurbsCurve::try_smoothed_in`,
`NurbsSurface::try_smoothed_in`, and `TriangleMesh::try_smoothed_in`.
These immutable methods accept `SmoothingOptions`, `SmoothingCoordinates`
(World, CPlane with an orthonormal `Frame3`, or Object), and an optional set of
selected grip indices. `try_smoothed` accepts a fixed frame directly.
**Smooth is not yet registered as
a Viboceros command.** Document transactions and the interactive workflow remain
to be implemented.

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
  primitive promotion, selection and parent/grip precedence. These are retained
  Rhino observations; the geometry replays do not establish Viboceros document
  or application behavior.
  In 36 selected Object recipes, separating closed/periodic aliases increases
  the displayed grip count and clears grip selection at command end. Grips
  remain enabled, and Undo restores the original controls and selection.

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

## Remaining work

The retained families do not establish singular-frame or extreme-range native
behavior, general geometry, or application behavior.
Analytic/polycurve promotion in the document, general trimmed
B-rep reconstruction, SubD, edit-point and subobject workflows, command option
memory/cancellation, UI integration, and performance comparisons remain open.
Full Rhino parity is unproven.
