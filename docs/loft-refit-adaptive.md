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
acceptance: a missing complete interval bound triggers further subdivision,
including features between stations. The complete assembled curve still needs a
fresh continuous Bernstein bound within the requested tolerance.
Correspondence proposals retain natural source knots within their existing
256-interval budget, so nonuniform source clocks do not hide span boundaries.
The [refinement validation](adaptive-refinement-validation.json) records small
feature, reversal, model-unit and resource-limit regressions alongside 16 native
profiles whose features fall between all nine global length stations.

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
command fitting remains unfinished.

Twenty-four further native records vary only the source knot spacing. Equal
source images can produce different fitted control counts, so a polygon proxy
cannot establish native subdivision by geometric equivalence alone. Direct
[interval-clock measurements](interval-clock-validation.json) retain the
corresponding scalar length queries; they do not qualify a sampling rule.
Sixty-four source inspections preserve fitted definitions under degree elevation,
affine chart changes and polyline simplification. Every measured source has a
C1 break but no G1/G2 break; that distinction alone does not select subdivision.
`NurbsCurve::parametric_discontinuity_parameters` exposes the public OpenNURBS
C0/C1/C2 query alongside geometric continuity, retaining the source chart.
[Source-continuity validation](source-continuity-validation.json) records these
comparisons and limiting derivatives without extending adaptive admission.

## Original source charts

`try_propose_joined_refit` constructs pieces directly from the original source.
One nine-station incremental length lookup supplies all dyadic split locations,
using interpolation inside that lookup for finer cuts. Interval checks retain
their local stations and complete bounds. The existing polygon-proxy APIs keep
their interval-local split clock; this new proposal does not replace them.

Raw assembly and final certification are separate. Locally bounded pieces can
form an uncertified raw curve, while their retimed Hermite reconstruction has a
complete bound against the original source. Only that final bound admits the
joined proposal; an unproved intermediate curve never escapes the API.

The new construction reproduces all 48 curved holdouts across styles, commands,
frames, reversal, weight gauges, affine domains and model units at `1e-9` when
the proposed sections enter the existing Loft path with further fitting disabled.
Twelve curved-clock transitions and the earlier 31 scale/curvature records also
match. This does not establish command admission: only seven of 24 uneven
polyline cases match, and twelve independent smooth, rational, spatial and
degree-five holdouts retain ten unproved proposals and two differing control
nets. [Source-chart validation](loft-refit-original-source-validation.json)
records those counterexamples. General adaptive command preparation remains
unfinished; the existing command policy is unchanged.

Invalid inputs, exhausted budgets and unresolved certificates produce no partial
fit or source mutation. Limits remain 128 pieces, depth 24 and 512 source
controls, plus the existing exact-arithmetic and correspondence budgets.
[Earlier raw-piece measurements](loft-refit-adaptive-validation.json) retain
historical bindings and immutable native captures.

```sh
cargo test -p viboceros-geometry --lib loft::refit --frozen
cargo test -p viboceros-oracle --test loft_refit_adaptive_join_native \
  --test loft_refit_original_source_native --frozen
python3 -m unittest tools.rhino_oracle.test_loft_refit_adaptive_join \
  tools.rhino_oracle.test_loft_refit_sampling_stages
```
