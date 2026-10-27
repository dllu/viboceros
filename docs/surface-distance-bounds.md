# Certified surface distances

[Closest-point search](surface-closest-point.md) · [Validation](surface-distance-validation.json)

`NurbsSurface::minimum_distance_to_point` returns a certified distance bracket
for a finite point and the complete active tensor surface. The seeded form
accepts native UV parameters from an independent solver, including a native
kernel. Seeds are proposals; exact evaluation on the original source supplies
the witness.

The minimum lies between `upper_distance - maximum_error` and `upper_distance`.
`lower_distance_bound()` rounds the lower endpoint downward. Returned UV values
use the stored domains; the displayed witness point is rounded. The distance
bracket applies to the exact surface value, which can differ from that rounded
point. This bounds distance error, not UV error or uniqueness.

The query reuses the existing exact tensor extraction, Bernstein subdivision,
Euclidean norm bounds and arithmetic budgets. Positive homogeneous patch hulls,
boxes and separating planes prove global lower bounds. A local box-center axis
also bounds basins whose normals differ from the best witness. Every active
patch participates; numerical samples alone never certify a minimum.

Supported inputs have degrees at most 16, at most 4096 controls, common-sign
weights and interior knot multiplicities no greater than the degree. This
covers the untrimmed tensor image. Trimmed-face membership, mixed signs and
discontinuous tensor images are outside this query. Work, arithmetic size,
subdivision depth48 and 65536 pending patches are bounded. `None` means
unsupported or unproved, including insufficient native UV/value precision.

The shared stopping predicate now accounts for witness-distance rounding after
pruning. A curve regression previously reported a `sqrt(2)` distance bound with
error `5e-324`; that unattainable precision now returns `None`.

Tests use analytic plane, boundary, rational-cylinder, nonconvex saddle,
unclamped-domain and extreme-scale minima, including deliberately poor seeds.
The spatial Sweep1 integration check proves source-witness membership within
`1e-12`, then bounds its distance to the complete other surface within `1e-9`.
For the captured reversed spatial case and a model constructed at `1e-7`, this
proves an image gap greater than `2.376e-6`. Sweep1 compatibility at `1e-6`
therefore remains unfinished. The test records classification instead of
requiring the geometry defect to persist; one close witness would not prove
complete surface agreement.

```sh
cargo test -p viboceros-geometry --test surface_minimum --test curve_minimum_precision --test sweep_surface_distance --frozen
```
