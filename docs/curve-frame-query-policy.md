# Frames at sampled corners

`ArrayCrv` now uses a separate query-boundary policy over the shared adaptive
curve-frame integrator. This fixes the original axis-aligned spatial corner
case while keeping the kernel's density-independent minimum-twist contract.
The command API and viewport preview use the same placement solver.

The first frame uses the outgoing tangent and a curvature normal when one is
available. A straight start prefers an almost-perpendicular coordinate direction
in Z, X, Y order (dimensionless threshold `sqrt(f64::EPSILON)`), then the common
normal-derived frame. Every later returned query uses the incoming limit.
Before continuing from a query, the carried X axis is projected into the outgoing
tangent's normal plane by two cross products. Corners crossed between queries
still use their shortest tangent rotation. These are distinct operations: an
extra query at a corner can change the final orientation.

The positive-side continuation follows the public OpenNURBS
`ON_TuneupEvaluationParameter` convention: a parameter in the final `1e-10` of
its active span can continue from that span's endpoint. The sampled origin and
incoming axes retain their original parameter. No guide geometry is edited.
A singular projection cannot seed the next interval; public SDK observations
show the next returned frame is freshly seeded from that query's tangent,
including when the interval is curved.

The geometry API exposes an independent seed side and an optional query restart
hook. `FrameQueryRestart.query_x` edits returned axes and `outgoing_x` edits
continuation, each projected into its respective normal plane. The default hook
preserves the existing transport. Span traversal, adaptive integration and its
resource counter are shared in one pass. Callback evaluations are reserved in
the command's evaluation budget. Ambiguous antiparallel corners and position
jumps retain the kernel's explicit errors.

The implementation was derived from public
[GetPerpendicularFrames results](https://developer.rhino3d.com/api/rhinocommon/rhino.geometry.curve/getperpendicularframes),
actual ArrayCrv outputs, and the licensed OpenNURBS source already in the
repository. No proprietary implementation was inspected.

## Evidence and remaining differences

The [receipt](curve-frame-query-validation.json) pins producer and implementation
sources and retains every observation. SDK captures cover native polylines,
degree-one and elevated cubic NURBS, polycurves, curved cubic chains, queried and
unqueried corners, near-parallel projections, parameter rescaling and near-zero
seed directions. Actual ArrayCrv captures additionally check source attributes,
selection, group definitions and memberships, and real Undo/Redo. Both API and
UI replays include all earlier 62 workflows and 60 added corner workflows.
The independent kernel tests retain Bishop-ODE agreement, scale and translation
invariance, closed holonomy, reversal, resource failures and density invariance.

Some orientations remain unqualified:

- Eighteen added arrays hit a **projection pole** on rotated right-angle paths.
  The carried axis is parallel to the next tangent. Native floating-point
  roundoff then supplies an arbitrary direction: inserting a midpoint query on
  a straight span changes Rhino's final rotation substantially. These observations
  are retained as diagnostics, with all document/history assertions active.
- The SDK's PolyCurve parameter round-trip can turn a one-ULP query above a
  corner into the exact corner. Its equivalent NURBS has different axes for the
  identical recorded geometry and parameter. The reconstruction test reports
  this representation-specific boundary diagnostic separately.
- Smooth native frame integration has the previously measured approximation
  error. One exact curved-corner SDK batch exceeds `1e-6`; its diagnostic bound
  is `1e-5`. The earlier closed Freeform array remains bounded at `5e-5`.
- Four earlier vertical Roadlike/Stairlike observations still contain collapsed
  native maps. Grip sources, general trimmed-edge topology, automatic closed-seam
  ties and pathological curves remain outside this capture's qualification.

The diagnostic cases are not counted toward the strict `1e-6` geometry results.
Captures are untimed. Controller tests exercise read-only instance previews and
component routing; they do not compare rendered pixels or establish a speed
comparison against native Windows Rhino.
