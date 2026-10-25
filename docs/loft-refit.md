# Loft Refit investigation

[Loft](loft.md) · [Section rebuilding](loft-rebuild.md)

Positive-tolerance Refit is still unimplemented. The oracle now rejects a
positive `refit_tolerance` explicitly, including command requests, instead of
silently producing an ordinary loft. Zero disables simplification; invalid
negative tolerances and conflicting rebuild requests fail before geometry work.

Fresh Rhino 8.32 captures retain 100 ordered API cases across five profile
families, five Loft styles and four tolerances. All 75 positive-tolerance results
have polynomial cubic sections and retain their input definitions. All 25 zero
cases equal the existing native zero-Rebuild captures. A 16-case pilot repeats
cases in the main family; another 45 cases use equal translated profiles to
isolate the section fitter from common-basis union. Successful captures and their
content-addressed producer archives are immutable.

Neither independent public fitter tested so far explains Loft Refit. At
`tolerance=0.001`, a smooth rational quarter arc produces multiple internal knots
in the loft, while independent `Curve.Fit(3, ...)` sections have only simple
internal knots. With equal translated radius-two quarter arcs, Loft Refit uses
two cubic spans; `BezierCurve.CreateCubicBeziers` returns four, with different
handles. These are measurements, not evidence of a shared hidden algorithm.
The public [cubic-Bezier API](https://mcneel.github.io/rhinocommon-api-docs/api/RhinoCommon/html/M_Rhino_Geometry_BezierCurve_CreateCubicBeziers.htm)
and [Loft Refit API](https://mcneel.github.io/rhinocommon-api-docs/api/RhinoCommon/html/Overload_Rhino_Geometry_Brep_CreateFromLoftRefit.htm)
remain independent reference operations.

The shared curve fitter has been refactored into adaptive policy, parameter
sampling, interpolation/handle solving and separate regression modules. This
moves existing code without changing its arc-length fitting policy. It does not
establish Rhino Refit parity or a continuous approximation bound for that fitter.
A separate [certified cubic primitive](cubic-approximation.md) now supplies
source-parameter fits and whole-curve bounds for the next fitting work. The
[validation record](loft-refit-validation.json) records the checks and limits.

```sh
cargo test -p viboceros-geometry --lib curve_fit
cargo test -p viboceros-oracle --test loft_refit_native
cargo test -p viboceros-oracle --features native-smlib --test loft_refit_native
python3 -m unittest tools.rhino_oracle.test_loft_refit tools.rhino_oracle.test_capture_family
```
