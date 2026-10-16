# Local transitions for Gordon curve-family fitting

`fit_gordon_curve_family_with_clock(inputs, interval_spans, fit_tolerance,
numerical_tolerance, policy)` adds an explicit clock policy to the existing
shared cubic fitter. `GordonCurveFitClock::WholeInterval` retains the original
quintic clock. The existing `fit_gordon_curve_family` entry point continues to
select that policy.

`GordonCurveFitClock::Local { transition_fraction }` keeps an affine arc-length
clock between endpoint transition windows. Each window occupies the supplied
fraction of source arc length between consecutive contacts, in `(0, 0.5]`.
Each curve uses one speed at each contact. Quintic Hermite formulas connect it
to the neighboring affine speeds with zero second derivative at the joins.
The smaller adjacent secant supplies each common contact speed. Positive stored Bernstein increments
qualify the transition pieces; collapsed windows or controls reject. When two
half windows meet, both use the same literal midpoint, including nonbinary
contact values. Endpoint evaluation retains the literal contact without
extrapolating through inverse-coordinate roundoff.

This changes only private parameter proposals. Equal-arc-length sampling,
usd-brep constrained least squares and bounded geometric refitting retain their
existing roles. All original family, degree, control, sample and native work
limits remain. Every returned curve still shares the prescribed simple cubic
basis and certifies each actual restored world contact. The sampled fit limit
is unchanged, and a downstream surface must separately certify every complete
pre-fit original curve. Neither clock certifies a continuous curve fit by
itself or guarantees surface topology.

The new regression builds a C2 surface from six fitted quadratic sources at
`1e-7`, then requires complete original-image proofs at `1e-6`. The original
whole-interval regression also passes. Other cases cover affine clock centers,
first and second arc-length jets at joins, nonbinary endpoints and midpoints,
collapsed charts, invalid widths, independent source domains, negative rational
weights and unchanged sources. Local windows of `0.1`, `0.25` and `0.5` also fit
the nonbinary-station line family in its existing 11-control basis.

The extended `network_family_parameterization` example accepts an optional
second argument specifying the local transition fraction. With no second
argument it retains the whole-interval policy. Schema two records the selected
policy explicitly. Common stations still come only from the original curves'
mean normalized contact arc lengths; saved Rhino data supplies comparison
outputs only.

The [local-clock observation](../tools/rhino_oracle/observations/network_parameterization_local_clock_viboceros_20261016.json)
retains all six original-image proofs on the actual constructed surface:

| Policy | Tensor controls | Interior knot multiplicity | Largest sampled fit distance | Largest Rhino query-output difference |
| --- | --- | --- | --- | --- |
| Earlier whole-interval clock | 123×123 | 1 | `3.98e-8` | `0.005695` |
| Local half windows | 123×123 | 1 | `6.56e-8` | `0.0007353` |

All six local original-image bounds are below `1e-6`. The local tensor retains
C2 continuity and improves this comparison by about 7.7 times at the same
degree and control count. The earlier crossing-constrained arc-length
candidate, with repeated interior knots, remains slightly closer at about
`0.0007333`. The local policy remains experimental and has not been selected as
an application default.

The full `1e-6` Rhino query comparison still fails. The separate
[reference audit](network-source-reference-audit.md) retains an exact
incompatibility between one saved reference output and the simultaneous source
preservation and closest-point comparison requirements. No check, tolerance or
proof budget changes here. NetworkSrf remains unregistered; common station
selection, independent interior fitting, broader preparation and performance
qualification remain unfinished.

```sh
cargo run --release -p viboceros-smlib --features native \
  --example network_family_parameterization -- 60 0.5 > /tmp/local-gordon.json
```

The [validation record](local-gordon-curve-clock-validation.json) retains native
and oracle regressions, compiler/sanitizer checks and the complete observation.
No new licensed capture was needed.
