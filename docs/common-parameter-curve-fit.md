# Common-parameter cubic curve fitting

`viboceros_geometry::try_fit_cubic_curve_at_stations` fits a polynomial cubic
through `CurveFitStation` constraints. The native equivalent is
`viboceros_smlib::fit_cubic_curve_at_stations`; an external-solver geometry API
is also available. Each station supplies an original source parameter, a
requested unit output parameter and a target position. Both original endpoints
must be included at output parameters zero and one. Caller order is retained
in `ConstrainedCurveFit::constraint_parameters`.
The input list may be unordered; sorting by source arc length must yield
strictly increasing output parameters.

The fitter reuses the existing arc-length sampler, adaptive error checks,
fixed endpoint/corner handles, five-diagonal interpolation and exact rational
stored-curve position checks. The native path uses the existing usd-brep
fixed-row solver. Legacy arc-length fitting keeps its original domain and
parameter policy.

A scalar B-spline interpolates the station pairs `(output parameter,
normalized source arc length)` with degree `min(3, station_count - 1)` and
averaged interpolation knots. It reuses the existing small interpolation
solver. Strictly positive derivative controls prove that the stored map is
increasing everywhere. A map that cannot prove this is rejected; the fitter
does not clip an overshooting interpolant into a different map. Natural source
breaks are inverted into the new chart with bounded bisection. Known station
parameters and their original arc distances are retained directly.

Endpoint and retained-corner derivative magnitudes follow the chain rule:
source unit tangent times total source length times the scalar-map derivative.
Simple cubic knots preserve C2 continuity away from retained source corners.
The resulting surface from a later Gordon construction has its own continuity
requirements; smooth fitted curves alone do not qualify that surface.

The limits are 2–64 stations and 4,096 fitted controls. Source and output
stations must increase together and cover both endpoints. Nonfinite values,
collapsed normalized distances or knot spans, nonpositive map derivative
controls and conflicting positions reject. An oversized natural source-span
list rejects before repeated map inversion. Original source geometry is never
changed.

Position constraints require exact rational bounds at absolute numerical
tolerance. Error elsewhere remains adaptive sampled fitting. This API does
not supply a continuous source-to-fitted-curve certificate, and it does not
claim Rhino NetworkSrf fitting parity.

## Network parameterization investigation

The native example compares three construction choices on the retained
`biquadratic_network` fixture:

```sh
cargo run --release -p viboceros-smlib --features native \
  --example network_parameterization > /tmp/network-parameterization.json
```

The example uses saved public SDK output, privately owned source curves and
the same native Gordon constructor for each candidate. It also reads the
saved Rhino tensor in Rust and samples the parameter traces of all six input
curves. No new licensed reference capture is required. The retained
[observations](../tools/rhino_oracle/observations/network_parameterization_viboceros_20261016.json)
and [validation record](common-parameter-curve-fit-validation.json) make the
experiment reproducible.

| Candidate | Maximum returned query-point difference from Rhino |
| --- | ---: |
| Raw Gordon network | approximately `0.0274` |
| Crossing-constrained arc-length refits | approximately `0.000733` |
| Global smooth station maps | approximately `0.00960` |

These are distances between returned closest-point outputs, not continuous
surface separation or Hausdorff bounds. Every supplied candidate curve image
passes the constructor's complete surface-image bound at `1e-6`. For refitted
inputs that proves the fitted images; their pre-fit originals still need a
separate continuous certificate.

The probe now also performs [original-curve admission](original-network-curve-admission.md)
and emits schema two with per-curve bounds or errors. All six originals certify
for the smooth candidate; the arc-length candidate certifies five, with one
proof reaching the unchanged work limit. The linked schema-one capture above
is retained as historical evidence.

The arc-length candidate improves this fixture's interior comparison. The
tested global smooth map worsens it. Neither passes the `1e-6` query comparison
or qualifies NetworkSrf. Saved-tensor traces keep each source curve close to a
constant surface coordinate but show different parameter distributions from
the tested candidates. Those sampled traces guide further fitting work; they
do not prove exact isoparametric containment.

NetworkSrf remains unregistered in the application. Further work must resolve
the surface fitting policy, continuity, complete original-curve admission and
broader Rhino comparisons. The global scalar-map policy is a curve-fitting
building block, not the selected NetworkSrf algorithm.
