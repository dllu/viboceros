# Curve range-loss recovery and limiting tangents

[Rational numerical policy](nurbs-numerics.md) · [One-sided limits](curve-sided-evaluation.md) · [Surface recovery](surface-rational-range.md)

Three-dimensional NURBS curves now have a guarded exact-rational path for points,
first/second derivatives, and limiting tangents. Ordinary successful evaluation
still uses the existing centered, weight-normalized floating-point formulas.

## Failures recovered

The audit reproduced these failures before changing the implementation:

- A quadratic with X controls `MAX, 0, 0`, weights `2^-700, 1, 2^700`, and
  parameter `2^-350` returned X=0 instead of approximately `3.4175792574734558e97`.
- A rational line with X controls `0, a` and weights `1, a`, `a=1e-200`, lost
  its weighted coordinate. Its endpoint speed incorrectly became zero instead
  of one, and its finite second derivative was erased.
- A constant quadratic with weights `MAX, MIN_POSITIVE, -MAX` was falsely
  reported as a pole at the midpoint.
- Tiny nonzero speeds and higher-order stationary limits were reported as
  degenerate tangents. Reversed higher-order limits could also acquire an
  incorrect direction from cancellation.
- Even lossless preparation is insufficient: midpoint evaluation of a constant
  quadratic with weights `1, 2^-100, -1` rounded its denominator to zero later
  in the recurrence.

## Dispatch and arithmetic

Active homogeneous preparation flags subnormal/erased normalized weights and
subnormal/erased products of nonzero local coordinates and normalized weights.
Flagged inputs use exact evaluation immediately. Otherwise the floating-point
evaluator retains its existing uncentered retry for overflowing local results;
any remaining reported evaluation failure is resolved by the exact evaluator.
Parameter validation happens first, so nonfinite and out-of-domain requests are
still rejected without entering rational arithmetic.

`nurbs/exact.rs` owns the shared exact homogeneous de Boor recurrence,
derivative-control construction, and final binary64 conversion. Curves and
surfaces share this machinery; their dispatch and Euclidean quotient rules remain
separate. Original stored finite coordinates, weights, knots, and parameters are
converted to `num-rational::BigRational` before multiplication or division. Only
final requested Euclidean components are rounded. Fully interpolated endpoints
retain their exact stored coordinates. No curve is rescaled, edited, or cached.

Independent one-sided limits retain the selected native span, including
unclamped inputs and full-order positional jumps. The fallback evaluates native
parameter derivatives; it does not substitute a unit-domain derivative. Genuine
zero denominators remain errors, including removable poles on constant curves.
A requested output that rounds to infinity remains `NonFinite`; a higher-order
derivative that was not requested does not prevent point-only evaluation.

## Tangent direction before rounding

The floating-point tangent path now has its own small module. If its first
derivative rounds to zero, it asks the exact path to distinguish a tiny regular
speed from a genuinely stationary point. It no longer infers the first nonzero
derivative order from rounded higher derivative polygons.

For exact homogeneous jet `(H,W)` and point `C`, if all lower Euclidean derivatives
vanish, the order-`k` numerator is `H^(k) - C W^(k)`. The exact path examines these
numerators through the curve degree. It scales the first nonzero vector by its
largest absolute component **before** rounding and normalization, retaining
`sign(W)` and the incoming-limit factor `(-1)^(k-1)`. The tangent can therefore
remain representable when the speed underflows or overflows binary64. The point
itself must still be representable. A truly constant span remains degenerate.

This is not a curvature limit solver. Unflagged, nonzero floating-point derivative
results are not universally certified against the exact curve.

## Validation

The deterministic generator uses Python `Fraction` with an independent sum of
B-spline basis functions and their analytic derivatives, rather than evaluating
the control polygon with de Boor. Its 127 checked-in cases cover degrees 1–8,
clamped Bezier, multi-span, unclamped, and full-order-split curves; both knot
sides; positive, common-negative, and mixed signs; weights `2^-1000` to `2^1000`;
varied coordinate scales; translated and extreme parameter domains; and a true
pole. Public point/first/second-derivative methods are checked bit-for-bit for
every requested finite component, with overflow and pole errors checked separately.

Analytic tests additionally cover all reproduced failures, stationary endpoint
orders 1–6 in both orientations and weight signs, overflowed speeds with finite
tangents, minimum-subnormal parameter domains, genuine poles, unchanged source
geometry, and invalid parameters. The existing 70 independent surface references
also exercise the shared exact implementation.

```sh
python3 tools/numerics/generate_curve_rational_reference.py | \
  diff - crates/viboceros-geometry/src/nurbs/evaluate/exact/reference.txt
cargo test -p viboceros-geometry exact_curve_jets_match
cargo test -p viboceros-geometry nurbs::evaluate::tests
```

A fresh licensed Rhino 8 run in private Xvfb passed all
[23 rational-curve fixtures](curve-rational-range-rhino-comparison.json) and
[45 side-aware fixtures](curve-rational-range-sided-rhino-comparison.json) at
absolute `1e-8`, relative `1e-10`. These existing fixtures include native jets,
tangents, domains, and the documented arc-length/representation fields. The
reports retain every operation's comparison and harness timing. The rational
report's very large absolute differences concern huge homogeneous scales, which
are compared relatively; its aggregate maximum is not a position-error measure.
These ordinary-input comparisons are separate from the extreme-weight exact
references and are not native-kernel speed benchmarks.

## Scope and cost

Exact evaluation has input-dependent arbitrary-precision time and memory costs;
it is considerably slower than the ordinary floating-point path. Genuine poles,
constant tangents, and other failures can now incur that cost too. There is no
degree cap or timing assertion in the test suite.

The preparation guard and failure recovery are not a universal forward-error
bound: an unflagged floating-point evaluation that returns a finite but inaccurate
answer can still escape detection. Two-dimensional trim curves now have their own
[recovery policy](uv-rational-range.md). Isocurve extraction, structure edits, and
other homogeneous operations do not automatically inherit this policy. Neither
the independent references nor ordinary Rhino comparisons establish full Rhino
compatibility or native-kernel speed parity.

The [release measurement record](curve-rational-range-performance.json) compares
baseline `1135404` with this change using three rounds of 200 iterations. All
2,277 curve jet/tangent sample records across 69 operations remain bit-for-bit
unchanged, as do 114 surface closest-point results across 27 operations. Median
curve probe times increase by **2.1–5.4%**. Those probes include construction,
edits, and 33-sample JSON record generation, not only differential evaluation.
Curved surface closest-point batches increase by **0.4–2.4%** in the same run.
Raw round times and request reconstruction are retained; these host-dependent
measurements quantify guard cost on ordinary inputs, not exact-fallback speed.

The 127-case exact reference test took 62.09 seconds in debug and 1.40 seconds
in release. These are whole-test durations, including construction and checking
all requested orders and expected errors. The full workspace passed **2,470
Rust tests**, with 18 intentionally ignored, plus all **178 Python tests**;
strict Clippy, formatting, and deterministic reference regeneration also passed.
