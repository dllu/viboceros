# Curve-area oracle

`tools/rhino_oracle/fixtures/curve_area.json` covers analytic circles/ellipses,
a closed cubic and its reversal, a quadratic/line polycurve, a rational circle,
and translated/reparameterized cubics. Native tests check all eight against
analytic areas and verify that the public `Area` command agrees with the kernel
without changing its temporary document, selection, or history.

The Rhino worker measures the public `AreaMassProperties.Compute(curve)` API,
not the interactive `_Area` command. It owns and disposes the curve and every
mass-properties result, including warm-up results. No Rhino document objects
are created. The ellipse is represented as an exact rational NURBS in Rhino.

## Retained comparison

The [raw report](curve-area-measurement.json) was collected on 2026-09-08 using
the licensed Rhino 8 Wine/FEX installation in a private Xvfb session:

```sh
tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/curve_area.json \
  --timeout 180 --absolute-epsilon 1e-8 --relative-epsilon 1e-10
```

Six cases pass; the report deliberately retains two failures. Native ellipse
area is `18.849555921538759` (6π), versus Rhino's `18.849556154152708`.
Native rational-circle area is `12.566370614359174` (4π to rounding), versus
Rhino's `12.566370769435153`. Maximum disagreement is about `2.33e-7`.
Native results match the independent analytic references; the comparison
threshold was not relaxed to hide these differences.

Rhino's documented [curve overload](https://mcneel.github.io/rhinocommon-api-docs/api/RhinoCommon/html/M_Rhino_Geometry_AreaMassProperties_Compute_4.htm)
offers planarity tolerance, not an integration-tolerance argument. A separate
[B-rep overload](https://mcneel.github.io/rhinocommon-api-docs/api/RhinoCommon/html/M_Rhino_Geometry_AreaMassProperties_Compute_2.htm)
does expose integration tolerances. Default numerical integration is a plausible
explanation for the conic differences, not a confirmed diagnosis of Rhino's
internals. A tolerance-controlled B-rep comparison remains separate future work.

The report contains only one timed iteration per case after warm-up, collected
alongside native tests. Its timing ratios are not a reliable performance
benchmark. These eight examples do not establish general curve-area parity,
self-intersection semantics, or interactive Rhino command parity.
