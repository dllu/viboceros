# Curve frame transport

[Architecture](architecture.md) · [Curve parameters](curve-parameters.md) · [Oracle](oracle.md)

`CurveRef::rotation_minimizing_frames` transports a perpendicular, right-handed
frame along all seven native curve families. `ArrayCrv` and swept `Spiral`
share this geometry implementation. It is a foundation for future sweep
construction; `Sweep1` is not implemented by this module.

## Contract and numerical method

The first requested parameter seeds the frame. Parameters must be finite,
strictly increasing, nonempty, and inside the native curve domain; they need
not cover the entire curve. A single parameter returns the seed frame.
An optional initial X direction is projected into the tangent's normal plane;
otherwise the seed is deterministic. A direction parallel to the tangent is
an error. Frame Z follows the unit tangent and Y completes the orientation.

`FrameTransportOptions` sets an angular error target (default `1e-10` radians),
a curve-evaluation budget (default 131,072), and the exact requested side
(default `ParameterSide::Right`). Both sides of a corner are traversed when
continuing beyond it, regardless of which side is returned at that parameter.
Non-antiparallel corners use the shortest tangent rotation. Antiparallel
corners and position jumps are rejected because no unique transport exists.
At structurally discontinuous joins, unequal endpoint coordinates are a jump,
even if a composite's joining tolerance admitted that gap. Ordinary
lower-multiplicity NURBS knots do not require bit-identical left/right points.

The `curve_frame` module separates traversal and frame construction from span
break selection (`breaks`) and numerical transport (`transport`). It uses only
unit tangents from the shared [one-sided evaluator](curve-sided-evaluation.md):
no subtraction of world-space chords and no division by parameter speed.
Valid stationary endpoints use the evaluator's limiting tangent.

Shortest great-circle rotations on the tangent sphere provide the basic
second-order method. A four-level Richardson table combines 1, 2, 4, and 8
steps into an eighth-order candidate. Corrections use signed twist angles
around the common endpoint tangent, keeping axes orthonormal. The difference
between the two sixth-order entries estimates error. The total angular
budget is apportioned across native parameter intervals; failed estimates
trigger bounded bisection. Knot spans, composite joins, and polyline vertices
are explicit integration boundaries. Natural line/polyline spans need only
their exact corner rotations, without adaptive integration.

This is an adaptive error estimate, not a certified continuous bound. Evaluation
and recursion limits, nonrepresentable parameter subdivisions, degenerate
tangents, and failure to converge return errors. Ill-conditioned rational
curves and unsampled tangent oscillations remain finite-precision/adaptive
sampling limitations. Closed spatial paths retain their genuine accumulated
twist (holonomy); the final frame is not artificially forced to equal the seed.

## Independent verification

Kernel tests compare a spatial cubic against independently integrated Bishop
frame differential equations and verify eighth-order convergence against an
analytic helix. They cover planar normal invariance, initial-axis gauge changes,
reversal, sparse/dense queries, exact left/right corners, positional jumps,
stationary endpoints, resource failures, and closed-loop holonomy.
Scale tests include `1e±140` geometry, large parameter domains, and a
`[1e12, -2e12, 3e12]` translation with bit-identical native tangents and rotations.
A regression test prevents rounding differences at an ordinary C2 knot from
being mistaken for a position jump.

## Rhino comparisons and remaining differences

The public [RhinoCommon GetPerpendicularFrames API](https://mcneel.github.io/rhinocommon-api-docs/api/RhinoCommon/html/M_Rhino_Geometry_Curve_GetPerpendicularFrames.htm)
supplies the reference. Records contain native parameters, points, tangents,
and the relative rotation `F(t) F(first)ᵀ`. This removes the arbitrary initial
perpendicular-axis choice for rotation-minimizing transport. Coordinates are
not rounded. The native probe requests left limits, matching Rhino's later
exact-corner queries in the tested batches. All Rhino runs used a private Xvfb
display and public APIs or actual commands, without proprietary code inspection.

| Fixture | Cases | Absolute comparison limit | Observed maximum difference |
| --- | ---: | ---: | ---: |
| `curve_frames.json` | 21 | `1e-6` | `7.30e-7` |
| `curve_frames_multispan.json` | 1 | `1e-5` | `7.59e-6` |
| `curve_array.json` (seven command scenarios) | 1 | `1e-6` | `1.40e-7` |
| `swept_spiral.json` | 3 | `2e-7` | `8.04e-8` |

All comparisons also use relative `1e-12`. The 22 frame cases' points and
tangents are checked separately at absolute `2e-12` plus relative `1e-12`;
their observed maximum difference is `3.11e-15`. The looser rotation limits
are not geometry-evaluation tolerances. On the spatial cubic, changing from
17 queries to two changes native endpoint rotation by `1.43e-12`, versus
`1.96e-7` in Rhino. Native integration is independently checked at its tighter
target rather than tuned to Rhino's output-density error.

Two diagnostic fixtures deliberately do **not** pass those comparison limits:

- `curve_frames_diagnostics.json` has seven cases. Rhino returns no batch for
  a single query or the tested stationary endpoint; native frames are valid.
  A large translation changes Rhino's tangents/rotations while native axes
  remain identical. The other cases expose exact-corner behavior, including
  Rhino using the outgoing tangent when a corner is the first query but the
  incoming tangent when it is a later query.
- For the path `(0,0,0) → (1,0,0) → (1,1,0) → (1,1,1)`, querying its exact
  interior corners changes Rhino's final perpendicular axes by 90 degrees
  compared with querying only the endpoints or avoiding the corners. This
  occurs for polyline, polycurve, and degree-one NURBS representations.
  `curve_array_corner_diagnostics.json` also reproduces the mismatch with an
  actual four-item `ArrayCrv`, including grouped axis witnesses and selection.
  It is an untimed state probe. Native `ArrayCrv` uses incoming corner tangents
  but currently retains stable minimum-twist transport; Rhino's query-dependent
  corner restart is an unresolved compatibility difference.

```sh
tools/rhino_oracle/run_headless.sh compare tools/rhino_oracle/fixtures/curve_frames.json \
  --absolute-epsilon 1e-6 --relative-epsilon 1e-12
tools/rhino_oracle/run_headless.sh compare tools/rhino_oracle/fixtures/curve_frames_multispan.json \
  --absolute-epsilon 1e-5 --relative-epsilon 1e-12
```

Release measurements on this ARM host put the 17-frame spatial cubic near
0.19 ms natively versus 0.24 ms in Rhino under FEX/Wine. These short, separate
runs exclude construction/startup but are not a statistically controlled
benchmark or a comparison with native Windows Rhino. The existing ordinary
`ArrayCrv` fixture times only its line-division primitive, not repeated command
execution; do not interpret that timing as command performance.
