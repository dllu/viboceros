# Certified curve-image correspondence

The NURBS kernel has three continuous distance certificates:

| Method | Supplied correspondence |
| --- | --- |
| `parameter_curve_deviation_bound` | Affinely normalized curve domains |
| `mapped_curve_deviation_bound` | Strictly increasing piecewise affine parameter pairs |
| `projected_curve_deviation_bound` | A nonlinear parameter spline proposed from one spatial coordinate |

Every successful query proves the complete rounded curve definitions are within
the requested absolute distance. `None` is inconclusive; sampled agreement never
establishes acceptance. Sources remain unchanged. These are positional bounds,
without a topology, tangent, or injectivity guarantee.

The projected proposal uses the target's largest endpoint coordinate displacement.
Projected source controls propose target parameters, retaining the source degree,
knots and weights. All map controls stay in the target domain; its clamped ends
are exactly the target domain endpoints. Common-sign weights and continuous
source knots make the parameter map continuous and onto that whole domain, even
if it backtracks. Thus a bound along the map proves symmetric image distance.
Interior full-order source knots are declined: a jump could skip a target interval.

A tensor surface with two identical rows represents the target curve exactly.
The existing [surface-image certificate](surface-curve-certificates.md) proves its
composition with the rounded parameter spline against the complete source.
No new Bernstein algebra or sampled acceptance path is introduced. Both curves
must be clamped, with common-sign weights and degrees at most 16. Controls outside
the target coordinate slab and a zero target endpoint displacement decline the
proposal. Existing surface-image degree, exact-work, bit and depth limits apply.

G1 fitting also requires a strictly increasing map. Exact Bernstein coefficients
of `A'W - AW'` certify an increasing rational map. Bounded exact subdivision
retains isolated stationary points; constant intervals and reversals decline it.
This guard preserves retraced source traversal: Rhino uses 50 or 34 controls for
the retained retraced-line examples, despite their zero image distance to a line.
The unrestricted public distance certificate makes no traversal-order claim.

G1 fitting tries the ordered proof after affine correspondence and before the
existing closest-point map. An inconclusive or exhausted projection retains that fallback's
255-query, 256-interval limits. [Validation](projected-correspondence-validation.json)
records recovery of four curved Loft initial fits, native fitting clocks and
64 additional style, command, frame, reversal, gauge, domain and scale cases.
General adaptive Loft subdivision remains unfinished.

```sh
cargo test -p viboceros-geometry --lib certificate::curve_pair --frozen
cargo test -p viboceros-oracle --test loft_refit_curved_native --frozen
```
