# Original network-curve admission

Certified pullback now tries a constant-coordinate proposal when the verified
UV endpoints share a U or V coordinate. The existing derivative-free cubic
fitter searches a numerical isocurve for its UV nodes. The extracted curve is
only a proposal: acceptance still checks the complete original spatial curve
against the original stored surface tensor. The assembled returned UV curve
receives a final certificate, including any rounding in its knots and controls.

Misleading hints decline this proposal and fall through to the existing general
fitter. Diagonal endpoint pairs do not activate it. The original source domain
and fixed endpoints remain intact. Tests exercise both directions, a curved
nonisoparametric path with aligned endpoints, and an off-surface interior
perturbation whose endpoints remain valid.

The interval certificate already handles variable Bernstein source degrees.
Its caller now enables that existing path for linear and quadratic spans as
well as cubics. Independent exact-rational proofs check its accepted and
rejected decisions on rational low-degree sources. Hidden-excursion tests
verify that matching ordinary sample nodes cannot admit an incorrect image.
The outward rounding, exact fallback, absolute tolerance, arithmetic/depth
limits and two-million-operation exact work budget are unchanged.

The stronger native network regression certifies all six pre-fit original
curves against the surface built from common-station cubic fits at `1e-6`.
It also checks original domains, network contacts and source nonmutation.
This closes the original-image gap for that fixture and candidate without
assuming that the sampled curve-fit tolerance was a continuous proof.

The `network_parameterization` example now records a bound or an error for
every original curve. The retained
[initial schema-two observations](../tools/rhino_oracle/observations/network_parameterization_original_bounds_viboceros_20261016.json)
cover the same single `biquadratic_network` fixture and saved Rhino reference.
The earlier [schema-one observations](../tools/rhino_oracle/observations/network_parameterization_viboceros_20261016.json)
remain unchanged.

The subsequent [exact isocurve restriction](exact-isocurve-restriction.md)
reduces the original stored tensor before extraction and reuses bounded
one-dimensional data across knot crossings. Its
[current observations](../tools/rhino_oracle/observations/network_parameterization_isocurve_bounds_viboceros_20261016.json)
certify every original curve:

| Candidate | Complete original curves certified at `1e-6` |
| --- | ---: |
| Raw Gordon | 6 of 6 |
| Crossing-constrained arc-length refits | 6 of 6 |
| Global smooth station map | 6 of 6 |

The initial arc-length source-index-1 proof reached the exact work limit.
Its current bound is about `9.9851e-7`, with the same tolerance and work limit.
The smooth-candidate bounds remain at most about `9.95e-7`.

These certificates prove a complete original-curve-to-surface correspondence.
They do not prove injectivity, topology, symmetric boundary distance, surface
continuity or Rhino interior parity. The three returned-point comparisons
still fail the `1e-6` Rhino criterion, so NetworkSrf remains unregistered. The
surface fitting policy and broader Rhino comparisons remain unfinished.

Reproduce the observations with:

```sh
cargo run --release -p viboceros-smlib --features native \
  --example network_parameterization > /tmp/network-original-bounds.json
```

The [validation record](original-network-curve-admission-validation.json)
retains the test, sanitizer, compiler and diagnostic evidence.
