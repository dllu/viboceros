# Surface evaluation after homogeneous range loss

[NURBS numerical policy](nurbs-numerics.md) · [Batched grids](surface-grid-evaluation.md)

Surface point and differential evaluation normally uses binary64 de Boor
recurrences on a centered, weight-normalized active control net. Normalization
can erase a small weight, and multiplying a small local coordinate by its weight
can erase a coordinate. Later division can magnify that loss into a substantial
error. A finite result alone therefore does not establish that preparation was
safe.

For example, a bilinear surface with first-row weights `2^-700, 1`, first-row X
coordinates `MAX, 0`, and second-row weights `2^700, 2^700` previously evaluated
to X=0 at `(u,v)=(0.5,0)`. The correctly rounded answer is approximately
`3.4175792574734558e97`. Recovering only failed projections or exactly interpolated
corners cannot fix this case.

## Guard and fallback

The active-net preparation flags either of these conditions:

- A nonzero stored weight normalizes to zero or a subnormal number.
- A nonzero local coordinate times its normalized weight becomes zero or subnormal.

The check is conservative: even an exactly representable subnormal product
selects the fallback. It applies independently to the centered preparation and
the existing uncentered overflow retry. Ordinary nets retain their floating-point
evaluation path. The batched grid does not cache a flagged net's rounded U
contractions; affected cells use the scalar dispatcher.

The fallback in `nurbs_surface/evaluate/exact.rs` converts the original finite
binary64 controls, weights, knots, and parameters to `num-rational::BigRational`.
It forms world-space homogeneous controls, evaluates the selected tensor-product
span, constructs derivative control nets, and applies the rational quotient rule
using exact rational arithmetic throughout. Only the final requested Euclidean
components are rounded to binary64. Points at fully interpolated controls retain
their original stored coordinates, including signed zeros.

Points, first derivatives, pure second derivatives, and the mixed second
derivative share this path. Native parameter scaling, unclamped spans, independent
one-sided knot limits, and extended boundary-span evaluation remain supported.
The source surface is never modified.

An exactly zero homogeneous denominator remains `ZeroWeightAtParameter`, even
when all control locations coincide. A requested component that rounds to
infinity returns `NonFinite`; overflowing unrequested derivatives do not prevent
point-only evaluation. Finite outputs may round to subnormal numbers or signed
zero. There is no model-tolerance clamp or fabricated derivative.

## Independent validation

`tools/numerics/generate_surface_rational_reference.py` uses Python `Fraction`
and explicit Bernstein basis derivatives, independently of Rust's de Boor and
derivative-control recurrences. Its checked-in bit-pattern reference contains
70 tensor-Bezier cases with degrees through `(5,4)`, positive/common-negative/
mixed-sign weights spanning `2^-1000` to `2^1000`, varied coordinate scales,
anisotropic domains, endpoints, and rational extrapolation. Tests compare every
requested point/first/second-derivative component bit-for-bit and check overflow
errors separately. Regeneration is deterministic:

```sh
python3 tools/numerics/generate_surface_rational_reference.py | \
  diff - crates/viboceros-geometry/src/nurbs_surface/evaluate/exact/reference.txt
cargo test -p viboceros-geometry exact_surface_jets_match
cargo test -p viboceros-geometry range_loss
```

Additional analytic regressions check the formerly false-pole edge, the silently
zeroed large coordinate, finite derivatives after a weighted-coordinate
underflow, exact constant jets despite signed-weight cancellation, genuine poles
and overflow, all four limits at crossed discontinuities, grid dispatch, an
unclamped rational extrusion against a separately evaluated curve, and constant
continuation on a minimum-subnormal parameter domain. Existing grid/scalar parity
and saved Rhino closest-point replays provide separate regression coverage.
The extreme-weight references are mathematical checks, not new live Rhino results.

## Cost and limits

Arbitrary-precision evaluation is substantially more expensive than the usual
floating-point path. Its time and memory depend on degree and the sizes of the
intermediate rational numerators and denominators; it is not a constant-cost
replacement. The independent high-degree reference test is noticeably slower in
debug builds. No timing assertion is part of the test suite.

The [measurement record](surface-rational-range-performance.json) compares
baseline `5f1db5d` with this change in an aarch64 release build. Three rounds of
200 iterations retain all 114 ordinary closest-point results bit-for-bit. Median
curved-query timings increase by **1.6–6.1%**; affine queries increase by
approximately **0.05–0.06 µs** (5.1–6.0%). These fixtures measure the added guard,
not the exceptional exact evaluator. The first after-round has visible timing
outliers; the record retains every raw round, and these are bounded host-dependent
measurements, not a statistical performance guarantee.

The independent reference test took 92.96 seconds in debug and 2.37 seconds in
release, including construction, all three requested orders, error checks, and
bit comparisons. The full debug geometry suite took about 8.5 minutes, primarily
because its exhaustive extreme-weight grid test now exercises exact evaluation.
The complete workspace passed **2,458 tests** with 18 intentionally ignored;
all 178 Python tests, strict Clippy, formatting, and deterministic reference
regeneration also passed.

This guard detects range loss in **active homogeneous preparation**, not all
possible subsequent recurrence underflow or signed cancellation. Unflagged nets
continue to use floating-point arithmetic. Three-dimensional curves have their
own [recovery policy](curve-rational-range.md), sharing the exact recurrence.
Isocurve extraction, surface structure edits, and other homogeneous operations
do not automatically inherit this fallback. The change is not a universal correctly rounded geometry-kernel
claim, a proof of closest-point global optimality, or a Rhino performance-parity
claim.
