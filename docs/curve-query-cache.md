# Curve query coefficient cache

[Closest-point search](curve-closest-point.md) · [Surface query cache](surface-query-cache.md)

The curve closest-point correctness audit increased ordinary query costs. An
allocation probe at `8be40e1` found **478 allocations** for the quarter-circle
query, **575** for the nonuniform multispan curve, and **182** for the endpoint
line. A public second-derivative evaluation made four or five allocations per
station. Hardware sampling was unavailable because the host
restricts performance counters; no host security settings were changed.

## Ownership and numerical policy

`nurbs/evaluate/query` owns the most recently used knot span. A closest-point
search has separate query state for the original curve and the translated
refinement curve, retaining the original-frame ranking rule. Span changes
replace the prepared state; no global cache or mutable source geometry is used.

Ordinary spans retain locally centered homogeneous controls. The shared
`evaluate/jet` module constructs first and second derivative nets only when
requested, and reuses a floating-point scratch buffer for de Boor evaluation.
Each station copies the immutable coefficients into that scratch; an earlier
evaluated station is never reused. Failed derivative-net construction is cached
as an error for that order, without making point or first-derivative queries
depend on higher-order representability.

Scratch capacity is reserved for exactly the active control count, and the
candidate vector reserves the known seed count. These capacities avoid repeated
growth without adding or removing any seed, candidate, or refinement step.

Public first/second-derivative calls use the same quotient rule and prepared-net
helper, but with fresh state. The point-only public path retains its lightweight
endpoint and de Boor implementation. Rational degree-one curves still have
nonzero Euclidean second derivatives when their weights differ.

Mixed-sign or range-sensitive spans retain exact rational homogeneous controls
and lazily built exact derivative nets. The exact denominator is evaluated and
checked at **every** station before those derivative nets are used. A prior
finite point, constant control hull, or cached derivative cannot certify that a
later point is not a pole. Cached nets depend on the immutable knots and selected
span, not on the parameter used when they were first requested.

Parameter validation and left/right span selection precede cache access. A valid
fully interpolated endpoint retains the public point evaluator's exact control
point shortcut. Derivative queries still follow the complete jet policy.

Late floating-point failure follows the public recovery sequence: retry in the
unshifted frame when appropriate, then use original exact inputs. It does not
permanently promote the span to exact evaluation or change later point rounding.
The cache does not relax candidate ordering, stationarity tolerances, pole checks,
iteration limits, or model-space comparisons.

## Validation scope

Four stateful tests exercise discontinuous/full-order knots, both sides of a
knot, repeated returns to earlier spans, mixed-sign poles and their adjacent
floats, invalid parameters, subnormal domains and coordinates, overflowing
derivatives, wide control hulls, and weight ratios spanning the binary64 range.
They compare both bits and errors with uncached public evaluation and verify
that the source curve remains unchanged.

The existing **127-case independent Fraction basis reference** now also checks
cached points, first derivatives, and second derivatives. It includes exact
poles and independently unrepresentable derivatives. This supplements cache
parity checks with an independent numerical reference; it does not replace the
existing analytic jet and closest-point regressions.

The five robust closest-point probes and all 23 rational-jet fixture operations
are bit-identical to `8be40e1`. Previously documented Rhino discrepancies remain
explicit, including the translated semicircle's endpoint choice. This purely
coefficient-reuse change does not establish any new Rhino compatibility claim.
No new Rhino session was run for this cache-only change.

The unfiltered release workspace suite passes **2,569 Rust tests**, with **18
existing opt-in tests ignored**. All **196 Python oracle tests**, strict
all-target Clippy, warnings-denied rustdoc, and formatting/diff checks pass.
Regeneration of the 127-case curve and 150-case surface rational references is
byte-identical. Existing geometric assertions and tolerances were not weakened.

## Reproduction

The [final release record](curve-query-cache-performance.json) compares the
same three ordinary curve fixtures on aarch64, pinned to CPU 5. Allocation
counts come from the counting example; times come from the separate oracle:

| Query | Allocations before → after | Requested bytes before → after | Oracle time before → after |
| --- | ---: | ---: | ---: |
| Rational quarter circle | 478 → 14 | 53,936 → 3,392 | 29.14 → 24.78 µs |
| Nonuniform rational multispan | 575 → 59 | 76,664 → 9,784 | 31.98 → 24.98 µs |
| Endpoint line | 182 → 13 | 18,976 → 3,120 | 9.13 → 8.96 µs |

The curved probes take **15% and 22% less time** than the previous checkpoint.
The endpoint line is essentially unchanged. This recovers part, not all, of the
earlier correctness-related regression. Uncached public jets still allocate
three or four times; this is not a claim that all individual evaluations are
faster. Five timing batches of 500 iterations are recorded for each curve case.

Downstream curved surface queries take 1.6–4.5% less time in this run. The
unchanged affine path measures about 2% slower, illustrating the host noise
floor. All **190 ordinary surface results** remain bit-identical. Signed-weight
whole-process pairs measure 60.81→62.16 ms for endpoints, 1.009→1.012 s for
off-surface queries, and 39.34→39.26 s for all 270 queries; a single pair does not
establish a speed change. Every signed result, including all 270 sample points
and the complete surface definitions, remains bit-identical.

No builds or Rhino launches ran during measurement. Core isolation, CPU
frequency, and unrelated host activity were not controlled.

The allocation diagnostic is deliberately not a timing or allocation-count gate:

```sh
cargo build -p viboceros-geometry --example profile_curve_queries --release
taskset -c 5 target/release/examples/profile_curve_queries
```

It counts Rust allocator calls (including reallocations) and total requested
bytes, not peak/live memory or external C allocations. It is single-threaded;
construction, warm-up, and printing are outside each measured batch. The counting
allocator adds instrumentation overhead, so use the separate release oracle
benchmark for timing comparisons. The same diagnostic source was used for both
revisions; it was added during this audit.

The ordinary curve benchmark uses `nurbs_curve_closest.json`, five batches of
500 iterations with unique operation IDs. Surface timings and value auditing use:

```sh
python3 tools/numerics/benchmark_surface_query.py /path/to/oracle-before target/release/viboceros-oracle --cpu 5 --rounds 1 --baseline-commit 8be40e156ce346148aa951f42b93e5fb7fa8c9ea --artifacts /path/to/new-diagnostic-directory
```
