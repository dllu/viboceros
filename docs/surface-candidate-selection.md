# Surface closest-point candidate selection

[Closest points](surface-closest-point.md) · [Query cache](surface-query-cache.md)

The general surface search now compares candidates on one consistent basis:
their evaluated points on the original surface. Three independent regressions
failed at `b28d039` and pass with this selection change.

## Reproduced failures

### A boundary-curve hit that is not a surface hit

A degree-(2,1) rational planar patch has a repeated first U control and a large
X offset. At V=0 its first two weights are `23/101`; the third is one. The first
X coordinate is `10000000000000002`, and the last is 12,800 units larger.
Normalizing a column during isocurve extraction and projecting its rounded
homogeneous coordinates changes the first X coordinate by two units.

The extracted curve therefore evaluates to target X=`10000000000000004` at U=0,
while the original surface evaluates to X=`10000000000000002`. Treating this
curve point as a zero-distance best candidate prevented later genuine surface
hits from improving it. The repeated control makes the starting U derivative
singular, so final polishing did not repair it. The old search returned `(0,0)`
and a point **two model units away**.

An independent rational quadratic formula supplies a genuine hit:
`u² = 2w / (12800 − 2(1−w))`, `v=0`. The regression checks that point and verifies
that the selected native point now equals the target. It does not require one
particular U inside a rounded-coordinate plateau.

### Rounded distances erase an interior peak

Consider the polynomial patch
`S(u,v) = (u, v², 256u²(1−u)²v²(1−v)²)` on the unit square. Its unique highest
point is `(.5,.25,1)` at `u=v=.5`. It is directly below targets at
`(.5,.25,H)`, for `H=1e20`, `1e100`, or `MAX`.
The stored Bernstein coefficient `64/9` is rounded to binary64; that preserves
the location of the maximum, whose evaluated height rounds to one.

All grid distances round to the same binary64 number. Sorting those rounded
numbers and keeping sixteen starts retained only the first V row, whose
derivative in V is identically zero. Those singular starts could not refine to
the peak. The old search returned `(0,0)` for the first test. Ranking stored
points exactly before truncation retains the peak instead.

### Finite geometry with unrepresentable distance

For `S(u,v)=(u,v,uv)` and target `(MAX,MAX,MAX)`, the unique minimum is `(1,1)`.
Every Euclidean distance overflows, so the old grid filter discarded every
candidate and reported a degenerate search. The new search returns the correct
corner without requiring a finite distance. A second test has an overflowing
coordinate subtraction on a patch at X=`−MAX`, not merely an overflowing norm.

## Selection mechanics

`closest_point/candidate.rs` pairs UV parameters with the shared immutable
`point/distance_key.rs` key, also used by [curve searches](curve-closest-point.md).
The key stores each point and an enclosure of its exact squared distance from
the fixed target. Every coordinate subtraction,
square, and sum is rounded outward using adjacent floats. A nonnegative lower
bound and possibly infinite upper bound cover subnormal and overflowing cases.
No square root or bound on a library `hypot` implementation is assumed.

Disjoint bounds certify distance order cheaply. Overlapping bounds use the
existing exact `Point3::compare_distances` predicate. The interval is only a
filter: it never resolves an ambiguous ordering approximately.

Deterministic partial selection retains the best sixteen grid points, then sorts
only those starts. Exact distance ties are resolved by V then U, reproducing the
original unique, ascending V-major grid order. Tests compare this against a full
exact stable sort for empty, short, threshold, and 1,089-cell grids with many ties.

Boundary curves only propose parameters; their comparison points are evaluated
on the original surface. Refinement and normalized-domain recovery use the same
candidate type. A failed recovery candidate does not replace a valid incumbent.
Refinement returns parameters, not a second independently cached best distance.

## Independent validation and limits

The distance enclosures are checked against arbitrary-precision rational squared
distances for 144 explicit extreme input pairs and 1,024 generated finite cases.
The filtered comparator is also checked against the independent exact predicate.
Additional tests exercise ordinary disjoint bounds and stable exact-distance ties.

This is exact ordering of **stored evaluated points**, not certified global
optimization of an arbitrary rational surface. The search still uses bounded
sampling and local refinement. Overflowing residuals may prevent refinement even
though candidate selection can retain a valid point. Isocurve extraction itself
still rounds controls; this change does not make its image bit-identical to the
original surface or locate unsampled poles.

## Licensed Rhino comparison

The [fixture](../tools/rhino_oracle/fixtures/surface-closest-candidates.json)
contains the translated boundary case and distant peak. Fresh Rhino
**8.32.26160.13001**, on an owned private Xvfb/i3 display, returns the same model
points and distances as the new native implementation. The peak's UV also agrees
exactly at `(.5,.5)`.

The translated case has a real parameter discrepancy: Rhino reports the target
at U=0; the original native surface evaluates two units away there. Native U is
approximately `0.0043733` for the same rounded hit. The
[full comparison](surface-candidates-comparison.json) therefore **fails on U**;
this is not hidden behind a wider UV tolerance. Both
[native](surface-candidates-native-reference.json) and
[Rhino](surface-candidates-rhino-reference.json) records are retained. The Rust
replay independently asserts the prescribed model points and records the UV
discrepancy explicitly.

The distance-overflow cases are native analytic tests, not Rhino comparisons:
the current JSON closest-point probe requires a finite serialized distance.

## Performance

The [release record](surface-candidates-performance.json) compares `b28d039` with
the final bounded-selection implementation on aarch64, pinned to CPU 5.
Ordinary curved queries take **3.7–6.5% more time** for exact candidate ranking;
the unchanged affine path measures 0.7–1.1% slower. All 190 ordinary benchmark
query results remain bit-identical. Each ordinary case uses five rounds of 200
iterations.

The signed-weight sweep workloads use one whole-process pair each, so these are
bounded diagnostic timings, not statistically established speed changes:

| Workload | Before | After |
| --- | ---: | ---: |
| Two endpoint queries | 60.81 ms | 62.85 ms |
| Six off-surface queries | 1.010 s | 1.022 s |
| All 270 fixture queries | 40.481 s | 39.410 s |

The endpoint and off-surface values remain bit-identical. In the full workload,
18 of 270 sampled points change, with a maximum coordinate change of
`4.545e-10`; all surface definitions remain bit-identical. This is below the
fixture's `1e-7` modelling tolerance. Boundary and seed selection may legitimately
choose a different evaluated point; preserving the old rounded ranking is not
a correctness requirement.

Reproduction, preserving the previous release executable before rebuilding:

```sh
python3 tools/numerics/benchmark_surface_query.py /path/to/oracle-before target/release/viboceros-oracle --cpu 5 --rounds 1 --baseline-commit b28d039f93646db77f42a6f4b75b1cbd0b3d9fc0 --artifacts /path/to/new-diagnostic-directory
```

The optional artifact directory retains raw requests and responses for auditing
value changes. No builds or Rhino launches were initiated during measurement;
clock frequency, core isolation, and unrelated host activity were not controlled.

Final validation: **2,557 Rust tests pass** in the unfiltered release workspace
suite, with **18 existing opt-in tests ignored**. All **195 Python oracle tests**,
strict all-target Clippy, warnings-denied rustdoc, formatting/diff checks, and
unchanged regeneration of the 150-case exact rational surface reference pass.
