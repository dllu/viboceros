# Loft Refit preparation

[Loft Refit](loft-refit.md) · [Validation](loft-refit-preparation-validation.json)

Smooth curved Refit remains unimplemented. New public Rhino 8.32 measurements
isolate a suitable reference: [NurbsCurve.MakeCompatible](https://mcneel.github.io/rhinocommon-api-docs/api/RhinoCommon/html/M_Rhino_Geometry_NurbsCurve_MakeCompatible.htm) with simplification
method `2`, the complete profile set, no point ends, and the requested Refit
and angle tolerances. Nine equal translated pairs reproduce all control coordinates and
weights of the corresponding loft sections; normalized knot
vectors agree within `2e-16`. The loft then scales its surface domains.

The single-input call differs geometrically or structurally in eight of those
nine cases. Simplifying the inputs first does not explain the difference.
Independent `Curve.Fit`, cubic-Bezier fitting, or single-input `MakeCompatible`
therefore cannot simply replace the multiple-profile preparation. This evidence
does not yet establish how every larger or mixed profile set is fitted.
Six further cases vary profile count, scale and companion shape. For these three
source families at two tolerances, two, three and four equal translated inputs
produce exactly the same complete definition of profile zero. Different
companions can change its basis; control counts alone do not prove a geometric
change. The shared [curve correspondence certificate](cubic-approximation.md)
now checks complete images across distinct parameter clocks. Three paired native
fits certify at `0.001`; this does not implement the fitting policy.

Another 21 cases retain public length and normalized-length queries at three
fractional tolerances. The existing quadrature model reproduces 63 length
estimates, and curve evaluation reproduces 441 positions at the native returned
parameters, within `1e-9`. Scalar normalized-length inversion and incremental
batches have distinct numerical behavior; a `0.0565` position difference is
retained as a counterexample. Inverse-parameter parity and continuous curve
bounds are not established by these length observations.

The [polygon implementation](loft-refit.md) now fits certified straight regions,
preserves resolved corners and matches region charts before constructing a shared
cubic basis. Unclamped rational endpoints remain exact through the final rounding
bound. New native captures retain 171 cases covering spatial and closed polygons,
different corner counts, point ends and the one-degree corner threshold.
Smooth cases and native empty-result cases remain explicit limitations;
[validation](loft-refit-polylines-validation.json) distinguishes them from matches.
The earlier straight-profile references remain regressions. Captures are immutable.

```sh
cargo test -p viboceros-oracle --test loft_refit_lines_native
python3 -m unittest tools.rhino_oracle.test_loft_refit_clock
```
