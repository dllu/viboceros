# Continuous surface/trim correspondence

[Geometry architecture](architecture.md) · [Existing pullbacks](commands/split.md) · [Rhino oracle](oracle.md)

`NurbsSurface::parameter_curve_deviation_bound(uv, spatial, limit)` proves a
continuous model-space distance bound between the surface image of a UV curve
and a spatial NURBS curve. Each curve's domain is mapped affinely to `[0,1]`.
The query returns `Some(bound)` only when the entire matched-parameter distance
is within `limit`. `None` means the correspondence is outside the limit or could
not be certified. It never establishes a match. Sources remain unchanged.

`try_pullback_curve_certified` proposes cubic Hermite spans with the regular
pullback fitter. Each proposed span must receive a continuous certificate at
the caller's absolute tolerance; inconclusive proposals cause further fitting
subdivision. The original spatial spline is restricted with exact polar forms,
so rounded subcurve extraction cannot change the reference being certified.
All fitting proposals reuse the exact spatial spline and tensor patches and
share one work budget. The assembled UV curve receives a final complete check.
Unsupported inputs or exhausted limits return an error. This supports trim
construction that needs a proof of complete edge correspondence. General curved
Boolean construction still needs
face partitioning, material classification, and shell assembly.

Surface cutting now requires the certificate for straight trim proposals,
retained bilinear pullbacks, fitted trims, and endpoint adjustments. An interior
excursion hidden at every old sample station cannot become a split boundary.
The ordinary `try_pullback_curve` and existing B-rep validation retain their
sampled behavior for callers that have not requested this stronger contract.

## Exact construction

The independent `surface_pullback/certificate` module has separate spline
extraction, tensor-surface, and Bernstein algebra helpers. Stored binary64
controls, weights, and knots are interpreted as exact rationals. Arbitrary common
positive or negative weight gauges are normalized exactly. Curve knots are
aligned in their normalized domains, and exact polar forms restrict each source
span without rounded knot insertion.

When a UV span's projected control hull stays in one tensor patch, Bernstein
products form its exact homogeneous surface image. Subtracting the spatial curve
through a common positive denominator gives a rational difference curve. Its
projected control hull bounds every point. Exact dyadic subdivision refines
inconclusive hulls. A candidate floating-point norm becomes a bound only after
its square is checked against the exact rational squared norm.

At a tensor knot crossing, the query restricts every intersected tensor patch
to the UV hull's bounding rectangle. Surface and spatial control boxes provide
a conservative distance bound. Exact dyadic subdivision localizes crossings
that have no convenient binary64 parameter. No sampled point is an acceptance
test. Curve excursions, rational denominator failures, and exhausted resources
cannot become successful certificates.

Clamped polynomial surfaces also have a global affine-reference path. Exact
Greville stations reproduce the affine map in the surface basis. Exact rational
L1 control residuals bound the surface's deviation from that map. A spatial
curve with the UV curve's normalized knots and proportional same-sign weights
receives a second control-residual bound; convex basis partitions make their
sum a continuous Euclidean upper bound. UV control hulls must stay inside the
surface domain. The final bound rounds upward. This avoids repeated tensor-knot
subdivision for rational circles on many-span planar rebuilds and also qualifies
their rounded spatial image controls. Images exceeding the control-residual
bound and mixed-sign UV weights are not accepted by this path.

## Scope and limits

- Surface and UV degrees up to 16; spatial curve degrees up to 64. The general
  Bernstein path bounds composed surface-image degree at 64; the polynomial
  affine-reference path avoids forming the composed polynomial.
  [Certified pushup](certified-surface-pushups.md) constructs
  spatial images and independently qualifies their rounded control definitions.
- Sign-coherent nonzero weights; mixed signs are uncertified.
- UV remains in the natural surface domain. Interior full-order surface knots
  are currently uncertified. Regular multiple knots and arbitrary source-curve
  knots are supported, including unclamped source curves.
- Two million exact work units, 8,192 bits per checked rational, subdivision depth
  48, and at most 64 tensor patches in one crossing-box test. All spans and stages
  share one work budget. Certified fitting proposals share a separate budget of
  the same size; the exact proposal check and final assembled-curve check each
  use one ordinary certificate budget.

Resource exhaustion returns `SurfaceCurveCertificateWorkLimit`. Unsupported
certificates return `None`. A zero limit can prove exact correspondence, but
some exact matches remain inconclusive at a non-dyadic knot crossing. These are
limits of the certificate. This query does not prove trim simplicity,
surface injectivity, manifold topology, or equivalence under an arbitrary curve
reparameterization. The bounded affine-reference check uses its own exact-work
budget before the general Bernstein path; an inconclusive result continues with
the ordinary certificate budget.

## Native evidence

The [13 public SDK recipes](../tools/rhino_oracle/fixtures/surface_curve_image.json)
and [raw observations](../tools/rhino_oracle/observations/surface_curve_image.json)
were captured with Rhino 8.32.26160.13001 on private Xvfb, in an empty owned
document at idle and a fresh private settings scheme. The probe retains full
surface, UV, and spatial control definitions, 129 paired sample stations,
closest-point locus errors, and source purity checks. Five recipes use public
[`Surface.Pushup`](https://mcneel.github.io/rhinocommon-api-docs/api/RhinoCommon/html/M_Rhino_Geometry_Surface_Pushup.htm)
on cylinders, spheres, and a torus. Its API describes a geometric fit tolerance.
See [capture provenance](surface-curve-certificate-provenance.json).

Rust tests reconstruct those public definitions, compare all paired sample
points at `1e-11`, and require continuous matched-parameter certificates at
`1e-6` for ten records. The constant offset and interior bump are rejected.
The sphere crossing a tensor boundary is also rejected at that uniform limit:
Rhino's retained maximum matched-parameter sample error is about `0.00649`,
while its sampled closest-point locus error is below `3.32e-7`. That native
parameterization difference stays explicit.

Independent kernel tests cover exact warped images, cancelling rational surface
and UV speeds, negative and extreme weight gauges, different and overflowing
curve domains, subnormal gaps, unclamped knots, exact and non-dyadic tensor
crossings, fitted regular pullbacks, invalid UV, and resource limits. The maximum
degree test exercises a degree-64 image compared with a degree-16 spatial curve,
using degree-80 binomial coefficients beyond the machine integer range. An
adversarial degree-16 surface vanishes at all 17 uniform stations within `1e-6`, but has large
excursions between them; the continuous query rejects it.

Fitting regressions also exercise a non-affine planar pullback whose sampled
Hermite proposal passes while its true error exceeds tolerance between stations.
Certified fitting refines that proposal and proves the resulting curve. A
degree-(15,4) graph surface has a straight diagonal's endpoints, tangents, and
all 17 sampled stations, but large intermediate excursions. Certified fitting
and the actual surface-split constructor reject that spatial curve.

The [two non-affine split command recipes](../tools/rhino_oracle/fixtures/surface_split_nonaffine_trimmed.json)
were recaptured on private Xvfb in a fresh scheme. The
[raw four-face output](../tools/rhino_oracle/observations/certified_surface_split.json)
and [provenance](certified-surface-splits-provenance.json) retain full underlying
surfaces, topology, edge domains, metadata, selection, and 65 equal-UV-arc-length
stations per non-isoparametric trim. The local replay compares all output fields
and preserves the two independently fitted parameterizations.
The [Python API replay](../tools/rhino_oracle/observations/certified_surface_split_comparison.json)
has a maximum numeric difference of `4.67e-10`. Its retained timings measure
the complete harness workflows, including geometry extraction and Rhino's
emulation/bridge overhead; they do not establish general kernel performance.

This evidence does not prove general curved Boolean parity, arbitrary topology,
closest-locus error bounds, or native performance.

## Python debugging API

The local oracle operation `surface_curve_deviation` accepts `surface`,
`parameter_curve`, `spatial_curve`, and `limit`. Definitions use the existing full
NURBS knot convention. UV controls use three wire coordinates with zero Z, then
become the kernel's separate `NurbsCurve2` type. Results contain `certified`,
`bound`, `limit`, and the explicit `normalized_curve_domains` correspondence.
The closed native capture operation is `surface_curve_image`.

The companion [`surface_pullback_certified` operation](constrained-surface-pullbacks.md#python-debugging-api)
fits a UV curve, optionally with both endpoints fixed, and returns its full
definition and continuous bound. Surface cutting uses this fitter when merely
adjusting endpoint controls cannot obtain a certificate.
Without constraints it first tries [certified automatic branches](automatic-surface-pullbacks.md)
at seams and singular endpoints. Successful proposals share exact reference
extraction and retain their bound, avoiding repeated proofs in the Python API.

```sh
cargo test --release -p viboceros-geometry surface_pullback
cargo test --release -p viboceros-oracle surface_curve_image
python3 -m unittest tools.rhino_oracle.test_surface_curve_image

# Choose a fresh private scheme for every live capture.
tools/rhino_oracle/run_headless.sh exec python3 -m tools.rhino_oracle rhino tools/rhino_oracle/fixtures/surface_curve_image.json --scheme VibocerosOracleSurfaceImageExample --timeout 600 --output /tmp/surface-image-rhino.json
```
