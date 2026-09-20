# UV evaluation and parameter-space range

[Numerical policy](nurbs-numerics.md) · [Curve recovery](curve-rational-range.md) · [B-rep meshing](brep-meshing.md)

Two-dimensional NURBS trims now recover points and native first derivatives
after detected homogeneous preparation range loss or a reported floating-point
evaluation failure. Ordinary successful evaluation keeps its centered,
weight-normalized floating-point path.

## Reproduced failures

Before the change, native regressions reproduced:

- A quadratic UV coordinate returning zero instead of approximately
  `3.4175792574734558e97` with controls `MAX, 0, 0`, weights
  `2^-700, 1, 2^700`, and parameter `2^-350`.
- A rational line with U controls `0, a` and weights `1, a`, `a=1e-200`,
  returning endpoint derivative zero instead of one.
- Constant curves falsely reported as poles after signed cancellation, both
  with extreme weights and with weights `1, 2^-100, -1` whose preparation
  did not itself lose range.
- A constant curve on a minimum-subnormal parameter interval failing because
  intermediate homogeneous derivatives overflowed, despite a zero final jet.

`nurbs2/evaluate/exact` uses the shared, dimension-generic homogeneous recurrence
in `nurbs/exact`. It converts the original stored binary64 values to exact
rationals before multiplication or division and rounds only requested final UV
components. It does not promote the curve to 3D or request unneeded higher jets.
Interpolated endpoints retain their stored coordinate bits, including signed zero.
Both knot sides retain their native spans. Invalid parameters, genuine poles,
and genuinely unrepresentable requested results remain errors. Source geometry
is unchanged.

## Independent parameter axes

The boundary-jet regression also exposed two separate B-rep defects. A unit
square with U domain `[0, 1e-200]` and V domain `[0.4, 1.4]` was rejected as
having no stable UV area. After fixing that, an interior point was still
classified outside because a unit-floored scan tolerance merged opposite edges.

`brep/parameter_normalization` now centers and scales U and V independently.
Each axis uses direct differences when possible and scaled subtraction when a
difference overflows. A constant axis stays exactly zero. Positive independent
axis scales preserve orientation and containment without allowing the larger
parameter range to erase the smaller one. Loop winding, polygon validation,
triangulation, and grid inclusion share the helper. Nonuniform scaling can change
the chosen triangulation; no identical-triangle-layout guarantee is intended.

Scan and interval tolerances now scale with interval **width**, not its origin
or an arbitrary unit floor. Overflowing widths are scaled before subtraction.
An underflowed epsilon becomes zero, merging only identical representable
parameters. Event and root midpoints use the overflow/underflow-safe binary64
midpoint operation. This does not widen model-space tolerance or surface domains.

## Validation

The UV reference test reuses X/Y components of the 127 checked-in independent
Python `Fraction` B-spline basis-sum references. Public point and first-derivative
results are checked bit-for-bit; unrequested Z and second derivatives do not
affect success. Coverage includes degrees 1–8, multiple/unclamped/full-order-split
spans, both knot sides, signed weights, extreme and translated domains, poles,
and actual requested-output overflow. Eight additional analytic tests cover the
reproductions, constant boundaries, endpoint bits, invalid parameters, and source
immutability.

Normalization tests cover both axis orders, every rectangle origin corner,
opposite `MAX` coordinates, adjacent representable large coordinates,
minimum-subnormal extents, translation/scale covariance of interval tolerances,
and coincident/collinear inputs. Two B-rep integration tests check a recovered
boundary derivative against its 3D edge and preserve containment, holes,
orientation, area, closed naked boundaries, and perimeter through both tessellation
and polygon meshing on anisotropic UV domains. The holed-face cases exercise both
thin-axis choices, translated unit-width domains near `1e6`, and simultaneous
parameter scales of `1e200` and `1e-200`.

```sh
cargo test -p viboceros-geometry nurbs2::evaluate::tests
cargo test -p viboceros-geometry brep::parameter_normalization
cargo test -p viboceros-geometry brep::tessellation::tests::range
```

A fresh licensed Rhino 8 run in private Xvfb passed all
[five mesh-boundary fixtures](uv-rational-range-mesh-rhino-comparison.json),
[six trimmed mass-property fixtures](uv-rational-range-mass-rhino-comparison.json),
and [two pretrimmed surface-split fixtures](uv-rational-range-split-rhino-comparison.json)
at their existing documented comparison tolerances. Mesh records match exactly;
the largest mass-property difference is about `1.21e-9`, below its `1e-8` absolute
comparison tolerance. These ordinary-input comparisons are separate from the
extreme-weight exact references and do not prove full Rhino parity.

The full workspace passed **2,484 Rust tests**, with 18 intentionally ignored,
and all **178 Python tests**. The expanded holed-domain cases also passed a
follow-up run. Strict Clippy, formatting, and deterministic regeneration of the
shared curve/UV reference data passed.

## Performance

The [release measurement record](uv-rational-range-performance.json) compares
baseline `cb3e6b6` against this change in three rounds of 100 iterations. All
15 mesh records and 18 mass-property records retain identical binary64 results.
Median meshing times change by **-4.1% to +0.9%**; mass-property times change by
**-1.2% to +0.3%**. Construction and command/record checks are outside the timed
sections. Other test/build/oracle jobs were active, and the raw round timings
include outliers; these small differences are not evidence of a speedup or
statistical significance. They do not measure exceptional exact-evaluation cost.

The 127-case exact UV reference test took 17.20 seconds in debug and 0.44 seconds
in release, including input construction, both requested orders, error checks,
and bit comparisons. These are whole-test timings, not per-evaluation benchmarks.

## Limits

Exact recovery has input-dependent arbitrary-precision time and memory costs.
The preparation guard and failure retry do not certify every finite unflagged
floating-point result. This is not a universal correctly rounded UV evaluator.
Scalar trim-root coefficient construction, root isolation, structure edits,
and other homogeneous operations do not automatically inherit the exact policy.
Topology predicates and triangulation remain floating-point, tolerance-based
algorithms; independently scaling axes does not prove robustness for every
nearly coincident or self-intersecting loop.

A further diagnostic with unit-width UV domains near `1e12` still fails
edge-to-trim correspondence validation. `LiftedTrim` rounds the evaluated UV
coordinate before surface evaluation; native UV spacing at that origin is
already about `1e-4`, too coarse for the default model-space tolerance on a
unit-size face. Exact UV output rounding does not fix this composed-evaluation
loss. The domain-normalization helper handles those origins, but that alone
does not make every downstream B-rep operation translation-invariant.
