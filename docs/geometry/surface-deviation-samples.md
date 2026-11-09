# Surface deviation witnesses

[Complete UV bounds](surface-correspondence.md) · [Planar image bounds](surface-planar-images.md)

`NurbsSurface::deviation_samples(other, divisions, numerical_tolerance)` compares
a regular normalized U/V grid on two surfaces while retaining their original
parameter domains. It returns three summaries: corresponding parameter pairs,
first-to-second local witnesses and second-to-first local witnesses.

Each summary stores its sample count, local refinement failure count, and the
point pair with greatest sampled distance. A witness includes both model-space
points and their natural parameters, so it can be drawn or re-evaluated.
The directional comparisons start local closest-point continuation at matching
normalized parameters. If continuation fails, the valid seed pair is retained
and the failure is counted. A refinement that increases distance is discarded.

These are diagnostics, not complete bounds or guaranteed global nearest points.
Zero sampled error can miss an interior feature. A regression places a unit
spike entirely between a 16-division grid's stations; a finer grid finds it and
the complete tensor certificate rejects the coarse grid's apparent agreement.
Sampling never replaces the fitting APIs' certificate gates.

Two query-owned evaluation caches are reused throughout the comparison.
Inputs remain immutable. Supported limits are 1..64 divisions in each direction,
degrees through 16 and at most 4096 controls per source. Failed grid evaluation
returns an error; failed local continuation is reported in the summaries.

`closest_parameters_from_seed(point, seed, numerical_tolerance)` is also public
for callers that already own a reliable chart seed. Finite seeds are clamped
to the active domain; nonfinite seeds are rejected. This is local continuation.
`closest_parameters` provides the existing bounded multi-start search.

The local JSON operation `surface_deviation_samples` accepts `first` and
`second` surface definitions and optional `divisions` (default `[16,16]`).
Request tolerance controls numerical refinement. The response always includes
`certified:false`; each summary exposes `maximum_witness_distance`,
`sample_count`, `refinement_failures` and the winning `witness`.

The [public fitting investigation](../surface-fit-achieved.md) uses these
witnesses alongside independent complete output comparisons. Reproduce its
offline proposal metrics with:

```sh
cargo run -p viboceros-oracle --example surface_fit_metric_experiment
```
