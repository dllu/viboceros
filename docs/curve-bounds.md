# Tight curve bounds

[Architecture](architecture.md) · [Array layouts](plane-arrays.md)

`CurveRef::tight_bounds(Tolerance)` and `NurbsCurve::tight_bounds(Tolerance)`
return a tolerance-controlled box for the complete active curve. Analytic
curves use analytic extrema; lines/polylines use vertices; polycurves combine
all native segments. NURBS control-point bounds remain a separate, cheaper API.
Existing general document bounds and the BoundingBox command are not globally
replaced by this new query.

## Rational subdivision

The kernel clamps the active domain and separates every original knot span,
including both limits of full-order discontinuities and closed seams. It
accumulates attained endpoint/midpoint coordinates, subdividing each span until
every control-hull face is close enough to an attained coordinate. Per-axis
epsilon is the maximum of absolute tolerance, relative tolerance times that
axis's coordinate magnitude, and a 16-ULP-scale rounding floor.

A Euclidean control hull is accepted only when all weights in that span have
the same sign. Mixed-sign spans must be refined; a failed subdivision never
justifies returning the control-point box. Alternative split locations avoid
projective intermediate controls. Near-zero relative control weights in mixed
spans are rejected when they make Euclidean subdivision unreliable.
The algorithm allows 131,072 visited nodes and depth 64. Unresolved poles,
unrepresentable subdivisions, and exhausted budgets return errors.

This is floating-point, tolerance-controlled refinement, not certified interval
arithmetic. It may reject a regular but ill-conditioned rational curve. Relative
tolerance is coordinate-scaled, so large world translations permit larger
absolute errors. Span extraction currently uses exact sequential NURBS splits;
very large multi-span inputs are a remaining performance optimization target.

## Verification and diagnostics

`curve_bounds.json` compares 16 public Rhino `GetBoundingBox(True)` calls:
degree 2/3/5/9 positive and mixed weights, an analytic quadratic extremum,
common gauges `1e-200`/`1e200`, an unclamped curve, a regular curve whose midpoint
split has a control at infinity, and three analytic circle orientations.
All agree at absolute `1e-8`, relative `1e-12`; the maximum observed coordinate
error is `8.99e-10`. Rust tests additionally check dense rational samples,
analytic extrema, negative gauges, full-order knot limits, closed rational
seams, and pole rejection.

`curve_bounds_diagnostics.json` retains a negative-common-gauge quadratic.
Multiplying all weights by -1 leaves the mathematical curve unchanged; native
bounds remain identical. The tested Rhino 8 tight-box call instead reports a
maximum Y of 4 rather than `124/13`, an error of about 5.54. This is an explicit
oracle discrepancy, not a reason to alter the native geometry or widen epsilon.
Rhino rejects the separate full-order-knot unit-test input as invalid, so that
case is a native correctness test, not a claimed Rhino comparison.

Bounds API calls are timed after warmup; array command fixtures are untimed.
Performance comparisons use native release code against this machine's
Wine/FEX Rhino, not native Windows Rhino. The initial 100-iteration run was
faster for 11 of 13 NURBS cases; both degree-nine cases were roughly 1.2–1.3×
slower. These measurements do not establish overall kernel performance parity.
