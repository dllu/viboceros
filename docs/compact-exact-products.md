# Compact exact products

[Numerical robustness](numerical-robustness.md) · [Finite sums](numerical-sums.md)

The exact dot-product fallback no longer always processes the full binary64
exponent range. It selects a four-word integer accumulator when all products
and carries fit, while retaining the 66-word pair-product and 99-word
scaled-product fallback paths. This targets the exact projections needed during
[curve closest-point refinement](curve-closest-point.md), without changing the
ordinary compensated arithmetic or the search algorithm.

## A conservative exponent window

`binary_accumulator/products` examines nonzero input pairs without multiplying
them. A binary64 significand has at most 53 bits, so pair products need at most
106 bits and triple products at most 159. Subnormals may use fewer bits; the
selector deliberately keeps the conservative bound.

Let `L` be the smallest product shift, `H` the largest exclusive upper bound on
its occupied bits, and `N` the number of terms. The window starts at
`B = 64 floor(L/64)`. It is accepted only if
`H + ceil(log2(N)) - B <= 256`. Positive and negative magnitudes are accumulated
separately, so this reserves enough room even if every term has the same sign.
The decision never relies on estimated cancellation. The bounds can only expand
as terms are examined, allowing an immediate full-range decision when they do
not fit. Zero products do not constrain the window; an empty or all-zero input
uses a zero accumulator with base zero.

The chosen accumulator receives products directly. There is no intermediate
array of decomposed products or replay after partial accumulation. Triple
products are split into two exact `u128` pieces with the same sign; their sum is
the original product, so the same carry bound applies to every partial sum.

The window's bit-zero exponent is `B-2148` for pair products, `B-2149` for a dot
product divided by two, and `B-3222` for scaled products. These are integer
exponent offsets, not floating-point coordinate rescalings. Every significand
bit is retained before one final rounding.

## Rounding and predicates

The shared rounder accepts the exact exponent origin. It retains at most 53
significant bits, never a quantum finer than `2^-1074`, then applies guard/sticky
bits and nearest-even rounding. A short accumulator may start above the
subnormal quantum; its result is normalized by exact bit shifts, without a
second rounding. A significand just beyond the compact buffer at the underflow
boundary is handled explicitly.

Exact cancellation returns positive zero. A negative nonzero exact sum that
rounds below the subnormal range retains negative zero. True rounded overflow
remains infinity; public APIs that require finite results still reject it.
`FiniteSum` shares the rounder but keeps its existing 34-word streaming storage.

Distance ordering keeps direct, full-range integer accumulation. Identical
points return an exact tie immediately, and equal coordinate contributions are
skipped because they cancel algebraically. No rounded-distance shortcut or
tolerance is used. A materialized-product/window prototype was rejected for
this predicate: it slowed the tested near-tie and wide-range cases. The compact
window is used where the measurements support it, not indiscriminately.

## Independent checks

New tests include:

* Exact window-fit boundaries for pair and triple products, carry headroom,
  zero operands, common exponent changes, and widely separated exponents.
* 1,920 comparisons of compact/full accumulator rounding with independently
  constructed arbitrary-precision integer ratios, including dense/sparse
  magnitudes and cancellation.
* Explicit halfway, sticky-bit, normal/subnormal, signed-zero, and overflow
  boundaries, including an output significand beyond the compact buffer.
* 768 clustered input sets, each checked for ordinary, halved, and scaled exact
  dots against arbitrary-precision rational arithmetic.
* 512 finite-binary64 point triples checked against independent rational squared
  distances, supplementing existing integer comparisons across binary scales.

The existing bit-by-bit shifted-product adder tests, long borrow chains,
hardware addition/multiply/FMA comparisons, and 256-case Python `Fraction`
scaled-dot reference remain in force. These checks validate the scalar
arithmetic; they are not a claim of certified global NURBS optimization or full
Rhino compatibility.

## Measured results

The [raw timing and value-audit report](compact-exact-products-performance.json)
compares baseline `44192b2` with the final executable identified by its SHA-256.
Both ran pinned to CPU 5 on this aarch64 host. Primitive medians, in nanoseconds
per call:

| Public API case | Before | After |
| --- | ---: | ---: |
| Exact distance ordering, near tie | 43.80 | 40.22 |
| Exact distance ordering, wide exponents | 42.71 | 41.80 |
| Exact distance ordering, identical points | 51.15 | 3.25 |
| Ordinary dot product | 4.29 | 4.35 |
| Overflow-cancelling dot product | 92.00 | 21.44 |
| Subnormal dot product | 92.52 | 24.34 |
| Wide-exponent cancelling dot product | 96.75 | 101.17 |

The compact dot cases improved by 4.29× and 3.80×. Classification adds work when
the full-range fallback is still needed: the wide-exponent dot case was 4.6%
slower. Ordinary dots do not use the new selector; their small timing difference
does not establish a change in that path.

Curve closest-point medians were 24.67 → 20.84 µs for the rational circle,
25.02 → 25.36 µs for the nonuniform multispan curve, and 9.25 → 8.57 µs for the
endpoint query. The multispan result was 1.3% slower, so this is not a universal
query speedup. Ordinary curved-surface queries took 2.7–8.7% less time; affine
surface cases took 1.8–4.2% less. Signed-weight sweep whole-process timings
changed by less than 0.3%, too little to infer an improvement from one pair.
Curve allocation counts and requested bytes stayed unchanged (14, 59, and 13
allocation calls for the three closest-point cases).

Every returned value, including signed-zero bits, matched the baseline across
the 15 ordinary curve batches, 190 ordinary surface query results, all 270
full signed-sweep samples and their two complete definitions, and the smaller
endpoint/off-surface sweep checks. All five extreme closest-point operations
and 23 rational-curve fixture operations also matched bit-for-bit. These are
before/after native comparisons; no fresh Rhino session was run for this scalar
optimization. The Rhino-only `distance-command.json` history fixture is not
supported by the native oracle and is not included in these counts.

The final release workspace passed 2,575 tests with 18 existing opt-in tests
ignored. All 196 Python tests, strict all-target Clippy, warnings-denied rustdoc,
and formatting checks passed. Regenerated scaled-dot (256 cases), rational-curve
(127 cases), and rational-surface (150 cases) references were byte-identical to
the committed files.

These are diagnostics, not timing gates. No builds or Rhino launches overlapped
the timing runs, but host frequency and core isolation were not controlled.
Before/after groups ran sequentially; small differences may be noise. The
report retains raw batches, executable hashes, and value fingerprints.

## Reproduction

The primitive diagnostic uses public geometry APIs, five batches of 20,000
queries per case, and black-boxed inputs/results:

```sh
cargo build -p viboceros-geometry --example profile_exact_predicates --release
taskset -c 5 target/release/examples/profile_exact_predicates
```

Preserve the baseline release executable before rebuilding. The curve oracle
benchmark uses `nurbs_curve_closest.json`, five batches of 500 iterations with
unique operation IDs. Surface timings and raw value-audit artifacts use:

```sh
python3 tools/numerics/benchmark_surface_query.py /path/to/oracle-before target/release/viboceros-oracle --cpu 5 --rounds 1 --baseline-commit 44192b2f9c4a801d7baf42c2f0ad777242599320 --artifacts /path/to/new-diagnostic-directory
```
