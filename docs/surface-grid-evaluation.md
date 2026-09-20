# Batched surface-grid evaluation

[Closest points](surface-closest-point.md) · [NURBS numerical policy](nurbs-numerics.md)

The internal `NurbsSurface::for_each_grid_point` evaluates a tensor grid in
V-major/U-minor order and reports each point or error to a callback. Closest-point
seeding uses it without changing its parameter samples, candidate order, sixteen
refinement starts, four boundary searches, or subsequent refinement policy.

## Reused work, unchanged arithmetic order

The scalar evaluator builds a centered, weight-normalized active control net,
interpolates its rows in U, then interpolates the resulting column in V. Within
one V knot span, each U sample's row interpolations are independent of V. The
batch evaluator retains those columns and their original local origins. Active
controls are shared across consecutive U samples in the same U span.

The shared de Boor recurrence now accepts caller-owned scratch storage. Existing
owning callers keep the same recurrence; the grid path reuses its U and V buffers.
There is no new basis formula, reordered contraction, degree cap, approximate
surface recognition, or changed knot-side policy. Interpolated controls and
world-origin restoration use the scalar evaluator's helper.

Only the current V span's columns are retained. Unsorted parameters, duplicates,
and revisiting earlier spans are valid; changing V spans rebuilds the cache.
Additional storage is proportional to the U sample count times `degree_v + 1`,
plus one active control patch and scratch buffers. For exact columns this counts
rational values, whose integer sizes are input-dependent. Empty grids do no work. All
caches belong to the query, not the surface.

A failed cached projection uses scalar evaluation for that cell. This preserves
validation precedence, genuine poles, and exact recovery when intermediate
coordinates overflow but the world-space point is finite.
Prepared nets with mixed-sign weights or subnormal/erased weights or weighted
coordinates instead cache exact homogeneous U contractions from the original
controls. V interpolation and the final projection stay exact. An intermediate
row with zero weight is valid; only the final denominator is tested. Both paths
reuse only the current V span and preserve scalar output bits and errors.
The [pole audit](surface-pole-recovery.md) also tests exact poles whose rounded
denominator is nonzero; waiting for cached projection to fail cannot detect them.
Its [dyadic specialization](exact-dyadic-evaluation.md) accelerates eligible exact
recurrences without approximating them.

## Exact-control recovery

The audit exposed a scalar bug: a nonzero weight near `MIN_POSITIVE` can normalize
to zero beside a weight near `MAX`, falsely reporting a pole at an exactly
interpolated control. Point-only evaluation now recovers that stored control
after a failed homogeneous projection. The knot multiplicities and selected
spans must identify the control exactly; model tolerance is not involved.

The subsequent [range-loss audit](surface-rational-range.md) also found finite
edges and silently incorrect interior results, and added a guarded exact-rational
path for points and derivatives. Genuine poles and unrepresentable requested
derivatives remain errors. Both scalar and grid tests check every corner, both
common weight signs, and exact signed-zero coordinates. This does not solve every
possible floating-point error in unflagged nets.

## Validation

The deterministic parity test checks 17,820 cells on 180 rational surfaces with
U degrees 1–5, V degrees 1–4, multiple spans, shuffled/duplicate samples, signed
weights, extreme coordinate/weight scales, and tiny weights. Successful points
match scalar binary64 bits; failures match scalar error records. Further tests
cover translated and reversed surfaces, UV swaps, subnormal/overflow-width
domains, discontinuous knots, exact interpolation, signed overflow fallback,
invalid parameters, empty grids, and unchanged source geometry.

An independent dyadic polynomial formula checks `S=(u,v,u²+uv+2v²)`. Exact-control
recovery is checked against the stored controls, not merely another evaluator.
The existing closest-point Rhino replays retain their separate `1e-8` and `1e-7`
bounds. Bitwise parity is regression evidence, not proof that every scalar
evaluation is mathematically accurate.

```sh
cargo test -p viboceros-geometry grid_points
cargo test -p viboceros-geometry grid_and_scalar_recover_exact_corners
cargo test -p viboceros-oracle closest_surface
cargo test -p viboceros-geometry --release benchmark_surface_grid_evaluation -- --ignored --nocapture
```

The manual microbenchmark compares scalar and batched traversal of a `33 × 33`
grid on one-span and two-spans-per-axis rational quadratic patches. It runs three
rounds of 200 grids per method and checks identical accumulated coordinates;
there is no machine-dependent timing assertion in the normal suite.

## Measurement

The [measurement record](surface-grid-performance.json) contains raw round times,
request reconstruction, and correctness checks. On the aarch64 release build,
grid traversal improved by **9.92×** for one span and **8.82×** for two spans per
axis, with identical coordinate sums. Median time per evaluated point fell from
approximately `215–217 ns` to `22–25 ns`.

The complete closest-point benchmark compares baseline `5eb3e26` against the new
release, sequentially after builds/tests finished, using three rounds of 200
iterations. Times are median microseconds per query:

| Fixture case | Before | After | Speedup |
| --- | ---: | ---: | ---: |
| Quarter cylinder, unit domains | 238.056 | 120.290 | 1.98× |
| Quarter cylinder, anisotropic domains | 252.001 | 128.008 | 1.97× |
| Paraboloid, unit domains | 442.221 | 269.971 | 1.64× |
| Paraboloid, anisotropic domains | 433.712 | 285.955 | 1.52× |
| Warped bilinear saddle | 190.162 | 116.258 | 1.64× |

All 114 query results across 27 operations are bit-for-bit unchanged, including
native/normalized parameters, evaluated points, and distances. The four affine
controls remain approximately `0.94–0.99 µs` per query (within 1% of baseline).
The exact-control recovery is a separate, intentionally changed failure case,
not exercised by these ordinary-weight benchmark fixtures.

This run did not launch Rhino again: saved-reference replays and bitwise native
before/after comparisons protect the existing oracle evidence. These measurements
do not establish native Rhino speed parity or a universal global-minimum guarantee.
