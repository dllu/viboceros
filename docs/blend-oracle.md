# Curve blend API comparison

[Blend command](commands/blend.md) · [Oracle setup](oracle.md)

The [eleven shared line inputs](../tools/rhino_oracle/fixtures/blend_lines.json)
were run against Rhino 8.32's public
[`Curve.CreateBlendCurve`](https://developer.rhino3d.com/api/rhinocommon/rhino.geometry.curve/createblendcurve)
API in an owned private Xvfb session. The
[raw response](../tools/rhino_oracle/observations/blend_lines.json) records the
full degree, control points, weights, knots, and parameter domain. This is an
API comparison; it does not prove identical interactive `Blend` command input
or handle behavior.

For the five same-continuity line inputs, Rhino returns a degree 1 line for
G0, a degree 3 blend with each G1 handle equal to the endpoint distance, and
a degree 5 blend with each G2 handle 40% of that distance. Viboceros matches
their degrees and every control point within `1e-12`. Rhino's curve domain
ends near its computed arc length; the saved raw domains differ from
Viboceros's independently integrated lengths by less than `3e-8`.

Six mixed-continuity inputs use RhinoCommon's endpoint-specific overload.
Both engines produce degree 2 for G0/G1, degree 3 for G0/G2, and degree 4 for
G1/G2. Their controls match within `1e-12`, and both engines retain normalized
`[0,1]` domains. The [oracle regression](../crates/viboceros-oracle/src/blend_curve.rs)
checks these fields without rewriting the recorded Rhino values.

The [48 parallel-tangent line inputs](../tools/rhino_oracle/fixtures/blend_parallel_lines.json)
cover all six mixed combinations with collinear, offset, translated, scaled,
diagonal, nearly collinear, and spatial endpoints. Their
[raw Rhino definitions](../tools/rhino_oracle/observations/blend_parallel_lines.json)
match Viboceros in degree, knots, weights, domain, and every control point
within `1e-12`.

Another [96 spatial mixed inputs](../tools/rhino_oracle/fixtures/blend_mixed_spatial.json)
cover sixteen independent 3D line pairs and all six mixed modes. Their
[raw definitions](../tools/rhino_oracle/observations/blend_mixed_spatial.json)
also fully match within `1e-12`. Across these probes, let `L` be the endpoint
distance and let `a` and `b` be the absolute chord projections onto the two
unit source tangent lines divided by `L`. The endpoint handle lengths for a
degree `n` mixed blend are:

```text
first  = 2L/n × [1.4 − a + (a − b)(0.3 + 0.5b)]
second = 2L/n × [1.4 − b + (b − a)(0.3 + 0.5a)]
```

The formula was inferred from line-source Rhino observations. A separate
[21-case arc-source fixture](../tools/rhino_oracle/fixtures/blend_curved_sources.json)
compares arc-to-line, line-to-arc, and arc-to-arc blends at all six mixed modes
and G2/G2. Its [raw Rhino definitions](../tools/rhino_oracle/observations/blend_curved_sources.json)
match Viboceros in degree, every control point and weight within `1e-12`, and
all mixed domains and knots. The G2/G2 arc-length domains and knots differ by
at most `3e-8`.

The [28-case NURBS-source fixture](../tools/rhino_oracle/fixtures/blend_nurbs_sources.json)
adds polynomial and rational cubic inputs with varying curvature. Its
[raw Rhino definitions](../tools/rhino_oracle/observations/blend_nurbs_sources.json)
match every control point and weight within `1e-12`, all mixed domains and
knots, and all degrees. The four G2/G2 domain and knot residuals are below
`3e-8`.

The [21-case segmented-source fixture](../tools/rhino_oracle/fixtures/blend_polycurve_sources.json)
adds polylines and line/arc polycurves. Its
[raw Rhino definitions](../tools/rhino_oracle/observations/blend_polycurve_sources.json)
match every control point and weight within `1e-12`, all mixed domains and
knots, and all degrees; G2/G2 domain and knot residuals remain below `3e-8`.

The [30 endpoint-pick inputs](../tools/rhino_oracle/fixtures/blend_endpoint_picks.json)
cover three other start/end choices with all nine continuity pairs, plus three
equal-continuity end-to-start cases using the endpoint-specific overload. Their
[raw Rhino definitions](../tools/rhino_oracle/observations/blend_endpoint_picks.json)
match Viboceros in degree, knots, weights, normalized `[0,1]` domain, and every
control point within `1e-12`. Unlike the three-argument end-to-start API,
Rhino's endpoint-specific overload uses the chord-projection handle formula
also for G1/G1 and G2/G2. Interactive `Blend`, surface edges, and other source
types remain unmeasured.

```sh
tools/rhino_oracle/run_headless.sh rhino tools/rhino_oracle/fixtures/blend_lines.json --timeout 600
cargo test -p viboceros-oracle --lib blend_curve::tests
```
