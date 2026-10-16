# Automatic fitting of Gordon networks

The native kernel exposes `fitted_gordon_network_surface(curves,
edge_tolerance, interior_tolerance, numerical_tolerance, clock)`. It discovers,
orders and orients the crossing families, chooses common stations, refines
shared cubic curve bases, constructs the Gordon tensor and certifies every
complete pre-fit original curve. Callers no longer have to assemble prescribed
families or choose a fitting grid themselves. This remains an experimental
kernel producer; the application NetworkSrf command is not registered.

`FittedGordonNetwork` returns the surface, fitted curves, original family
indices, common stations, accepted span counts, fitting attempt counts and
sampled fit diagnostics. Its `source_images` retain each original input index,
a UV curve in that input's original domain, and a complete world-space distance
bound. Each UV curve has its prescribed fixed coordinate. Sources stay intact,
including reversed directions, independent domains and rational weight gauges.

## Construction and admission

The producer reuses [raw Gordon discovery](smlib-gordon-network.md),
[shared family fitting](shared-gordon-curve-fit.md), the optional
[local clock](local-gordon-curve-clock.md), and strict
[prepared construction](prepared-gordon-families.md). Native intersections
propose contacts. Missing pairs can also use the existing portable intersection
search and a closest-point proposal on an exact native translational sweep:
`A(u) - B(v) + B(start)`, queried at `B(start)`. Positive-weight control hulls
and the exact point-distance predicate exclude separated pairs cheaply.
Actual source positions qualify the targets. These searches do not certify
contact completeness, uniqueness or surface topology.

Contact arc lengths are measured on private normalized source copies. Their
normalized fractions supply averaged common stations. Interior contact targets
use paired source midpoints. A natural endpoint takes priority over its
partner's interior proposal; a corner midpoint must satisfy both original
endpoint constraints. Complete natural inputs are preserved instead of being
trimmed to approximate intersection parameters.

The curve-fit budget is one tenth of the smaller model tolerance. Original
target constraints are prevalidated before native fitting. Seed-only native
constrained least squares starts at four uniform spans per contact interval,
then doubles the count up to the existing control caps. Only a typed measured
fit rejection requests refinement. Other errors stop. At the final grid, the
existing bounded parameter correction and geometric refitter may improve the
candidate.

Sample measurements evaluate every candidate curve point independently of
whether the tighter Newton stopping test converged. They remain diagnostics,
not continuous fit certificates. The final surface must also pass strict
complete fitted-curve admission and complete original-image admission at each
source's requested edge/interior tolerance. If a general pullback proposal
varies its nominal fixed coordinate, that coordinate is restored and the
modified UV curve is proved again against the unchanged original surface and
source. A failed or inconclusive proof rejects output.

## Limits and checks

Existing limits remain: 4–64 original curves, 2–32 per family, degree at most 16,
128 controls per original and 2,048 total original controls. Curves must be
open, continuous, supported by the common-sign weight importer, and terminate
at the outer-family contacts. Prepared fitted curves also share a 2,048-control
total budget and retain the 128-control individual cap.

The first family reserves the second family's minimum basis before refining;
the second uses the budget left by the accepted first basis. A 2-by-32 line
network therefore succeeds without an unnecessarily small uniform control cap
on every source. A balanced 17-by-17 network rejects before fitting because
even its minimum bases exceed the prepared budget. Uniform grids permit at
most six seed levels and one final geometric refit. The native tensor limit
and all certificate work/bit/depth limits remain unchanged. General near-gap
optimization, closed networks, injectivity, boundary surface continuity and
performance qualification remain unfinished.

Regressions cover remote transformed/reversed inputs and original domains,
curved-grid refinement, complete original-image witnesses, negative rational
gauges, uneven family counts, invalid topology and budgets, and a `5e-8`
interior contact gap with intact natural endpoints. Existing raw construction
and fixed-family fitting retain their regression coverage.

## Retained comparisons

The [quadratic and rational observations](../tools/rhino_oracle/observations/fitted_network_viboceros_20261016.json)
and [nonuniform observation](../tools/rhino_oracle/observations/fitted_network_nonuniform_viboceros_20261016.json)
retain fitted curve definitions, complete UV witnesses, all 20 original-image
bounds and 75 saved SDK query comparisons. Source data alone chooses the
families, stations and grids; reference geometry never feeds fitting.

| Fixture | Accepted U/V spans per interval | Tensor controls | Largest sampled fit distance | Largest Rhino query difference |
| --- | --- | --- | --- | --- |
| Biquadratic | 62 / 62 | 127×127 | `5.75e-8` | `0.0007354` |
| Rational cylinder | 16 / 4 | 35×11 | `2.26e-8` | `5.478e-6` |
| Nonuniform | 41 / 41 | 126×126 | `9.29e-8` | `0.001413` |

All three tensors are cubic with simple interior knots and all 20 complete
original-image bounds satisfy `1e-6`. The `1e-6` Rhino output comparison still
fails on all three fixtures. The separate
[reference audit](network-source-reference-audit.md) retains an exact conflict
between one reference output and simultaneous source preservation/output
matching. No mismatch is relabeled as passing and no limit changes here.

```sh
cargo run --release -p viboceros-smlib --features native --example fitted_network \
  > /tmp/fitted-network.json
cargo run --release -p viboceros-smlib --features native --example fitted_network \
  nonuniform_network > /tmp/fitted-network-nonuniform.json
```

The [validation record](automatic-gordon-network-fit-validation.json) retains
the tests, sanitizer/compiler checks and observation hashes. No new licensed
Rhino capture was needed.
