# Surface bounds with supplied UV mappings

[Surface correspondence](surface-correspondence.md) · [Sampled diagnostics](surface-deviation-samples.md)

`NurbsSurface::mapped_surface_deviation_bound(other, [u_pairs, v_pairs], limit)`
certifies two complete untrimmed surface images under supplied, independent
piecewise affine U/V mappings. Each list contains normalized `[self, other]`
parameters, strictly increasing in both columns from `[0,0]` to `[1,1]`.
Each axis accepts 2..4097 pairs. Natural domains are normalized exactly from
their stored endpoints. Reverse or swap a surface before supplying the maps.

A returned bound covers every parameter, including both limits of every
interior knot. Since both mappings cover their complete domains, the bound also
bounds symmetric Hausdorff distance. Curved, rational surfaces can therefore
receive complete image bounds despite different parameter speeds. The maps
need not be optimal; coupled U/V warps require another correspondence.

The implementation partitions the mappings at every original knot crossing
in either tensor, using rational arithmetic. Exact evaluations at mapped grid
points reject unsuitable proposals before expensive tensor products. Passing
that grid never authorizes a result: complete rational tensor restrictions,
Bernstein difference hulls and subdivision prove every intervening patch.
Outward interval tensor arithmetic handles ordinary cases; uncertain, zero or
subnormal bounds retain exact rational fallback under the same work budget.
Sampling or closest-point queries may propose mappings without weakening this
admission. Stored geometry is neither sampled into replacement geometry nor edited.

The existing surface certificate limits apply: degrees through 16, at most
4096 controls per source, independent nonzero common-sign weights, bounded
arithmetic size, work and subdivision. Unclamped and discontinuous tensors are
supported. The Cartesian map grid is charged before traversal. Exhaustion
returns `None`, as do outside or inconclusive correspondences. Invalid maps
and tolerances return errors. No trim, topology, normal or injectivity claim
is made. Existing surface fitting policies remain unchanged.

The local JSON operation `surface_mapped_deviation_bound` accepts `first`,
`second`, `correspondence: [u_pairs,v_pairs]` and optional `absolute_tolerance`
(default request tolerance). It returns `certified` and
`maximum_deviation_bound`; a declined bound is null, while exact agreement is zero.
Surface inputs accept the authored schema or the existing fitted/native readback.

Kernel regressions cover changed clocks in both axes on a curved rational
tensor, inverse maps, swapped axes, independent negative weight gauges,
subnormal displacement, inserted knots, discontinuities and unclamped inputs.
A polynomial excursion vanishes at all supplied grid points and is still
rejected by the complete interior proof.

The saved public Rhino fitting replay and its remaining inconclusive mappings
are recorded in [the validation record](../surface-mapped-validation.json).
Both builds replay all 15 calls: six parameter bounds, five additional mapped
image bounds, three inconclusive comparisons and one native null.
These source/output image bounds use each requested fitting tolerance; they
do not qualify automatic native knot selection or replace the `1e-9` output
comparison gate.
