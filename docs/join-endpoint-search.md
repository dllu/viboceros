# Join endpoint search

[Curve joining](curve-editing.md) · [Join commands](commands/join.md)

Curve joining must find nearby endpoints independently of unrelated distant
geometry. The former spatial grid could violate that requirement: with its
origin at `-2^53`, subtracting the origin from endpoint coordinates `1` and `2`
rounds to `2^53` and `2^53 + 2`. At tolerance `1`, the endpoints landed two cells
apart and were never compared. Changing input order could change connectivity.

## Conservative search

The kernel's separate `curve_join/search` module builds a balanced bounding-box
tree for positive tolerances. Each node stores exact minima and maxima of the
original finite endpoint coordinates. Median partitioning along the widest axis
keeps recursion logarithmic even for coincident points. Paired traversal covers
every unordered endpoint pair once, unless its boxes can be rejected.

A box pair is rejected only when a coordinate interval gap exceeds tolerance.
The subtraction is monotone and uses the same binary64 coordinates as the final
distance test: its positive result cannot exceed that coordinate difference for
any contained pair. Overflow also safely rejects a gap larger than a finite
tolerance. There is no subtraction of a shared origin, division by a tiny
tolerance, or rounding of an expanded world-space search box.

This establishes completeness relative to the existing rounded-distance test;
it does not replace that test with an exact Euclidean predicate. Distance/tangent
ranking, source-order tie breaks, direction filtering, chain assembly, endpoint
movement, and command transactions are unchanged. Zero tolerance retains exact
coordinate hashing, including normalization of signed zero.

The existing ceilings remain 100,000 input curves, one million accepted
candidates, and 16 million endpoint comparisons. A separate 16-million limit
also bounds tree-node pair visits. Dense or difficult searches fail explicitly,
without partial document edits. The tree uses linear storage; worst-case pair
search remains quadratic, bounded by these limits.

## Evidence

The [48-case fixture](../tools/rhino_oracle/fixtures/join_search.json) and
[unmodified Rhino record values](../tools/rhino_oracle/observations/join_search.json)
cover Join/JoinCopy, each coordinate axis, reflected coordinates, scales `1` and
`0.001`, and distant-first versus distant-last source order. The preceding
implementation failed 12 cases. All 48 now match Rhino 8.32.26160.13001, measured
on an owned private Xvfb display, with absolute epsilon `1e-10`, no relative
epsilon, and maximum numeric error below `1.8e-15`. The regression test also
requires the entire untouched outlier record to agree exactly, so a large
coordinate-relative epsilon cannot conceal movement.

Independent tests compare complete endpoint pair sets and distances with a
literal all-pairs reference, with direction filtering both enabled and disabled.
They cover signed zero, subnormals, tolerance boundaries and adjacent floats,
all three axes, 256 deterministic multiscale point clouds, and overflowing
coordinate differences. Larger chain, planar, translated-planar, and 3D sets
exercise 100,000 endpoints per set. Dense-input tests check both candidate and
comparison ceilings; a public joining regression checks midpoint movement,
domains, source membership, and input preservation.

Seven-run release medians for 50,000 endpoints on this host:

| Workload | Previous search | Bounding-box tree |
| --- | ---: | ---: |
| Chain | 25.12 ms | 12.76 ms |
| 3D grid | 24.59 ms | 18.97 ms |
| Translated planar grid | 24.27 ms | 15.02 ms |
| Planar grid, tolerance `1e-300` | 36.47 ms | 14.98 ms |

These measurements include search construction and candidate generation, not
curve evaluation, sorting, assembly, UI, or Rhino. They are not a performance
parity claim or a timing assertion in CI. See the [comparison report](join-search-comparison.json)
for hashes, raw timings, and regression results.

```sh
cargo test --release -p viboceros-geometry curve_join::
cargo test --release -p viboceros-oracle join_command::tests
cargo test --release -p viboceros-geometry endpoint_search_benchmark -- --ignored --nocapture
tools/rhino_oracle/run_headless.sh compare tools/rhino_oracle/fixtures/join_search.json --timeout 300 --relative-epsilon 0
```
