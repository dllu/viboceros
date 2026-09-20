# Surface closest-point query preparation

[Closest-point search](surface-closest-point.md) · [Exact pole handling](surface-pole-recovery.md)

`nurbs_surface/evaluate/query.rs` owns the last active span's prepared controls
for one closest-point search. Refinement, line-search trials, and local polishing
share it. Grid seeding retains its separate tensor-contraction cache.

## Invariants

- The cache borrows its source surface and retains only one `(span_u, span_v)`
  pair. Crossing either span replaces its state; no global cache or surface
  mutation is involved.
- Every query validates U, then V, using the same strict, right-sided span
  selection as the public evaluator. Invalid parameters cannot reuse a valid
  station's result: station results are not cached.
- Ordinary spans retain prepared floating-point controls. A failed floating
  evaluation still takes the existing exact fallback for that request; it does
  not permanently change later stations' dispatch or rounded bits.
- Guarded spans retain exact original homogeneous controls. First and second
  homogeneous derivative nets are constructed lazily. They depend on the knots,
  degrees, spans, and original coefficients, not the current parameter. Each
  evaluation still performs exact tensor contraction, exact denominator-zero
  checking, the quotient rule, and final requested-component rounding.
- A higher-order overflow does not invalidate a later point request. Public
  stateless exact evaluation uses the same prepared engine with a fresh lifetime.
- A normalized-domain recovery surface owns a separate query cache; its
  coefficients cannot be reused on the original parameter frame.

## Terminal hits

Coordinate equality between an evaluated surface point and the target attains
the global lower bound zero for the evaluated-point objective. The first native
corner, best grid seed, improving boundary/refinement candidates, and accepted
refinement steps may terminate on this condition. A boundary hit is rechecked
on the surface, since an extracted isocurve can round differently.

The first corner is also the first grid station and retains the existing
equal-distance tie preference. A pole there is still rejected before any
constant-control shortcut. No modelling epsilon or underflowed distance is used
as evidence of coordinate equality. This does not certify a general rational
surface's global closest point, or remove error in evaluated positions.

## Regression coverage

Eight new tests cover exact-net lazy construction/storage reuse; ordinary/exact
cache transitions across U and V full-order knots; repeated pole, invalid, and
finite stations; late floating-point failure and derivative overflow; exact
corner/interior/singular hits; a pole at a constant patch's native corner; and
nonzero model-tolerance/subnormal separations. Cached results are compared to
stateless results by component bits and error variants. The 150 independent
Python Fraction/Bernstein records also exercise the prepared exact engine in
the order `2, 0, 1, 2`, including failures followed by lower-order requests.

## Reproducing performance measurements

Preserve the previous release oracle before rebuilding, then run:

```sh
python3 tools/numerics/benchmark_surface_query.py /path/to/oracle-before target/release/viboceros-oracle --cpu 5 --baseline-commit 88a0deaa8032301573c9690085856e32d6c0ab0f
```

The script compares all returned values, including signed-zero bits. Ordinary
cases use the two existing closest-point fixtures, 200 iterations, and five
rounds per operation. The two signed-weight degree-(5,2) outputs from
`sweep1_weights.json` use three alternating-order whole-process timing pairs
per workload: first-corner queries, three explicit off-surface targets, and all
135 original queries per operation. Sweep's `elapsed_ns` times construction,
not closest-point queries, so it is deliberately not used for those workloads.

CPU affinity does not isolate a core or fix clock frequency. These are bounded
native before/after measurements, not comparisons with native Rhino kernel speed.

## Measured results

The [release measurement record](surface-query-cache-performance.json) compares
`88a0dea` with this cache/terminal-hit implementation on aarch64, pinned to CPU 5.
No builds or Rhino launches were initiated during measurement; brief Python/file
diagnostics and uncontrolled host activity remain caveats.

| Signed-weight workload | Before | After | Speedup |
| --- | ---: | ---: | ---: |
| Two endpoint queries | 291.91 ms | 60.75 ms | 4.80× |
| Six off-surface queries | 1.114 s | 1.009 s | 1.10× |
| All 270 fixture queries | 49.126 s | 40.363 s | 1.22× |

All returned values are bit-identical across the six runs of each workload.
These whole-process measurements include sweep construction, definition
extraction, and serialization; endpoint time is not the cost of the hit check
alone. The full workload remains expensive. Caching derivative nets does not
remove exact tensor-contraction or rational quotient-rule work.

The ordinary nine-case benchmark also retains all 190 query results bit for bit.
Curved cases take 13.7–29.2% less time; the unchanged affine algorithm measures
3.1–3.9% slower (roughly 30–40 ns/query). Those small absolute regressions are
reported rather than hidden by a single aggregate speedup.

## Licensed Rhino check

The [signed-surface fixture](../tools/rhino_oracle/fixtures/surface-closest-signed.json)
stores the independently constructed retained-sweep surface directly. Its single
negative Bernstein coefficient does not indicate a pole: the earlier
[exact subdivision proof](surface-pole-recovery-signed-cost.json) gives a positive
denominator lower bound. Five queries include two endpoint hits and three
off-surface targets.

A fresh Rhino **8.32.26160.13001** run on an owned private Xvfb/i3 display agrees
on parameters, points, and distances within **3.155 × 10⁻¹¹**. The
[saved comparison](surface-closest-signed-comparison.json) and Rust replay use
`1e-9`; both [native](surface-closest-signed-native-reference.json) and
[Rhino](surface-closest-signed-rhino-reference.json) responses are retained.
The raw timing remains substantially slower on the guarded native path; this
change does not establish performance parity with Rhino.

```sh
tools/rhino_oracle/run_headless.sh compare tools/rhino_oracle/fixtures/surface-closest-signed.json --absolute-epsilon 1e-9 --relative-epsilon 1e-9 --timeout 240
```

Final validation: the unfiltered release workspace suite passes **2,550 tests**
with **18 existing opt-in tests ignored**, including the complete sweep fixture
integration and the new signed-surface replay. All **195 Python oracle tests**,
strict all-target Clippy, warnings-denied rustdoc, formatting/diff checks, and
byte-identical regeneration of the 150-case rational reference also pass.
