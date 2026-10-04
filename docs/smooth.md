# Smooth kernel and native evidence

[Architecture](architecture.md) · [Oracle testing](oracle.md)

The geometry crate implements synchronous control-point and mesh-vertex
averaging through `NurbsCurve::try_smoothed`, `NurbsSurface::try_smoothed`, and
`TriangleMesh::try_smoothed`. These immutable methods accept `SmoothingOptions`,
an orthonormal `Frame3`, and an optional set of selected grip indices. They
support World and construction-plane axes. **Smooth is not yet registered as
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
  closed and periodic polygons wrap through the unique controls. Fixed boundaries
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
- Rust replays compare the full control coordinates, weights, knots, degrees,
  domains and retained mesh faces in those 128 cases. The absolute comparison
  limit is `2e-12` in the captured units.
- Python checks verify native command-end, post-macro, Undo and Redo snapshots,
  primitive promotion, selection and parent/grip precedence. These are retained
  Rhino observations; the geometry replays do not establish Viboceros document
  or application behavior.

The source and observation hashes, capture schemes, and replay scope are in
[`smooth-provenance.json`](smooth-provenance.json).

```sh
cargo test -p viboceros-geometry --release smoothing::tests
python3 -m unittest tools.rhino_oracle.test_smooth
```

## Object coordinates and remaining work

`smooth_object.json` retains 80 native Object-coordinate recipes, including
separate X/Y/Z axes and repeated steps. These are research observations, not
completed kernel or application replays.

`smooth_frames.json` retains 18 independent public SDK witnesses for single-pass
Object X smoothing. The successful witnesses suggest these rules:

- Curve tangents are evaluated at each control's Greville parameter on the
  progressively edited curve. `TangentAt` supplies the endpoint-span
  extrapolation needed when periodic Greville parameters lie outside the domain.
  Native Object edits can separate repeated periodic control aliases.
- Surface directions are evaluated on the progressively edited surface in
  U-major order. Orthogonalizing U against V with `V cross N` matches the native
  X results. Using U directly or traversing in V-major order does not.
- Mesh frames use the original vertex normals for the pass and the public plane
  construction from a normal. Recomputing normals after each vertex does not
  match these captures.

These inferences match the captured X cases; they do not establish all Object
axes, multiple-step behavior, singular frames, or general geometry. The
witnesses use only public
[curve UVN directions](https://mcneel.github.io/rhinocommon-api-docs/api/RhinoCommon/html/M_Rhino_Geometry_NurbsCurve_UVNDirectionsAt.htm),
[surface UVN directions](https://mcneel.github.io/rhinocommon-api-docs/api/RhinoCommon/html/M_Rhino_Geometry_NurbsSurface_UVNDirectionsAt.htm),
and [mesh smoothing](https://developer.rhino3d.com/api/rhinocommon/rhino.geometry.mesh/smooth?overload=2).
The user-facing options are described in the
[official Smooth help](https://docs.mcneel.com/rhino/8/help/en-us/commands/smooth.htm).

Object-coordinate implementation, analytic/polycurve promotion, general trimmed
B-rep reconstruction, SubD, edit-point and subobject workflows, command option
memory/cancellation, UI integration, and performance comparisons remain open.
Full Rhino parity is unproven.
