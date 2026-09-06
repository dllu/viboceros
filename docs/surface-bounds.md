# Tight untrimmed surface bounds

[Curve bounds](curve-bounds.md) · [Array layouts](plane-arrays.md) · [Oracle](oracle.md)

Exact [trim-boundary image bounds](trim-boundary-bounds.md) are available
separately; they do not yet supply trim-aware interior extrema for a whole face.

`NurbsSurface::tight_bounds(Tolerance)` bounds the complete active untrimmed
surface, including independent limits at fully multiple knots in either
direction. It shares homogeneous span extraction and adaptive subdivision with
the curve kernel. Tensor patches split along unresolved spatial variation, or
denominator variation when weights have mixed signs. This avoids repeatedly
splitting the constant direction of an extruded ridge.

Same-sign rational control hulls enclose each accepted patch; attained corners
and interior points control refinement. The tolerance, floating-point caveats,
and resource budgets are the same as for [curves](curve-bounds.md). This is not
interval-certified arithmetic. Poles or unresolved ill-conditioning return an
error; negative common weight gauges do not change the geometry or bounds.

`Array Mode=Fill` uses these boxes in construction-plane coordinates for
standalone NURBS surfaces; `ArrayPolar Rotate=No` uses their world-box center.
Failure aborts the entire command without copies or an undo record. Existing
document bounds, the BoundingBox command, and **B-rep bounds are unchanged**.
Tight bounds of trimmed faces are not implemented by bounding the entire
underlying surface, and surface-array Rhino compatibility remains incomplete.

## Correctness evidence

Rust tests check separable quadratic extrema, an oblique quadratic against
independently derived extrema on all axes, a mixed-weight rational ridge,
positive/negative and extreme common gauges, unclamped spans, fully multiple
U/V knots, a large world translation, tilted spheres/tori against analytic
boxes, and pole rejection. Tensor extraction is checked against independent
B-spline evaluation over degrees 1–5 by 1–4 and every active knot rectangle;
subdivision is checked against the parent patch's parameter map.

`surface_bounds.json` has 25 public Rhino `GetBoundingBox(True)` comparisons:
degrees through 9, polynomial and signed rational surfaces, clamped/unclamped
knots, extreme common gauges, and closed surfaces. All agree at absolute
`1e-8`, relative `1e-12`; maximum observed box-coordinate error is `8.93e-10`.
API queries are timed after warmup; sample-grid diagnostics and actual array
commands are untimed.

In the recorded 20-iteration release run, the 25 passing native queries took
about 0.3–278 microseconds each and were all faster than the recorded Rhino
Wine/FEX calls (smallest speedup 1.36×). These API microbenchmarks include
different runtime overheads and do not establish native-Windows performance
parity or overall kernel performance.

## Inaccurate reference boxes

`surface_bounds_diagnostics.json` retains four failing Rhino box queries.
Each includes a separate 41×41 evaluated grid. Rhino's own grid contains points
outside its reported box, while native bounds contain the grid. Grid-box
coordinates agree between evaluators within `1.78e-15` in the recorded run.

| Diagnostic | Largest exclusion of Rhino's own sampled points |
| --- | ---: |
| Negative-common-gauge quadratic | 1.0 |
| Unclamped bicubic polynomial | 0.06407 |
| Unclamped bicubic signed rational | 0.09761 |
| Oblique quadratic | 0.04986 |

For the negative-gauge quadratic, the exact maximum Z is 3; Rhino reports 2.
For the oblique quadratic, the native box also agrees with separately derived
analytic extrema. These are explicit oracle discrepancies, not passing
references or grounds for changing native geometry. A finite sample grid is
an exclusion witness, not proof of a continuous enclosure.

## Actual surface-array coverage and remaining differences

`surface_array_bounds.json` preserves all 32 command cases across Top, Front,
Right, and oblique planes. Inputs include a quadratic, a signed rational ridge,
an unclamped surface grouped with a line, and a tilted torus. Records compare
original/copy identity, selection, group membership, both native surface domains,
25 surface points, and 33 points for the companion curve.

24 cases pass at absolute `1e-8`, relative `1e-12`. Eight fail: six have residual
placement errors of `1.46e-8`–`2.96e-8`; oblique quadratic Fill differs by
`6.24e-4`, and oblique torus Fill by `0.01817`. All original samples match within
`5e-15`; each copied object's discrepancy is a constant translation within
`3e-14`, rather than a deformation or parameterization mismatch. The actual
command's placement differs from what the independently verified native bounds
produce. This does not prove which internal Rhino bound routine it uses.
All eight remain in the fixture, so its full Rhino comparison intentionally
fails at the stated epsilon. No tolerance is widened to hide these differences.

A private-display release UI smoke test created an oriented ellipsoid, ran
Front-plane Fill, undo/redo, and Right-plane nonrotating polar copies, then
exported 3DM. All six surfaces retained their domains; 441 samples per object
matched independently calculated analytic placements within `8.88e-10`.
