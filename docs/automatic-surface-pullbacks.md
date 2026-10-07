# Automatic certified pullback branches

[Continuous certificates](surface-curve-certificates.md) · [Fixed endpoints](constrained-surface-pullbacks.md) · [Rhino oracle](oracle.md)

Unconstrained `try_pullback_curve` and `try_pullback_curve_certified` now discover
eligible straight UV paths before constructing Hermite nodes. This repairs six
closed native isocurves that previously failed without caller-supplied seam
endpoints. It also handles eligible curves ending at singular surface points,
including non-isocurve paths and curves with both endpoints on collapsed edges.

The discovery is independent of primitive recognition. Every accepted proposal
needs an exact-rational certificate for the complete spatial correspondence at
absolute tolerance. Sources and their stored parameter domains stay unchanged.
Explicit fixed endpoints remain authoritative and bypass automatic discovery.

## Proposal and proof

Closest-point searches propose the two endpoint coordinates. Alternative
coordinates use natural-domain boundaries, corners, and coordinates from the
opposite endpoint. Only plausible endpoint images are retained. If ambiguities
remain and no proposal certifies, one interior source station supplies further
coordinates. This recovers a constant coordinate lost at two collapsed ends.
There are at most 13 candidates per endpoint and 169 distinct endpoint pairs.

Three interior stations reject implausible straight paths before exact work.
These checks allow numerical evaluation roundoff and never establish acceptance.
The source uses its fractional sampling frame, preserving stations on adjacent,
subnormal, and wide knot domains. Surface knots and candidate coordinates move
together into a lossless local UV frame before interpolation. If interpolation
still loses a station's fractional position, that rejection check is skipped.
The certificate still uses the original surface, curve, and UV controls.

The proof controller reuses exact source extraction and tensor patches across
proposals, with one bounded work budget. Duplicate pairs are skipped. Exhaustion
or unsupported certificate representations stop discovery and leave the regular
fitter available. The regular fitter retains its own existing proof budget.
Eligible original closest endpoints are reused in Hermite nodes when discovery
does not succeed. No ambiguous alternative endpoint is forced into a failed fit.

Rational linear UV spans now split at exact crossings of tensor knots, including
fractions such as `1/3` and `1/7` that cannot be stored as binary64 parameters.
All source and UV knot cuts remain in the partition. This avoids dyadic chasing
when native domain changes place corresponding knots a few bits apart. For
isocurves, the surface proof uses exact one-dimensional restrictions and applies
the UV segment's rational parameter change directly to Bernstein weights.
Equal-weight paths avoid unnecessary tensor products in the fixed coordinate;
two-coordinate paths retain the general tensor composition proof.

`try_pullback_curve_certified_with_bound(spatial, endpoints, tolerance)` returns
`(NurbsCurve2, bound)`. `endpoints` is `None` for automatic discovery or `Some`
for the existing fixed-endpoint contract. Exact inverse and straight proposals
retain their successful proof. An assembled Hermite fit receives a complete
final certificate across every original source and UV knot span. The Python
`surface_pullback_certified` operation now uses this result directly, avoiding
repeated certificates merely to expose the bound.

## Regression evidence

Geometry tests cover both isocurve directions on cylindrical, spherical, and
toroidal surfaces with translated/rotated model frames, reversed parameter
directions, relocated domains, and swapped UV coordinates. Independent
polynomial definitions cover `S(u,v)=(u²,v,uv)` along its singular diagonal and
`S(u,v)=(4uv(1-v),4v(1-v),v)` with two collapsed boundaries. Signed extreme
weight gauges preserve valid proofs. Other tests keep fixed-endpoint authority,
absolute tolerance, resource limits, and rejection of hidden sampled excursions.

Fractional regressions use adjacent-float, minimum-subnormal, and maximum-range
source domains. A singular diagonal on an adjacent-float surface domain also
requires a zero continuous bound, proving that rounded UV station composition
does not stand in for the original path.
Additional regressions cross knots in both directions at non-binary rational
fractions and retain nonuniform UV weights under reversed and swapped charts.

The [24 public SDK recipes](../tools/rhino_oracle/fixtures/surface_pullback_linear.json)
were captured on private Xvfb in a fresh settings scheme. Their
[complete records](../tools/rhino_oracle/observations/surface_pullback_linear.json)
retain source purity, 129 stations per recipe, independent straight-path
witnesses, native output when present, and bounded native timings. The local
[runnable request](../tools/rhino_oracle/fixtures/surface_pullback_linear_certified.json)
omits endpoint constraints. Rust tests require certificates on all retained
source definitions, reconstruct the 3,096 native source/image stations at
`1e-11`, and independently recompute the returned continuous bounds.
See [provenance](automatic-surface-pullback-provenance.json).

Rhino's public [`Surface.Pullback`](https://mcneel.github.io/rhinocommon-api-docs/api/RhinoCommon/html/M_Rhino_Geometry_Surface_Pullback.htm)
fits a geometric locus, rather than promising normalized-parameter agreement.
It succeeds in 20 recipes. Both directions of a collapsed spherical boundary
curve and the singular diagonal return no native pullback. Transposing the
two-pole surface changes native parameter speed: paired errors reach about
`0.0283`, while sampled closest-locus errors remain below `1.4e-11`. Those
failures and speed differences remain explicit rather than being normalized
away. Native samples do not establish continuous error bounds.

Automatic discovery handles certified straight UV paths, not arbitrary periodic
chart tracking or nonlinear singular-endpoint fitting. It does not prove trim
simplicity, topology, injectivity, curved Booleans, or full Rhino compatibility.
Certificate degree, rational-size, work, and subdivision limits still apply.
Native SDK timings and local complete-proof timings have different contracts
and runtimes; they do not establish general performance parity.

The [recorded local run](../tools/rhino_oracle/observations/surface_pullback_linear_certified.json)
certifies all 24 sources with two UV controls and bounds below `1e-12`.
[Timing snapshots](automatic-surface-pullback-performance.json) show the relocated
cylinder fits improving from about 98 ms to 4 ms per call, roughly 25 times
faster, after exact crossing partitions and isocurve restrictions. Their
reported bounds tighten from about `6.74e-7` to `8.41e-16`. Other regular
isocurves improve by roughly 1.3–1.7 times in these measurements.
The local complete-proof path still takes more time in most native-success
recipes: the median local/native ratio is about 6.8. These bounded measurements
identify remaining performance work rather than establishing parity.

```sh
cargo test --release -p viboceros-geometry surface_pullback
cargo test --release -p viboceros-oracle surface_curve_image
python3 -m unittest tools.rhino_oracle.test_surface_pullback_linear
python3 -m tools.rhino_oracle viboceros tools/rhino_oracle/fixtures/surface_pullback_linear_certified.json --timeout 600 --output /tmp/automatic-pullbacks.json

# Use a fresh private scheme for each new native capture.
tools/rhino_oracle/run_headless.sh rhino tools/rhino_oracle/fixtures/surface_pullback_linear.json --scheme VibocerosOracleLinearPullbackExample --timeout 600 --output /tmp/automatic-pullbacks-rhino.json
```
