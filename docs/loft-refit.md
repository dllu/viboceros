# Loft Refit

[Loft](loft.md) · [Section rebuilding](loft-rebuild.md)

`Loft Refit=0.001` refits straight profiles to polynomial cubic lines before
lofting. Rational weights, uneven parameter spacing, and continuous multiple
spans are supported. Sources, selection and attributes remain intact; accepting
the result creates one undo step. Interactive Loft accepts `Refit` followed by a
tolerance, or `RefitTolerance=<value>`. `Simplify=Refit` selects the current Refit
tolerance, defaulting to `1e-9`; `Simplify=None` disables simplification. Refit and
Rebuild cannot be supplied together. Zero leaves the original profiles unchanged.

Positive Refit currently requires a clamped, continuous, non-reversing collinear
control polygon with same-sign weights. Exact rational comparisons admit the
profile; a sampled linearity test is insufficient. The cubic has uniform line
speed. Exact handle-rounding bounds guarantee its complete image lies within
the requested distance of the source segment. This preserves the geometric
segment, **not its original parameter correspondence**. Inputs exceeding the
512-control or 256-section limits, and tolerances below representable handle
accuracy, fail explicitly.

Curved profiles remain unsupported and fail before document mutation. Native
captures show that Rhino refits even polynomial quadratics and cubics, changing
their clocks. Neither `Curve.Fit(3, ...)`, the public cubic-Bezier fitter, nor the
separate [certified source-parameter approximation](cubic-approximation.md)
reproduces that general Loft policy. No curved-profile parity is claimed.

The [validation record](loft-refit-lines-validation.json) binds the implementation,
native captures and verification results. The earlier
[investigation](loft-refit-validation.json) retains its historical source bindings.
Native data and content-addressed producer archives remain immutable.

```sh
cargo test -p viboceros-oracle --test loft_refit_lines_native --test loft_refit_native
cargo test -p viboceros-oracle --features native-smlib --test loft_refit_lines_native --test loft_refit_native
python3 -m unittest tools.rhino_oracle.test_loft_refit_lines
```
