# Fitting-clock queries

[Loft Refit](loft-refit.md) · [Validation](loft-fitting-clock-validation.json)

`ArcLengthSampler::parameters_at_quadrature_normalized_lengths` shares a
Cash–Karp trace across independent numerical inverse queries. Arbitrary order
and repeated fractions retain the same results as separate scalar calls.
`parameter_at_quadrature_normalized_length` is the scalar convenience method.
Natural knots and interior integration checkpoints participate in stopping even
on constant-speed NURBS spans. Nearby stationary plateaus use their right endpoint.
A natural domain endpoint within the stopping error takes precedence over
interior checkpoints. Polyline and analytic constant-speed clocks remain exact.

Between NURBS checkpoints, constant-speed intervals use affine inversion.
Precise geometric inverses and prefix-dependent incremental batches retain their
separate policies. Numerical stopping errors are estimates, not continuous
distance certificates or a qualified adaptive Loft sampling rule.

Incremental NURBS batches normalize requests below `1e-5` unchanged; coarser
requests use `min(tolerance, 0.01) / 100` for integration and stopping. This policy
now also covers curved spans. The measured fitting clock requests `0.01`, giving
an effective tolerance of `1e-4`. Constant-speed multispan NURBS retain the
numerical trace and previous accepted root, so nearby fractions can return the
same parameter. Polyline, scalar and precise queries keep their own clocks. The
[batch validation](nurbs-batch-validation.json) compares 7,452 cold/prequeried and
spatial holdout positions, 900 Loft sampling positions and 216 original-NURBS
component positions with Rhino. Prior length, scalar and batch queries did not
change the controlled results. A full trace is necessary: retaining only natural
knots misses interior checkpoints and the accepted prefix.
The [curved-batch replay](sweep1/refit-clock-validation.json) adds 450 single-span,
multi-span and spatial precision checks plus 144 represented-profile positions.

The JSON/Python operation `curve_length_parameters` accepts a NURBS `curve`,
`fractions`, optional `fractional_tolerance` (default `1e-4`) and an optional
`subdomain: [start, end]` inside the active source domain. Both oracle engines
return parameters in the source chart, points, estimated length, the queried
domain and unchanged whole-source before/after definitions. Invalid intervals
fail without partial output.

At fitting precision `1e-4`, 418 ordered/reversed scalar positions and 19 protocol
records with 228 reordered/repeated positions match Rhino within `1e-9`.
Single-profile fitted domains match the corresponding length estimates in 24
measurements. Further [knot-query](scalar-knot-validation.json),
[source-type](constant-clock-validation.json) and
[interval](interval-clock-validation.json) comparisons retain their own evidence.
These clock comparisons do not establish the paired fitter's subdivision policy.

[Endpoint validation](scalar-endpoint-validation.json) compares 3,760 endpoint
and exact-tolerance-threshold positions across 129 further native records.
Complete neighboring-value and interior-query comparisons remain strict ignored
diagnostics. The original general scalar diagnostic also retains 14 mismatches
among 2,090 queries. First-checkpoint and refinement-endpoint substitutions
regress other references and remain rejected. General scalar SDK parity and
[adaptive Loft fitting](loft-refit-adaptive.md) remain unfinished.

Tolerance must lie strictly in `(0, 1)`. Kernel batches allow 1,048,576 fractions;
the oracle accepts 4,096. Preparation and inverse refinement each allow 65,536
steps, with 80 bracket updates per root. Errors yield no partial result or source
mutation.

```sh
cargo test -p viboceros-geometry --test scalar_clock_native --test scalar_checkpoint_acceptance_native
cargo test -p viboceros-oracle --test fitting_clock_native
# Retained strict diagnostics:
cargo test -p viboceros-geometry --test scalar_clock_native --test scalar_checkpoint_acceptance_native -- --ignored --nocapture
```
