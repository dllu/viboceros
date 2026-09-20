# Ellipse closest points

[Curve closest points](curve-closest-point.md) · [Align](commands/align.md)

`Ellipse3::closest_parameter` returns the ellipse's native rational-quadratic
parameter, not an angle. It avoids the previous conversion to a generic NURBS
search. For center `(1,2,3)`, radii `(5,2)` and query `(-10,5,3)`, that search
stopped about `1.26e-8` from the true closest point: rounding of evaluated
world coordinates can defeat distance-based descent near a stationary minimum.
Rhino's result agrees with an independent 65-decimal stationarity solution.

## Equation and conditioning

Project the target onto the principal axes and reflect it into their positive
quadrant, giving `(px,py)`. A nearest point exists in that quadrant by symmetry.
With positive unit-circle coordinates `(X,Y)`, the stationary equation is

```text
g = b² - a² + a px / X - b py / Y = 0.
```

As the point traverses the quadrant, `X` decreases and `Y` increases, so `g`
is strictly increasing whenever a projected coordinate is nonzero. Axis
endpoint limits also handle interior medial-axis queries. The center selects
the minor-axis quadrant, or the seam for a circle. Reflection ties choose the
first native parameter, without depending on signed zero.

The three coefficients and the two finite endpoint limits are formed from
exact binary64-input rationals and divided by a common scale before rounding.
This avoids overflowing squared radii, product underflow during construction,
and cancellation at the evolute's axis endpoints. The bisection uses equivalent
endpoint-relative expressions and `1-X = Y²/(1+X)` (or its symmetric form),
so tiny off-axis minima are not lost by subtracting a rounded cosine from one.
It stops at adjacent representable parameters, with a 1,076-step upper bound;
ordinary queries need roughly 54 steps. Evaluation uses the existing native
rational ellipse parameterization directly.

## Independent checks and limits

Tests check the high-precision exterior root, axis projections, medial-axis
ties, circle-center ties, seam selection, reversed/nondefault domains, positive
and negative quadrants, and common scales `1e-150`, `1` and `1e150`. Stationarity,
the ellipse equation and 720 independent angular competitors are checked.
A query one binary64 value below the evolute endpoint retains its tiny
off-axis solution; the next value projects to the endpoint exactly.

Axis projections and final parameters remain floating-point quantities.
Unrepresentable projections and nonzero normalized coefficients that become
subnormal or zero are explicit errors. Arbitrary exponent ratios and parameter
intervals are not promised full geometric accuracy. The monotonicity argument
describes the ideal ellipse; stored rational weights and evaluations remain
binary64 approximations. The 39-case `Align ToCurve` record includes four
ellipse cases that now pass `1e-8` without changing the comparison threshold.

`cargo run --release -p viboceros-geometry --example profile_ellipse_closest`
compares the previous NURBS dispatch and quadrant root locally, including final
evaluation. It is not a Rhino benchmark or a claim of command-level speed parity.
One DGX Spark release run (seven batches of 500 queries, after 20 warmups) measured
median batch averages of **36.080 µs** for NURBS dispatch and **3.805 µs** for
the quadrant root on the exterior regression query. The original route returned
`(-3.9252723127936116, 2.3444863178307482, 3)`; the new route returned
`(-3.92527231828194, 2.3444863052756992, 3)`. This is a single local query case,
not a distribution-wide throughput guarantee.
