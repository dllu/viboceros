# One-pass command-first curve joining

[Join commands](commands/join.md) · [Curve joining](curve-editing.md)

Individual command-first picks use source order, not the batch API's global
nearest-pair order. Only the first open source grows into a chain. Earlier
rejected sources are not reconsidered after a later extension, and closing the
chain ends the operation. The kernel's `curve_join/seeded` module implements
that connectivity policy directly.

For each later open source, it compares both of its endpoints with the current
chain's two free ends. Up to four candidates use the same distance, outward
tangent, direction filter, and endpoint-index rank as batch joining. The chosen
attachment replaces one free end; one additional endpoint comparison tests
closure. The existing representation-dependent prepend/append rules, seam
mapping, parameter assembly, and command document edits remain unchanged.

This is equivalent to the former graph traversal's source-priority rule: the
next accepted source is the first later source with an eligible connection to
either current end. Already considered sources are ineligible under the one-pass
pick policy, even if a later extension could connect to them geometrically.
The implementation therefore needs neither all-pairs candidate generation nor
an adjacency list, assigned-source table, or repeated search through unused
branches. Connectivity requires at most five endpoint comparisons per later
open source and linear partner storage. Endpoint/tangent evaluation, curve
representation inspection, assembly, and document work are separate costs.

Batch joining still uses the [bounded spatial search](join-endpoint-search.md).
The 100,000-input limit applies to both styles. Batch candidate/comparison budgets
do not apply to seeded connectivity, whose work is bounded directly by its
single source pass. Dense contacts among unused sources cannot exhaust a seeded
join's global candidate budget because no such graph is constructed.

## Validation and performance

A regression first reproduced the old failure: 40,000 connected line segments
followed by 600 branches at the seed's original start exhausted the 16-million
seeded scan budget. The new path joins the full chain and the first branch,
retains all other sources, and preserves the seed's native parameter stations.
A second regression closes a three-curve chain before 2,000 coincident unrelated
sources; the unused dense graph no longer prevents that closure.

An independent exhaustive graph reference checks all endpoint partners and the
closing edge for 1,024 deterministic mixed-representation input sequences,
four tolerances, and both direction policies: 8,192 comparisons. Cases include
lines, arcs, polylines, NURBS, mixed polycurves, stationary endpoint derivatives,
closed inputs, reversed directions, and empty input. The reference retains the
previous traversal in test-only code and independently enumerates/ranks pairs;
it does not call the production search or one-pass matcher.

Seven-run release medians for complete native `join_curves` calls on this host,
excluding fixture construction:

| Chain segments / additional branches | Previous graph traversal | One pass |
| --- | ---: | ---: |
| 1,000 / 200 | 2.13 ms | 0.168 ms |
| 10,000 / 200 | 11.77 ms | 2.013 ms |
| 40,000 / 600 | Scan-budget failure | 8.651 ms |

This measures native curve joining, including endpoint preparation, assembly,
and result construction, not Rhino or the document/UI path. The interactive
completion hook still restages the selected curves after each pick; these
measurements do not establish linear total latency for a sequence of UI picks.
There is no timing threshold in CI or general Rhino performance-parity claim.

The [comparison report](seeded-join-comparison.json) records source and release
binary hashes, raw timing samples, all 1,197 saved Join cases, and a fresh
140-case Rhino workflow check on an owned private Xvfb display. Existing raw
geometry, parameter domains, metadata, selection, and identity comparisons are
retained without output normalization.

```sh
cargo test --release -p viboceros-geometry curve_join::
cargo test --release -p viboceros-oracle join_command::tests
cargo test --release -p viboceros-geometry seeded_join_benchmark -- --ignored --nocapture
```
