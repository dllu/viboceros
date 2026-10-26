# Fitting-clock queries

[Loft Refit](loft-refit.md) · [Validation](loft-fitting-clock-validation.json)

`ArcLengthSampler::parameters_at_quadrature_normalized_lengths` shares one
Cash–Karp trace across independent numerical inverse queries. Arbitrary order
and repeated fractions retain the same results as separate scalar calls.
`parameter_at_quadrature_normalized_length` is the scalar convenience method.
Stationary plateaus use their right endpoint; natural domain ends remain exact.
Incremental batches keep their existing, prefix-dependent behavior.

The JSON/Python operation `curve_length_parameters` accepts a NURBS `curve`,
`fractions`, and optional `fractional_tolerance`, defaulting to `1e-4` for fitting
investigation. It returns parameters, points, quadrature length, domain and
unchanged before/after definitions. Both oracle engines implement this operation.

Native single-input Refit domains equal the source length estimate at `1e-4`
in 24 measurements. At this precision, 418 ordered/reversed scalar positions
and 19 direct protocol records with 228 reordered/repeated positions match within
`1e-9`. This does **not** establish which sampling sequence the fitter uses.
Adding a second coincident profile changes fitting while zero-simplification
preparation remains the same. Translation does not explain that branch.

Full scalar SDK parity remains unfinished. A retained strict diagnostic compares
2,090 queries at five precisions; it is ignored in the default suite and remains
failing. Other precisions must not be treated as native-compatible on the basis
of the `1e-4` comparisons. Adaptive smooth Loft Refit remains unfinished; the [initial proposal](loft-refit-candidates.md)
now has a separate continuous acceptance bound.

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
