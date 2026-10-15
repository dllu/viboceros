# Bounded curve approximation

`NurbsCurve::try_approximate_polyline(tolerance)` produces a polyline whose
parameters use the original NURBS domain. Every segment has a continuous
position-error bound at the corresponding parameter, at most the caller's
absolute tolerance. Knot boundaries remain vertices. Rational parameter speed,
collinear reversals and closed curves are retained within that positional bound.

The implementation reuses the exact rational span extractor and Bernstein
difference algebra from the surface correspondence certificates. Extracted nets
reuse exact subdivision when the stored native midpoint is exactly halfway;
otherwise they restrict the original spline at the actual stored parameter.
Rounded knot insertion and sampled checks do not decide acceptance. Rounded output
endpoints participate in the certificate. The bound describes the stored curve
and polyline mathematically; ordinary evaluation still has floating-point error.

Degrees through 16 and common-sign rational weights are supported, including
uniformly negative gauges. Mixed-sign weights, discontinuities, constant or
unresolved closed segments, exhausted parameter precision and arithmetic/output
limits decline without changing the source. The shared certificate engine limits
work to 2,000,000 units, rational numerators/denominators to 8,192 bits and
subdivision depth to 48. Output also uses the existing curve-division point limit.
Positional approximation does not certify topology or preserve sub-tolerance
features. This API does not claim Rhino's tessellation or vertex-count parity.

Mesh Pull preparation also exposes
`MeshProjectionIndex::closest_location(point)`, returning the closest point and
its indexed triangle. Quad triangles follow the existing shorter-diagonal split;
these indices differ from source mesh face IDs. Equal-distance ties retain the
lowest triangle index. Existing `closest_point` delegates to the same query.

These APIs now feed [mesh curve pulling](commands/pull-mesh.md), including
folded-face transitions. Discontinuous closest branches can still decline.
The original qualification below records the foundation before command wiring;
mesh tracing has a separate qualification. No live Rhino or performance parity
is claimed by either record.

See the [qualification record](curve-approximation-qualification.json) for
validation and source provenance.

```sh
cargo test -p viboceros-geometry --lib surface_pullback::certificate::polyline
cargo test -p viboceros-geometry --lib mesh::projection::tests
```
