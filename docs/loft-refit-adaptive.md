# Adaptive Refit proposals

[Loft Refit](loft-refit.md) · [Joining validation](loft-refit-joining-validation.json)

`try_propose_adaptive_linear_refit` constructs certified C0 cubic pieces for
exactly extracted straight-region sources without resolved one-degree tangent
breaks. It returns the raw curve, normalized length breaks and a complete
symmetric image-distance bound. These proposals remain separate from the Loft
command because native adaptive acceptance is unfinished.

An exact polygon proxy retains the directed source image and limiting tangents
while removing nonlinear rational clocks. Its vertices must be exactly
representable. Each interval reuses the shared endpoint-tangent cubic constructor;
an interior endpoint uses the original right-hand tangent. Seven interior length
stations propose pieces with a half-tolerance reserve. Sampling cannot establish
acceptance: the complete assembled curve needs a continuous Bernstein bound
within the requested tolerance, including errors between stations.

`try_propose_joined_linear_refit` adds composite C1 reconstruction. It partitions
the raw curve by G2 breaks using an absolute curvature tolerance of `0.01` in
inverse model units. Inside each region, adjacent handle lengths determine new
piece widths. Cubic Hermite reconstruction evaluates that retimed curve at the
original break parameters, producing double interior knots. Distinct regions
retain triple knots. The joined result requires a fresh complete source bound
within the requested tolerance; reconstruction receives no additional allowance.
The explicit angle uses the [existing corner policy](loft-refit-shallow.md).

Thirty-one scale and curvature-boundary records reproduce complete native Loft
surfaces within `1e-9` when these proposals enter the existing common-basis
preparation. Unequal-region and reversed-source holdouts still expose different
subdivision decisions. Three interior stations improve some cases but fail
others, so that change was rejected. Isolated intervals fit in Rhino across
translations, reversals and parameter clocks. Public cubic-Bezier construction
also produces different pieces from paired Loft preparation; it cannot serve as
a raw-piece oracle for the remaining fitting policy. Curved-source adaptive
fitting remains unfinished.

Invalid inputs, exhausted budgets and unresolved certificates produce no partial
fit or source mutation. Limits remain 128 pieces, depth 24 and 512 source
controls, plus the existing exact-arithmetic and correspondence budgets.
[Earlier raw-piece measurements](loft-refit-adaptive-validation.json) retain
historical bindings and immutable native captures.

```sh
cargo test -p viboceros-geometry --lib loft::refit --frozen
cargo test -p viboceros-oracle --test loft_refit_adaptive_join_native --frozen
python3 -m unittest tools.rhino_oracle.test_loft_refit_adaptive_join \
  tools.rhino_oracle.test_loft_refit_sampling_stages
```
