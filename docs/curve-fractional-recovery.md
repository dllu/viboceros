# Exact fractional stations and mixed-weight curve poles

[Fractional sampling](curve-parameter-sampling.md) · [Native rational recovery](curve-rational-range.md)

The curve sampler retains exact fractional parameters for exceptional queries,
instead of first rounding them into a native `f64`. Native curve evaluation also
uses exact arithmetic for mixed-sign control-weight spans, where a rounded blend
can hide a true rational pole. Source knots, controls, weights, and domains stay
unchanged. Common-sign weights, including all-negative gauges, keep the ordinary
fast path unless an existing range/failure guard requires recovery.

## Reproduced failures

At baseline `965b253`, a rational line on `[0, smallest_subnormal]` sampled only
its endpoints. Controls at X=`0,2` with weights `1,2` should instead have
X=`4f/(1+f)` at parameter fraction `f`. Exact half and quarter stations need not
have a representable native parameter.

A degree-one curve with a first exterior knot at `-1e308` and active knots near
`2^52` could not shift all its knots losslessly. A first-span sample returned
X=`-4` instead of `-3.75`. Separately, a one-ULP span inside a wide domain lost its
midpoint. These now use exact fractional evaluation without altering any knot.

The audit found two further distinctions:

- On domain `[0,1.5]`, the binary64 value `1/3` identifies a parameter slightly
  below `0.5`. Rounding it to `0.5` can choose the wrong branch of a discontinuous
  curve. Whole-domain queries with full-order interior knots now select the span
  using the exact interpolated parameter.
- A rational line with weights `1,-2` on that domain has a true native pole at
  `t=0.5`. The ordinary evaluator could miss this zero denominator because its
  blend ratio rounded to a nearby fraction. Native point/jet/tangent queries now
  reject the pole. Conversely, the exact **fractional** query `f=1/3` is finite:
  with X controls `0,1`, its correctly rounded X is `-12009599006321322`.

Tests retain the distinction between the native API and the fractional API;
neither changes the meaning of its input to match the other.

## Arithmetic and dispatch

`nurbs/sampling/exact` forms `a + (b-a)f` from exact binary rationals, chooses the
right-hand native span for whole-domain queries, or uses the explicit span for
span-local queries. Homogeneous evaluation and projection stay rational until
the final point components are rounded. Actual zero denominators remain errors;
outputs that cannot round to a finite coordinate remain `NonFinite`.

The fallback covers declined whole-curve origin shifts, subnormal intervals or
fractions, adjacent-float spans, subnormal intermediate parameters, interior
stations rounded onto an interval endpoint, and whole-domain positional jumps.
A failed ordinary interior sample also retries at its exact fractional parameter
before reporting a pole or overflow. Natural fraction endpoints retain the native
evaluation path and its stored-control endpoint shortcuts.

`nurbs/exact` now exposes the shared recurrence with an exact parameter argument.
Existing curve/surface exact jets call the same recurrence with their native
parameter converted to a rational. Exact curve control conversion is shared.
No parallel implementation of the derivative recurrence was introduced.

Native homogeneous preparation marks mixed-sign weights for exact evaluation
before trusting a small nonzero denominator. This is deliberately not a
near-zero tolerance test. The prior constant-curve false-pole regression now
checks this early guard while retaining its exact point and zero-derivative
assertions. Common-sign control preparation is independently checked to stay fast.

## Validation

The [independent generator](../tools/numerics/generate_fractional_curve_reference.py)
uses Python `Fraction` basis-function sums, not the production de Boor recurrence.
Its 173 reference rows exercise both sampler entry points, degrees 1–8,
unclamped and multispan curves, both common weight signs, mixed weights, positive
and negative knot origins, subnormal fractions/intervals/intermediates, true
poles, overflow, and exact branch selection. Every recovered coordinate matches
its reference bits. The ordinary native exact-jet tables remain regression gates.

Viewport tests verify the full projected polyline and click distances for
subnormal-domain rational quadratics, including source preservation. Native
mixed-weight regressions verify a true pole and analytic jets on both sides.
The complete workspace run passed its exhaustive surface-grid test; the final
follow-up skipped only that already-completed, unaffected grid test. Together
they verify **2,527 Rust tests**, with 18 intentionally ignored. All **195 Python
tests**, strict all-target Clippy, formatting, fixture regeneration, and
warning-free geometry documentation builds pass.

```sh
python3 tools/numerics/generate_fractional_curve_reference.py | \
  diff - crates/viboceros-geometry/src/nurbs/sampling/reference.txt
cargo test -p viboceros-geometry nurbs::sampling
cargo test -p viboceros-geometry mixed_weight
```

The [four new oracle cases](../tools/rhino_oracle/fixtures/curve_fractional_recovery.json)
compare rational lines and unclamped quadratics with both signs of large knot
origin and an unshiftable exterior knot. All four pass in licensed Rhino 8.32
in private Xvfb, at absolute/relative `1e-12`; the largest coordinate difference
is `8.881784197001252e-16`. Saved [native](curve-fractional-recovery-native-reference.json),
[Rhino](curve-fractional-recovery-rhino-reference.json), and
[comparison](curve-fractional-recovery-rhino-comparison.json) records retain the data.
The prior 12 ordinary sampling records remain bit-identical to their native
checkpoint references.

As in the previous probe, Rhino's privately owned shape reference gets a unit
domain before sampling. These comparisons do not assert raw native-grid parity,
nor use Rhino as the reference for undefined pole values. Subnormal and pole
claims are checked independently above. Harness timing ratios are not kernel
speedups.

## Cost and remaining scope

Exact fractional and mixed-weight arithmetic is materially more expensive than
ordinary floating-point evaluation. The four small native probe records take
roughly 70–430 microseconds each, including setup and point arrays. They measure
exceptional inputs, not ordinary drawing throughput. The cold fractional helper
stays out of the hot span loop; the public span evaluator is explicitly inlined.

The ordinary cubic sampling benchmark was repeated against `965b253`, pinned to
CPU 5, in five alternating-order rounds of 20,000 iterations per case. It uses
4 or 19 controls, all-positive weights, and local/`±1e12` domains. The
[before](curve-fractional-recovery-performance-before.csv) and
[after](curve-fractional-recovery-performance-after.csv) records show median
increases of `3.2–4.7%` for one-span curves and `0.0–1.9%` for 16-span curves.
The initial unpinned trial nearly doubled both the unchanged legacy loop and
the new loop in one batch; pinning removes one source of variation, not all
timing uncertainty. These bounded checks are not a performance guarantee and
do not measure the more expensive mixed-sign path.

The sampler is still not universally correctly rounded. Unflagged fast-path
queries retain floating-point error, and narrow relative spans with several
representable native interior parameters can still lose fractional resolution.
No already-rounded input knot is repaired. This does not make fixed viewport
subdivision adaptive, detect every pole between display stations, or change
parameter-returning geometry operations. Surface mixed-weight dispatch has its
own policy; the new native mixed-sign guard described here is for curves.
