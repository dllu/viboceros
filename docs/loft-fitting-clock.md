# Fitting-clock queries

[Loft Refit](loft-refit.md) · [Validation](loft-fitting-clock-validation.json)

`ArcLengthSampler::parameters_at_quadrature_normalized_lengths` shares one
Cash–Karp trace across independent numerical inverse queries. Arbitrary order
and repeated fractions retain the same results as separate scalar calls.
`parameter_at_quadrature_normalized_length` is the scalar convenience method.
Natural knots participate in numerical scalar acceptance even when each span
has constant speed. Nearby stationary plateaus use their right endpoint;
natural domain ends remain exact.
Incremental batches keep their existing, prefix-dependent behavior.

The JSON/Python operation `curve_length_parameters` accepts a NURBS `curve`,
`fractions`, and optional `fractional_tolerance`, defaulting to `1e-4` for fitting
investigation. It returns parameters, points, quadrature length, domain and
unchanged before/after definitions. Both oracle engines implement this operation.

Native single-input Refit domains equal the source length estimate at `1e-4`
in 24 measurements. At this precision, 418 ordered/reversed scalar positions
and 19 direct protocol records with 228 reordered/repeated positions match within
`1e-9`. These scalar measurements do **not** establish a fitter sampling sequence.
The [handle correction](loft-refit-adjustment.md) uses a separately qualified
incremental batch.
Adding a second coincident profile changes fitting while zero-simplification
preparation remains the same. Translation does not explain that branch.

A further 4,356 scalar queries cover near-knot acceptance at five precisions,
reversed order, frames, units, negative weight gauges, collinear and orthogonal
regions, and stationary spans. They match within `1e-9`; the previous constant-speed
shortcut bypassed these numerical stopping rules. Precise geometric inverses and
incremental batch behavior remain unchanged. [Knot-query validation](scalar-knot-validation.json)
retains these measurements and separate fitter counterexamples.

Full scalar SDK parity remains unfinished. The retained strict diagnostic compares
2,090 queries at five precisions and still fails on 14 curved-source queries;
it remains ignored in the default suite. The knot-family measurements do not
establish general parity at every precision. Paired fitting also differs from a
simple one-percent scalar midpoint model in the new boundary holdouts.
Adaptive smooth Loft Refit remains unfinished; [smooth proposals](loft-refit-candidates.md)
have a separate continuous acceptance bound.

Fresh overshoot references retain 24 direct queries with 768 positions, plus
nine straightness/degree queries with 45 positions. They distinguish early
fraction behavior from degree elevation and source-domain changes. Simple
first-checkpoint and endpoint-first substitutions regress other native queries
and were rejected. The [adapter validation](native-nurbs-adapters-validation.json)
records the strict diagnostic without claiming those queries now match.

Tolerance must lie strictly in `(0, 1)`. Kernel batches allow 1,048,576 fractions;
the oracle accepts 4,096. Preparation and inverse refinement each allow 65,536
steps, with 80 bracket updates per root. Errors yield no partial result or source
mutation. Quadrature stopping errors are estimates, not continuous certificates;
precise geometric queries retain the separate distance-inversion API.

```sh
cargo test -p viboceros-geometry --test scalar_clock_native
cargo test -p viboceros-oracle --test fitting_clock_native
# Known failing full scalar comparison:
cargo test -p viboceros-geometry --test scalar_clock_native -- --ignored --nocapture
```
