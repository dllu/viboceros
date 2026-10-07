# Certified UV-to-spatial curve images

[Continuous certificates](surface-curve-certificates.md) · [Development](development.md) · [Oracle](oracle.md)

`NurbsSurface::try_pushup_curve_certified(uv, tolerance)` constructs a spatial
NURBS curve from a UV NURBS curve. The variant
`try_pushup_curve_certified_with_bound` also returns its continuous model-space
deviation bound. Each returned curve keeps the UV curve's native domain and
affine normalized parameter correspondence. Acceptance uses the caller's
absolute tolerance. The surface and UV source remain unchanged.

This is a geometry-kernel and debugging-protocol API. A document `Pushup` command
adapter is still outstanding.

## Construction and proof

The first proposal uses exact rational Bernstein surface composition on each
original UV knot span. Linear rational UV curves also split at exact rational
tensor-knot crossings, including crossings with unequal endpoint weights.
Constant images become degree-one curves. Images of different degrees are
elevated exactly to a shared degree. Independent positive homogeneous gauges
and full-order interior knots retain each span's one-sided limits, including
discontinuous UV sources.

The exact homogeneous controls are projected into binary64 controls and weights;
native knot times are restored with one final rational rounding. The complete
assembled result then receives an independent correspondence certificate against
the original stored UV curve. Exact symbolic construction alone does not qualify
a rounded result.

When a UV control hull crosses tensor patches or an exact proposal cannot be
represented and certified, a derivative-free cubic interpolant is refined
adaptively. Surface evaluation uses a local UV chart and fractional curve
sampling, avoiding premature restoration of large parameter origins. Every
candidate is proved against exact restrictions of the original UV spline,
including all UV knots inside its interval. After restoring native knot times,
the complete returned curve is certified again. Samples construct proposals;
they do not establish the error bound.

## Limits

Surface and UV degrees are limited to 16, composed image and spatial reference
degrees to 64. Weights must be finite, nonzero and sign coherent. UV images must
stay in the natural surface domain; interior full-order surface knots remain
unsupported. Original nonuniform and unclamped UV knots are retained by exact
restriction. Adaptive fitting has depth 20 and at most
`MAX_CURVE_DIVISION_POINTS` output controls. All proposals share two million
exact work units and the certificate's rational-size limits. The independent
final certificate has its own bounded budget.

An unsupported or inconclusive image returns an error, including when rounded
native knot times cannot represent a qualified fit. Discontinuous sources that
need adaptive fitting may also fail. The API guarantees neither smooth joins nor
topology, injectivity, arbitrary reparameterization equivalence, or performance
parity with Rhino.

## Native evidence and replay

A fresh capture of the existing 13 public SDK surface-image recipes ran on
private Xvfb in an idle, empty owned Rhino document with settings scheme
`VibocerosOraclePushup20261006`. The
[raw observations](../tools/rhino_oracle/observations/surface_pushup_certified.json)
retain all original surface and UV definitions, 129 stations per source, and
source-purity results. Five recipes invoke public `Surface.Pushup` on cylinders,
spheres and a torus. The other eight define diagnostic reference curves.

The [local fixture](../tools/rhino_oracle/fixtures/surface_pushup_certified.json)
constructs the image of each original UV source at `1e-6`. Offset and bump
diagnostic references do not become target geometry: pushup constructs the
surface image itself. Rhino's retained sphere-seam matched-parameter discrepancy
also remains explicit. Its geometric fitting contract differs from this API's
normalized-parameter correspondence.

All 13 local images certify below their requested `1e-6` limit. Eight diagnostic
sources have zero certified error; the five curved primitive images have bounds
at most `3.501e-16`. Rust replay checks 1,677 returned-curve stations against the
native surface-image witnesses and separately recomputes the continuous bounds.

```sh
python3 -m tools.rhino_oracle viboceros tools/rhino_oracle/fixtures/surface_pushup_certified.json --timeout 600
cargo test -p viboceros-geometry --release surface_pullback::certificate::pushup
cargo test -p viboceros-oracle --release surface_curve_image::pushup_tests
```

See [local output](certified-surface-pushup-local.json) and
[capture provenance](certified-surface-pushup-provenance.json). Independent
regressions cover nonlinear nondyadic patch crossings, rational speeds, negative
gauges, discontinuities, constant images, degree-18 outputs, adjacent-float and
subnormal domains, and invalid inputs. Successful cases receive a separately
recomputed complete certificate.
