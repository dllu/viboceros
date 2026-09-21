# Joining independently represented curved boundaries

[Kernel contract](brep-edge-joining.md) · [Command](commands/join.md) ·
[Complete replay report](join-curved-certificates-comparison.json)

Join now certifies complete curved boundaries with different degrees, knots and
positive rational weights, including reversed directions and unclamped domains.
The retained spatial curve, surfaces and UV geometry are not refitted by this
certificate. Existing endpoint rebuilding remains a separate step.

## Exact certificate

The existing common-basis and straight-locus tests remain fast paths. Otherwise,
each curve's full knots are normalized with exact rational arithmetic. On the
union of active knot spans, polar-form evaluation produces exact homogeneous
Bézier controls. This avoids treating rounded degree elevation or knot insertion
as an exact geometric identity.

For rational curves `A = Na/Wa` and `B = Nb/Wb`, their difference is
`(Na Wb - Nb Wa)/(Wa Wb)`. Bernstein multiplication gives an exact control net
with strictly positive denominator controls. Consequently, the largest norm of
its projected controls bounds the entire span. Exact squared-norm comparisons
decide whether a bound fits the requested distance; floating-point `hypot` is
only a candidate, verified against that exact predicate.

An interior control outside the distance ball is inconclusive, so the net may subdivide.
A difference-curve endpoint outside the distance disproves this correspondence.
Unresolved hulls after sixteen levels are not accepted. New algebra is limited
to degree 16 per input and charged to the shared 16-million-unit Join budget,
including explicit assembly; exhaustion fails without changing input geometry.
Common-basis and straight-locus certificates retain their existing degree scope.

This proves an affine-parameter correspondence, not the minimum Hausdorff distance.
It can therefore miss a valid geometric match with a different parameter speed,
or report a larger conservative uncertainty than a closest-point calculation.
Curved partial overlap and general reparameterization remain unfinished.

Tests include exact polynomial degree elevation; multiplying a homogeneous
quadratic by `3(1+t)`, producing different weights but identical geometry; unequal
knot partitions; signed, subnormal and extreme common gauges; overflowing squared
distances; unclamped and discontinuous span extraction checked against an
independent exact Cox–de Boor evaluator; and bounds checked against independent
rational samples. Analytic interior maxima test both acceptance after subdivision
and conservative refusal at the depth limit. Samples are tests, not acceptance
criteria. A separate test verifies that assembly cannot reset an exhausted budget.

## Actual Rhino command evidence

The [48 requests](../tools/rhino_oracle/fixtures/join_curved_certificates.json) and
[raw records](../tools/rhino_oracle/observations/join_curved_certificates.json)
come from two owned private-Xvfb sessions running Rhino 8.32.26160.13001.
Both `Join` and `JoinCopy`, preselection and command-first selection, are exercised
on per-source native 3DM exports. Every discovery case is retained.

Forty cases match every recorded field at `1e-10` absolute / `1e-12` relative
epsilon. They cover degree elevation, nonproportional rational representations of
the same curve, inserted knots, reversed/shifted parameter domains, canonical
unclamped input, and both accepted and rejected interior gaps. Full geometry,
topology, uncertainty, surfaces, UV trims, integrals and document state are compared.

Eight records remain explicit discrepancies:

- Four unclamped inputs initially used full exterior knots `[-2,-1,0,1,2,3]`.
  The 3DM/OpenNURBS representation omits the two redundant exterior slots;
  Rhino's full-knot record reconstructs `[-1,-1,0,1,2,2]`. JoinCopy originals
  already show this difference: it is input interchange, not Join refitting.
  All other fields match. A fresh four-case batch using that canonical full
  vector matches completely. The initial records are preserved, not rewritten.
- Four nearby rational-weight cases join identically but native records an
  edge uncertainty of approximately `0.0004504074551483943`, versus Rhino's
  `0.0003749812509381556`. The native value is a certified bound for the affine
  parameter correspondence. It is not reduced to Rhino's value without a proof
  for a different correspondence. At normalized parameter `1/4`, exact arithmetic
  on the input already gives distance approximately `0.00045020279033980984`,
  greater than Rhino's recorded uncertainty. Every other field matches.

Across all eleven surface-command fixtures, **511 of 642** records fully match,
32 retain native execution failures and 99 have other differences. All earlier
594 rows retain exactly their prior match flags, numeric residuals and errors.
The report records fixture, observation, kernel-source, executable, raw-batch and
shared-artifact hashes. Harness timings do not establish kernel speed parity.

```sh
cargo test --release -p viboceros-geometry join_edges
cargo test --release -p viboceros-oracle curved_certificates
python3 -m tools.rhino_oracle replay tools/rhino_oracle/fixtures/join_curved_certificates.json --observations tools/rhino_oracle/observations/join_curved_certificates.json --absolute-epsilon 1e-10 --relative-epsilon 1e-12
```

The replay intentionally exits 1 for the eight archived differences. These tests
do not resolve the separately recorded endpoint-clustering and face-sense policies.

Verification passed 2,828 Rust tests (20 ignored), 232 Python tests, formatting,
and Clippy/Rustdoc with warnings denied.
