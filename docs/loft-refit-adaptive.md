# Adaptive Refit piece proposals

[Loft Refit](loft-refit.md) · [Validation](loft-refit-adaptive-validation.json)

`try_propose_adaptive_linear_refit` constructs certified C0 cubic pieces for
exactly extracted straight-region sources without resolved one-degree tangent
breaks. It returns the raw curve, normalized length breaks and a complete
symmetric image-distance bound. The existing Loft command does not use this
proposal while native joining remains unqualified.

An exact polygon proxy retains the directed source image and limiting tangents
while removing nonlinear rational clocks. Its vertices must be exactly
representable. Each interval reuses the shared endpoint-tangent cubic constructor;
an interior endpoint uses the original right-hand tangent, which can differ from
the tangent of an independently trimmed curve. Seven interior length stations
select pieces, recursively bisecting intervals that exceed half the tolerance.

Sampling only proposes the pieces. A complete Bernstein certificate must bound
the entire assembled curve within the requested tolerance, including errors
between stations. The half-tolerance sampling reserve is not a continuous bound;
a measured shallow-bend case exceeds it between samples. Invalid tolerances,
unsupported regions, exhausted budgets and unresolved certificates produce no
partial fit. Limits are 128 pieces, depth 24 and 512 source controls, plus the
existing exact-arithmetic and correspondence budgets.

Thirty native records reproduce the raw piece controls within `1e-9` after
restricting Rhino's final curves to their pieces. This does not establish the
final joined control net. Thirty-six fresh scale, tolerance and uneven-bend
records retain different native bases; a scaled quarter-degree case has eight
controls where its unscaled counterpart has ten. The proposed small-budget
knot-removal and region-matching model did not reproduce all these cases and
was removed. [Diagnostic angle checks](loft-refit-probes.md) now distinguish
public single-knot parity from composite fitting and region matching. No command
acceptance or existing fitting tolerance changed.

```sh
cargo test -p viboceros-geometry --lib loft::refit::adaptive --frozen
cargo test -p viboceros-geometry --test adaptive_linear_refit_native --frozen
python3 -m unittest tools.rhino_oracle.test_loft_refit_adaptive_lines
```
