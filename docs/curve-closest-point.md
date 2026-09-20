# NURBS curve closest points

[Architecture](architecture.md) · [Surface candidate selection](surface-candidate-selection.md)

`nurbs/closest_point` separates bounded search from curvature-aware refinement.
This audit reproduces six failures of the previous search at `d35a779`: rounded
distance ties, overflowing distances, overflowing displacements, overflowing
derivative products, mixed-sign poles, and recentering that loses a target offset.

## Selection and refinement

Each nonempty span contributes its endpoints and midpoint; 33 uniform stations
supplement them. Selection retains sixteen starts using exact distance ordering
of their **original evaluated points**, with native parameter order breaking
true ties. Only the retained starts are sorted. Invalid evaluations are skipped;
a finite point is not discarded merely because its distance overflows.

Curves and surfaces share immutable `point/distance_key` values: conservative,
outward-rounded squared-distance bounds resolve disjoint intervals quickly.
Overlaps invoke `Point3::compare_distances`, not an approximate tie threshold.
The bounds retain their independent arbitrary-precision tests (144 explicit
extreme pairs and 1,024 generated finite pairs).

`closest_point/refine` projects the exact point difference onto a normalized
tangent. It need not construct a finite displacement or multiply the residual
by an unnormalized derivative. The shared vector `parameter_step` converts
model-space motion into parameter motion without requiring a representable
derivative norm, including subnormal inputs. Curvature supplies a Newton proposal
when its scaled Hessian is positive and finite; otherwise the tangent proposal
remains available. Second-derivative failure preserves the first-derivative
fallback. Up to 24 backtracks per direction accept only non-increasing exact
distances between evaluated points, within a 64-iteration limit.

Recentered curves can improve stationarity calculations, but are only proposal
generators. Original curve evaluations and the original target rank both seeds
and returned proposals. A zero hit is coordinate equality, never rounded norm
equality or model tolerance. Only a valid hit at the first active parameter
terminates the whole search immediately; other starts can still produce a
lower-parameter tie. Intersection snapping retains its separate requirement
that the returned distance fit in binary64.

## Independent regression cases

* A unit line queried at `(.37,1e100,0)` projects to `.37`, although all reported
  distances round to `1e100`. Tests include wide and tiny parameter domains.
* A diagonal unit line queried at `(MAX,MAX,MAX)` chooses its upper endpoint,
  despite every Euclidean distance overflowing.
* A unit Y line at X=`MAX`, queried at `(-MAX,.37,0)`, still projects to Y=`.37`;
  its X displacement cannot be stored in binary64.
* A line with speed `1e200`, queried at `(.37e200,1e200,0)`, projects to `.37`.
  A second test uses three `MAX` derivative components, whose norm overflows.
* The degree-one signed curve `x(t)=-t/(1-2t)` has a pole at `.5`. Target X=`-.5`
  projects to `.25` on its valid branch; target X=`.5` has an exact endpoint tie
  resolved to zero. Both common weight signs are tested, with distant Y offsets.
* An upper rational semicircle of radius `R=2^54`, queried at `(1,-R,0)`, chooses
  its right endpoint. Recentring at the first control rounds `R+1` to `R`, falsely
  suggesting symmetry. Target X=`-1` and zero both choose the left endpoint.

The semicircle uses the usual quadratic quarter-arc controls and positive
weights `(1,sqrt(.5),1)`. Its endpoint minimum does not depend on treating the
rounded middle weight as an exact circle: convex combinations confine the right
quarter to `X+Y >= R`, `0 <= X <= R`, whose closest point to the target is `(R,0)`.
The left quarter satisfies `-X+Y >= R`, `-R <= X <= 0`; its squared distance is
at least `2(R+.5)^2`, exceeding the right endpoint's `(R-1)^2+R^2` by `4R-.5`.

## Licensed Rhino comparison

The [five-case fixture](../tools/rhino_oracle/fixtures/nurbs_curve_closest_robust.json)
has formula-derived native expectations, not golden values copied from our
implementation. Fresh Rhino **8.32.26160.13001** ran on an owned private Xvfb/i3
display. The [validated reference](curve-closest-rhino-reference.json) contains
three cases: both distant line projections agree exactly, while the semicircle
returns the **left** endpoint. Its rounded distance equals the right endpoint's
distance, but the points differ by `2R`. The [comparison](curve-closest-comparison.json)
explicitly fails; no widened epsilon hides this disagreement. The
[native response](curve-closest-native-reference.json) retains all five cases.

Two Rhino cases are not usable reference geometry. Rhino rejects the signed
curve as invalid. For the `1e160`-speed line, the original adapter serialized
an out-of-domain parameter and point coordinates `-1.23432101234321e308`, with
distance zero. The [diagnostic record](curve-closest-rhino-diagnostics.json)
preserves that raw observation separately. The adapter now rejects non-finite or
out-of-domain parameters, invalid points, and non-finite or negative distances,
and disposes the curve even on failure. Surface probes also validate returned
parameters and points, outside their search-only timed region.

Distances that cannot be serialized finitely are tested in the geometry crate,
not through the JSON distance-reporting probe. The test replay records the
semicircle discrepancy explicitly and never treats rejected or invalid Rhino
outputs as expected geometry.

## Limits

This is bounded numerical search, not a certified global minimum of every
rational curve. Exact comparison orders the stored evaluated points; it does
not make floating-point curve evaluations exact. Unsampled poles, local minima,
unrepresentable refinement projections, and information lost inside a proposal
frame remain limitations. Original-frame ranking prevents a rounded proposal
from replacing a better incumbent, but cannot invent an unproposed parameter.

## Performance

The [release record](curve-closest-performance.json) compares `d35a779` with this
implementation on aarch64, pinned to CPU 5. Five batches of 500 iterations per
curve time search plus final evaluation/distance, excluding construction:

| Curve query | Before | After | Time increase |
| --- | ---: | ---: | ---: |
| Rational quarter circle | 15.22 µs | 29.17 µs | 92% |
| Nonuniform rational multispan | 27.25 µs | 31.98 µs | 17% |
| Endpoint line | 6.44 µs | 9.15 µs | 42% |

This is a correctness improvement with a measured performance cost, not a speedup.
The first two returned points change slightly; exact rational comparison of
their original-point squared distances is retained in the record. Endpoint output
is bit-identical. Conservative bounds avoid exact predicates for well-separated
candidates, but ambiguous near-minimum comparisons and range-safe projections
still cost more than the old arithmetic. Further profiling is warranted.

The downstream surface benchmark uses five batches of 200 iterations for each
ordinary case. Curved cases take 12–26% more time; affine cases stay within 2%.
Of 190 query results (including repeats), 15 change across three unique queries.
Their largest point-coordinate change is `9.249e-11`, and largest normalized UV
change is `2.855e-11`. Exact rational comparison finds ten strictly closer points,
five equal-distance points, and none farther. Native parameter changes reach
192.96 on an extreme-width domain; normalized UV and model-space errors remain
small, so a bare native-parameter epsilon would be misleading.

Signed-weight sweep timings use one whole-process pair, not a statistically
established speed estimate: endpoints 61.17→62.39 ms; off-surface 1.024→1.009 s;
all 270 queries 39.29→39.26 s. Endpoint/off-surface values remain bit-identical.
Only four full-fixture sample points change, by at most `8.882e-16`; every
returned surface definition remains bit-identical.

Preserve the baseline release executable before rebuilding. The curve request
uses `nurbs_curve_closest.json`, with `iterations=500` and its three operations
repeated five times with unique `-round-N` ID suffixes. Run both executables with
`taskset -c 5`; take the median of each operation's five `elapsed_ns` values.
For surfaces and retained raw value-audit artifacts:

```sh
python3 tools/numerics/benchmark_surface_query.py /path/to/oracle-before target/release/viboceros-oracle --cpu 5 --rounds 1 --baseline-commit d35a779755cfa87508e3253167919f1c1a4147c4 --artifacts /path/to/new-diagnostic-directory
```

No builds or Rhino launches ran during measurement. Clock frequency, core
isolation, and unrelated host activity were not controlled. Fresh Rhino fixture
timings include bridge/emulation overhead and warm-up; they are not kernel
speedup measurements.

## Validation

The unfiltered release workspace suite passes **2,565 Rust tests**, with **18
existing opt-in tests ignored**. All **196 Python oracle tests**, strict
all-target Clippy, warnings-denied rustdoc, formatting/diff checks, stored
comparison replay, and local documentation links pass. Regenerating the 150-case
exact rational surface reference produces byte-identical output. No existing
geometric assertion was weakened to accommodate this change.
