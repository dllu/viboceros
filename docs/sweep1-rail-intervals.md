# Sweep1 rail continuity

[Sweep1 placement](sweep1-placement.md) · [Validation](sweep1-arc-parameters-validation.json)

Sweep1 checks corners using the original rail's one-sided tangents. Only knots
strictly inside the swept interval participate; native analytic parameters are
mapped to the original rational chart before checking its knot groups.

A trim just before a smooth repeated knot can leave a span only a few
floating-point steps wide. Extracting that span can round distinct controls to
the same point. Previously, the resulting zero tangent caused an erroneous
"rail corners require sweep miter construction" rejection. Checking the source
preserves its continuity without snapping the requested station or loosening
the angular tolerance.

The regressions cover a translated exact quadratic parabola and a translated
rational arc, forward and reversed. They retain the requested interval and
check fitted surface samples against independent parabola/circle equations.
These sampled fit checks are separate from the complete-image arc comparisons.
Genuine corners still fail even with one floating-point step of incoming span
left. Corners exactly at an interval boundary are excluded; extending that
interval to the full rail checks them again. Closed-rail seam validation is
retained.

Command regressions also build these arc sweeps through automatic and explicit
stations on both backends. They check analytic surface samples, preserved inputs
and atomic Undo.

This fixes continuity validation. The remaining rational near-Greville interval
and direction choices still differ from some Rhino captures.

```sh
cargo test -p viboceros-geometry --test sweep_knot_continuity --test sweep_anchoring --frozen
cargo test -p viboceros-command --test sweep_knot_continuity --features native-smlib --frozen
```
