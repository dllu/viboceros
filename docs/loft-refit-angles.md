# Loft Refit angles

[Loft Refit](loft-refit.md) · [Section preparation](loft.md)

The Rust geometry and native SMLib APIs expose
`try_loft_nurbs_curves_refitted_with_angle` and
`loft_surface_refitted_with_angle`. The Python JSON API accepts
`refit_angle_tolerance` alongside `refit_tolerance`. Angles are radians in
`[0, pi]`; invalid values fail before backend edits. The angle applies to API
construction. Command overrides fail explicitly because the interactive Loft
command does not expose this SDK setting.

The existing one-degree corner resolution floor still applies. A larger explicit
angle can suppress polygon corners, requiring the pending adaptive fitter.
Right-angle corners remain resolved at `pi/2`. Certified single-Bezier proposals
retain their existing complete half-tolerance image bound and native parity
reserve. Positive refits of straight or smooth profiles above `pi/2` remain
unsupported, consistent with the captured empty SDK results.

With fitting disabled (`refit_tolerance=0`), original profile definitions remain
unchanged, but the surface constructor still matches section regions. Higher-degree
profiles can require this matching even when their degrees and knot vectors are
identical. Angles zero and pi use the default region angle; intermediate angles
can change the common parameter chart without changing source control points.
Degree-one profiles with identical knots retain their existing chart.

[Validation](loft-refit-angles-validation.json) compares complete surface
definitions, domains and 17-by-17 samples against 217 immutable Rhino SDK records.
It retains unimplemented adaptive cases and empty native results. This does not
establish general adaptive Loft Refit or performance parity.

```sh
cargo test -p viboceros-oracle --test loft_refit_angles_native --frozen
cargo test -p viboceros-oracle --features native-smlib \
  --test loft_refit_angles_native --frozen
python3 -m unittest tools.rhino_oracle.test_loft_refit_angles
```
