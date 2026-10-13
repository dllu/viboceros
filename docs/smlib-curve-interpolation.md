# Native point interpolation

Linux `native-smlib` builds route InterpCrv and the interpolated modes of
CurveThroughPt/CurveThroughPolyline through usd-brep's cubic basis and matrix
routines. The bridge exports `interpolate_curve` for SDK policy and
`interpolate_curve_for_command` for command policy. The curve-through API and
oracle reuse the same preparation and producer.

`CubicInterpolationSystem` is shared with Loft; its existing `LoftInterpolation`
name remains an alias. The geometry layer validates degree, point counts,
coincidence, endpoint repetition and tangent/closure combinations before solving.
It retains knot spacing, automatic or requested tangent directions, handle
lengths, local-coordinate conditioning, constant coordinate channels, exact
endpoint/handle payloads, periodic repetition and polynomial weights.
Degree-one and unconstrained two-point construction bypass the solver. SDK and
InterpCrv retain their distinct two-point degree policies.

Open cubic systems use the native five-wide banded matrix routines after ordering
handle constraints beside the endpoints. Matrix storage, factoring, back-solving
and residual checks retain linear complexity in point count. Periodic systems
use the existing full native basis/factorization with folded control columns.
Both paths share the homogeneous back solve, exact span checks and normalized
residual checks introduced for Loft. Sources are immutable and native handles
stay outside document history; backend errors propagate through typed command
conversion.

The portable model retains its tridiagonal/pivoted fallback and periodic faer
solve. [Native TweenCurves sampling](smlib-curve-tween.md) now shares the banded
producer beyond the public 256-point interpolation budget; portable builds keep
their original linear-memory path without packing dense job data.
No additional basis evaluator or linear solver is implemented.

The [native regressions](../crates/viboceros-smlib/tests/curve_interpolation.rs)
compare complete bases, degrees, periodic flags, polynomial weights, controls
and 65 parameter samples against the portable model. They cover three knot
spacings, degree one/cubic, open/smooth/sharp closure, independent endpoint tangent
options, SDK/command two-point policies, smooth two-point cubic systems, nearby
translated command points, invalid requests and coordinate detail down to
`1e-280`. Shared-policy tests verify solver bypasses, constraint layouts,
immutable input locations, bad output rejection and validation before entry.
Open banded regressions also exercise 3, 4, 64 and 256 input points with all
three spacings.

The [retained comparison](smlib-curve-interpolation-comparison.json) contains
30 interpolation and 13 curve-through full-record comparisons against the
portable backend; all 43 match at absolute `1e-10`, relative `1e-12`.
The same native oracle replays 365 retained Rhino extraction/refinement/seam
records: 364 complete raw matches, with the existing whole-surface unused-knot
padding discrepancy retained. These earlier captures do not establish fresh
SDK interpolation parity.

The complete optimized geometry/command/bridge run passes 3,607 tests (11
ignored), including the open banded solver and public-budget regression. The
Python oracle suite passes 1,097 tests. Workspace lint passes with warnings
denied in both feature configurations, and formatting passes.

The `curve_interpolate_geometry` oracle accepts points, degree, knot spacing,
closure and optional endpoint tangents for direct SDK comparisons. The new
fixture covers those policies. Fresh reference capture remains unavailable
pending the Wine licensing startup described in [isocurve qualification](smlib-isocurves.md).
Native LU/basis tolerances and resource bounds still apply; arbitrary spacing or
coordinate ratios, continuous parity and kernel speedups remain unqualified.

```sh
CMAKE_BUILD_PARALLEL_LEVEL=10 cargo test --release -p viboceros-smlib \
  --features native --test curve_interpolation --test loft_basis
```
