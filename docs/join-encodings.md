# Join curve encodings

[Join commands](commands/join.md) · [Cycle behavior](join-cycles.md)

Join treats representation as well as geometric shape as significant. The kernel
recognizes lines, polylines, degree-one NURBS with nonzero spans, and polycurves
composed entirely of those encodings. Polycurve vertices are mapped through each
leaf's local-to-parent parameter map before assembly. This avoids an unnecessary
PolyCurve wrapper and preserves intermediate vertex parameters in individual-pick
joins. Wholly linear batches instead use chord lengths.

Higher-degree NURBS remain NURBS, including collinear Bézier control polygons.
No approximate line fitting or degree reduction is used. Degree-one rational
curves can become polylines: their geometric locus is retained, but unequal
weights' nonlinear parameter speed is not. This is the measured Join conversion
policy, not a parameter-preserving general-purpose NURBS conversion.

## Mixed polycurves

Adjacent recognized linear leaves coalesce using their parent interval widths.
The new polyline's local domain retains its first leaf's origin. Batch results synchronize local
domains with parent intervals. Individual-pick results that consolidate linear
runs rebuild the mixed composite from its local leaves, starting at the first
leaf's domain origin; they do not retain the temporary seed offset or synchronize
all local domains. Unmerged open results follow the ordinary seed-interval path.
Both local and parent domains matter when evaluating the result.

When a closing curve can merge with a mixed polycurve's linear tail behind a
nonlinear head, it appends at that tail. The decision inspects the actual first
and last leaves, not the enclosing PolyCurve type.
An analytic arc closing an accumulated polyline appends, whereas one closing a
single line prepends. A general NURBS closer follows its own path. Assembly
records the earliest source's parameter in the rebuilt result; JoinCopy uses
that mapping instead of assuming the source's old domain origin still applies.

For early closed JoinCopy, a standalone two-control-point NURBS has an additional
parameterization test: compare its native midpoint with its linear midpoint
using absolute document tolerance. Exceeding that tolerance retains the closing
seam; constant weight scaling and sufficiently small deviations restore the seed
seam. Multi-span NURBS and polycurve seeds follow their own representation paths.
These are command policies; all copied source geometry, weights, knots,
attributes, groups, and identity remain unchanged.

## Evidence

The [352-case fixture](../tools/rhino_oracle/fixtures/join_encodings.json),
[raw Rhino observations](../tools/rhino_oracle/observations/join_encodings.json),
and [comparison report](join-encodings-comparison.json) cover rational degree-one
curves, weighted quadratics, degree-2/3/5 Bézier controls, piecewise NURBS,
polycurves, nesting, reversed selection, open/closed chains, and both commands
and selection modes. Raw local domains, parent intervals, samples, definitions,
identity, creation order, selection, and metadata are compared without output
normalization, with absolute epsilon `1e-10` and relative epsilon `1e-12`.

The separate [192-case seam fixture](../tools/rhino_oracle/fixtures/join_weight_seams.json),
[raw observations](../tools/rhino_oracle/observations/join_weight_seams.json), and
[comparison report](join-weight-seams-comparison.json) distinguish midpoint deviation from maximum deviation and
exact weight equality, using model scales 2 and 2000, document tolerances from
`1e-6` to `0.5`, homogeneous weight scaling, reversed weight ratios, and single-
versus multi-span NURBS. One initial Wine-hosted Rhino run exited with an internal
CLR error during recording; the isolated case and remaining smaller jobs all
completed. No output from that failed run is included as a reference record.

Independent tests check mapped vertices, parameter speeds of polynomial linear
leaves, preservation of higher degrees, terminal-leaf closure, constant versus
unequal homogeneous weights, exact source retention, and undo/redo. The preceding
605 Join observations are also replayed. No performance parity or general
curve-network compatibility is inferred from these bounded cases.

```sh
tools/rhino_oracle/run_headless.sh compare tools/rhino_oracle/fixtures/join_encodings.json --timeout 540 --relative-epsilon 1e-12
tools/rhino_oracle/run_headless.sh compare tools/rhino_oracle/fixtures/join_weight_seams.json --timeout 420 --relative-epsilon 1e-12
cargo test --release -p viboceros-oracle join_command::tests
```
