# Loft Refit preparation

[Loft Refit](loft-refit.md) · [Validation](loft-refit-preparation-validation.json)

General curved Refit remains unimplemented. New public Rhino 8.32 measurements
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

The straight-profile implementation now handles unclamped active domains through
bounded exact rational extraction, without first rounding a clamped copy. Its
cubic endpoint and handle errors are checked against the exact active segment.
A separate 90-case family covers distinct unclamped polynomial/rational profiles,
mixed line clocks, reversed traversal and shifted domains across all five styles.
The earlier 121 straight-profile references remain regressions. Native captures
are immutable and retain their exact producer archives.

```sh
cargo test -p viboceros-oracle --test loft_refit_lines_native
python3 -m unittest tools.rhino_oracle.test_loft_refit_clock
```
