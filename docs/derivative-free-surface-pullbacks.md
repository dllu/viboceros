# Certified pullbacks at singular endpoints

[Automatic branches](automatic-surface-pullbacks.md) · [Continuous certificates](surface-curve-certificates.md) · [Fixed endpoints](constrained-surface-pullbacks.md)

Nonlinear UV paths can now fit through singular endpoint nodes without inverting
the surface Jacobian. Existing inverse, straight-path, and Hermite proposals
remain the first choices. If the regular fitter encounters a degenerate or
nonfinite node, cannot converge, or exhausts its proof budget, a separate
`surface_pullback/interpolation` controller tries derivative-free cubic fitting.
Every accepted fallback, including ordinary `try_pullback_curve` output, needs
a continuous certificate at the caller's absolute tolerance.

## Fitting and proof

The fallback samples the original spatial curve in a fractional parameter
frame. It interpolates closest UV points at one-third and two-thirds of each
interval, using endpoint controls directly. Four further interior stations
propose extrapolated endpoint coordinates. This recovers chart coordinates
lost at collapsed boundaries, without recognizing primitives or assuming a
regular endpoint tangent. Boundary, corner, and extrapolated alternatives are
proposals; their complete images must still certify. Explicit caller endpoints
remain authoritative throughout subdivision.

UV arithmetic uses a lossless local surface frame. Candidate controls are
restored into the original chart before proof. The certificate references the
original exact spatial spline and explicitly splits at every original spatial
knot inside each fractional interval. It never substitutes a rounded spatial
subcurve. Inconclusive proposals subdivide, retaining a shared midpoint UV node.
The existing segment, depth, work, rational-size, and degree limits still apply.

The final assembled UV spline receives another complete certificate after its
native knot times and controls are restored. A span certificate cannot silently
stand in for an assembled curve changed by rounding or shared-control merging.
The original surface, spatial definition, domain, and fixed endpoints stay
unchanged. This verifies correspondence rather than simplicity or topology.

## Evidence and boundaries

Independent polynomial regressions use `S(u,v)=(u²,v,uv)` with UV paths
`(t,t²)` and `(t,t³)`. The starting Jacobian is singular although the complete
spatial images are valid. Another surface,
`S(u,v)=(4uv(1-v),4v(1-v),v)`, loses the U coordinate at both V boundaries;
the nonlinear path `(.25+.5t²,t)` now fits in either direction, including swapped
charts. Adjacent-float and minimum-subnormal source domains retain their full
fractional stations and original returned domain. A straight spatial boundary
on the first surface requires `u=sqrt(t)`; its fit refines to 103 UV controls
and obtains a complete bound below `1e-6`.

The [ten public SDK recipes](../tools/rhino_oracle/fixtures/surface_pullback_interpolation.json)
were captured on private Xvfb with a fresh settings scheme. Their
[complete native records](../tools/rhino_oracle/observations/surface_pullback_interpolation.json)
keep 1,290 paired source/image stations, source purity, native outputs when
present, independent nonlinear UV witnesses, and closest-locus errors. The
[before-change audit](../tools/rhino_oracle/observations/surface_pullback_interpolation_before.json)
failed on all ten local definitions. The
[local run](../tools/rhino_oracle/observations/surface_pullback_interpolation_certified.json)
certifies all ten at absolute limit `1e-6`, preserving source domains. Polynomial
recipe bounds are below `9e-16`; the two square-root recipes have bounds about
`9.87e-7`. The Rust replay independently recomputes each returned curve's bound
and reconstructs native source and image stations at `1e-11`.
See [provenance](derivative-free-pullback-provenance.json).

Native public `Surface.Pullback` succeeds on eight recipes. Their closest-locus
samples remain below the input limit, while normalized paired errors are about
`0.102`, `0.227`, or `0.25`, depending on the recipe. Both singular cubic
directions return no native pullback. These failures and parameter-speed
differences remain explicit. Native geometric-locus fitting and the local
normalized correspondence contract are distinct; these records do not prove
full native parity or performance parity.

The fallback still depends on closest-point proposals and a bounded cubic
representation. It does not establish arbitrary nonlinear periodic chart
tracking, all singularities, trim topology, injectivity, or curved Booleans.
Domains that cannot represent required restored subdivision knots or controls
can still fail rather than returning an uncertified curve.

```sh
cargo test --release -p viboceros-geometry surface_pullback
cargo test --release -p viboceros-oracle surface_curve_image
python3 -m unittest tools.rhino_oracle.test_surface_pullback_interpolation
python3 -m tools.rhino_oracle viboceros tools/rhino_oracle/fixtures/surface_pullback_interpolation_certified.json --timeout 600 --output /tmp/interpolated-pullbacks.json

# Use a fresh private scheme for every new native capture.
tools/rhino_oracle/run_headless.sh rhino tools/rhino_oracle/fixtures/surface_pullback_interpolation.json --scheme VibocerosOracleInterpolatedPullbackExample --timeout 600 --output /tmp/interpolated-pullbacks-rhino.json
```
