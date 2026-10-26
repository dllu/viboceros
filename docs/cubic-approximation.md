# Certified cubic curve approximation

[Loft Refit investigation](loft-refit.md) · [Native curve fitting](smlib-curve-fit.md)

`NurbsCurve::try_approximate_cubic(tolerance)` builds polynomial cubic pieces
while retaining the source parameter domain and original knot boundaries.
`try_approximate_cubic_with_bound` also returns a continuous positional error
bound. Constant spans, closed curves, kinks and genuine one-sided jumps are
supported; jumps retain full-order knots instead of being bridged.

The shared exact spline engine restricts the original stored NURBS directly.
Hermite proposals interpolate exact endpoint positions and derivatives before
one final control rounding. Rational Bernstein differences certify the rounded
pieces and assembled curve. Outward finite intervals accelerate clear decisions;
uncertain, zero-limit or extreme cases retain exact rational arithmetic.
The same engine provides `parameter_curve_deviation_bound(other, limit)` for two
curves with affinely normalized domains. A returned bound proves the entire
correspondence; `None` is inconclusive or unsupported, not an equality proof.
For different parameter clocks, `mapped_curve_deviation_bound(other, pairs,
limit)` accepts 2–4097 normalized parameter pairs, strictly increasing from
`[0, 0]` to `[1, 1]`. It proves every interval of the resulting piecewise affine
correspondence, splitting exactly at either curve's knots. Coverage of both
domains also bounds symmetric Hausdorff distance; it does not find an optimal
matching or prove topology. Sampling can propose pairs, but cannot accept a fit.

The JSON/Python operation `curve_deviation_bound` accepts exactly two `curves`,
optional `correspondence` pairs (default: normalized identity), and optional
`absolute_tolerance`, including zero. It returns `certified`,
`maximum_deviation_bound` (null when unproved), and before/after definitions.
Three native curved Refit results certify within `0.001` using proposed matching
pairs, while their original normalized clocks fail that same correspondence
test. This validates a distance primitive, not the Rhino fitting algorithm.
[Validation](curve-correspondence-validation.json) retains the source bindings.

The JSON/Python oracle operation `curve_cubic_approximation` accepts a standard
`curve` definition, optional `absolute_tolerance`, and optional source
`parameters`. It returns the fitted definition, bound, samples and before/after
source definitions. Defaults use the request tolerance and 33 samples. Up to
4096 sample parameters are accepted and checked before fitting.

Fitting accepts degrees up to 16 and independently common-sign weight gauges.
It keeps at most 4096 controls and depth 48; fitting and final verification each
use the existing 2,000,000-unit work and 8192-bit rational limits. Mixed weight
signs, unrepresentable endpoints and unresolved budgets fail explicitly. Bounds
include stored-control rounding and preserve small coordinates and non-dyadic
knot intervals. They certify position, not smoothness after rounding, topology
or injectivity.

Fresh native source evaluations cover 20 curves, with 33 uniform and 11
additional positions per curve. The additional parameters are checked to lie
strictly between fitted interpolation knots.
The [validation record](cubic-approximation-validation.json) records their fits
at `1e-6` and `1e-9`, analytical witnesses and resource checks. An integer-control
polynomial vanishes at five uniform samples yet exceeds one unit between them;
the continuous bound detects it and forces refinement.

This is a source-parameter approximation primitive. Smooth Loft Refit supports
certified initial and corrected G1 proposals; its adaptive subdivision and joining
policy remains unfinished. See [Loft Refit](loft-refit-adjustment.md) and
[bounded knot reduction](geometry/knot-reduction.md) for those separate APIs.
No general Rhino Refit or arbitrary-scale performance parity is claimed.

```sh
cargo test -p viboceros-geometry --lib surface_pullback::certificate
cargo test -p viboceros-oracle --test cubic_approximation_native
```
