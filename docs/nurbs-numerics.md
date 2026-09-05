# Rational curve evaluation and homogeneous scales

[Architecture](architecture.md) · [Curve parameters](curve-parameters.md) · [Oracle](oracle.md)

NURBS point and derivative evaluation lives in `nurbs/evaluate.rs`; homogeneous
weight matching for concatenation lives in `nurbs/weights.rs`. These operations
preserve the native parameterization, not just the geometric locus.

## Differential evaluation

For homogeneous coordinates `(H(t), W(t))`, the Euclidean curve is `C = H/W`.
The evaluator applies de Boor to the homogeneous control polygon and its exact
derivative polygons, then applies the rational quotient rule:

```text
C'  = (H' - C W') / W
C'' = (H'' - 2 C' W' - C W'') / W
```

A degree-one rational curve lies on a line but need not have constant speed.
Although `H'' = W'' = 0`, its second derivative is `-2 (W'/W) C'`, generally
nonzero. For controls `0, 4`, weights `1, 2`, and domain `[0,1]`, the exact
formulas are `C(t)=8t/(1+t)`, `C'(t)=8/(1+t)^2`, and
`C''(t)=-16/(1+t)^3`. Returning zero acceleration solely because the degree is
one is incorrect; straightness concerns curvature, not parameter acceleration.

Each active control polygon is evaluated relative to its first control point.
This removes the large common coordinate offset before the quotient rule,
avoiding cancellation between `H'` and `C W'`. Only the returned point has its
world origin restored. Interpolated span endpoints return their exact control
point, including degree-multiple and full-order knots. Closest-point refinement
also translates the curve and target into one common frame before comparing
distances, so world-coordinate rounding does not quantize its objective.

An overflowing control-point difference uses the unshifted evaluation frame.
Signed-weight curves can leave their control hull; if a local result overflows,
evaluation retries without centering because the world-space result may still
be finite. An actual zero homogeneous denominator remains an error.

## Concatenation and seam relocation

Matching adjacent endpoint weights requires computing `weight * to / from`.
The implementation tries finite multiplication/division orderings instead of
requiring the intermediate ratio `to/from` to be representable. It preserves an
exact match when a control already has weight `from`.

NURBS concatenation, wrapped subcurves, seam relocation, and polycurve conversion
share this rescaling helper. When every resulting weight remains finite and
nonzero, adjacent spans share a control and a degree-multiple knot. Seam relocation
retains the preceding piece's endpoint, consistent with the licensed OpenNURBS
`Append` implementation. The existing concatenation/polycurve policy instead
uses the coincident endpoints' midpoint.

If a common scale would overflow or erase a weight, both original endpoint
controls and a full-order junction knot are retained. This preserves the original
pieces and their independent scales; it does not force endpoint averaging or
promise Rhino's minimal control structure. Tests cover scales `2^-700` and
`2^700`, opposite global signs, genuine nonrepresentable common scales, and
cyclic seam/subcurve edits.
OpenNURBS rejects internal full-order knot groups; direct 3DM NURBS export of
these fallback representations currently reports an invalid-curve error.
Interchange needs a decomposition into valid pieces before these kernel results
can be written. See [one-sided limits](curve-sided-evaluation.md) for native
positional-jump tests and the observed Rhino input rejection.

## Validation and limits

`nurbs_rational_jets.json` contains 23 public Rhino API comparisons: unequal-weight
degree-one, quadratic, and cubic curves; shifted domains; reversal; tiny/huge
global weight scales; and closed seams inside and outside the original domain.
Each result includes 33 native point/first/second-derivative/tangent samples,
18 arc-length stations, length, domain, closure, and the full rational definition.
All passed against Rhino 8 at absolute `1e-8`, relative `1e-10`. Geometry and
parameter differences were below `4.2e-11`; enormous homogeneous weights are
compared relatively rather than included in that absolute geometry bound.

```sh
tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/nurbs_rational_jets.json \
  --absolute-epsilon 1e-8 --relative-epsilon 1e-10
```

`nurbs_translated_jets.json` is a separate diagnostic, **not a passing Rhino
reference** at that epsilon. It translates the same three curve degrees by
`(1e12,-2e12,3e12)`. Native derivatives and tangents are bit-for-bit unchanged
at the sampled parameters; Rhino's public evaluations show translation errors:

| Degree | Maximum first-derivative error | Maximum second-derivative error | Maximum tangent error |
| --- | ---: | ---: | ---: |
| 1 | `3.06e-5` | `2.45e-6` | `1.48e-4` |
| 2 | `3.06e-5` | `5.73e-6` | `1.76e-4` |
| 3 | `1.40e-4` | `1.10e-4` | `3.37e-4` |

These are maximum component differences from each engine's untranslated results,
observed in the licensed Rhino 8 Wine/FEX installation. Even the horizontal line
gets a nonzero Y derivative in Rhino. Native translated derivatives agree with
Rhino's **untranslated** derivatives within `1.4e-15`. Native point coordinates
equal the untranslated result plus the offset; Rhino's points differ by up to
`4.89e-4`, comparable to coordinate rounding at this magnitude.

These three operations set `differential_only=true`, omitting both length and
division fields. Rhino's normalized-length inversion failed on the translated
line at the requested `1e-12` relative length tolerance. This mode isolates
differential evaluation; it does not substitute guessed length results. Native
tests separately retain full length/division checks and compare them to the
untranslated curves. Run the diagnostic with the runner's `rhino` subcommand
to capture raw results, or `compare` to inspect the expected discrepancies.

Analytic unit tests additionally check the degree-one formulas, constant curves
far from the origin, signed-weight poles, finite world points whose local offsets
overflow, and exact interpolated endpoints. Negative global weights remain covered
in native tests: this oracle's public Rhino control-point setter rejected them.

The kernel still uses `f64`, not extended-exponent arithmetic. Active-weight
normalization can underflow when weights within one span have an unrepresentable
dynamic range; derivative intermediates can also exceed the numeric range even
when a final mathematical answer is finite. The separate-span fallback does not
solve those within-span limitations. Sampling and passing tests are bounded
evidence, not a universal error proof or full Rhino compatibility claim.
