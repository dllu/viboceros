# Native exact curve and surface splits

`N_CrvSplit` and `N_SrfSplit` now classify the exact parameter and actual knot
multiplicity. Full-order groups copy independent left and right control blocks,
without sharing a pole. This fixes the curve buffer overrun/underflow and the
surface producer's former successful return without producing outputs. Curve
splits no longer snap the requested parameter using a domain-relative `1e-6`
threshold. Other editing span helpers and modeling tolerances are unchanged.

Both producers preserve the original outer padding on their respective sides.
The selected knot vectors consist of source knots strictly below the parameter
followed by `degree+1` copies, or those copies followed by source knots strictly
above it. The unselected surface basis is copied intact. At an unclamped active
endpoint, the unused outer piece can have an empty active domain and fewer than
`degree+1` controls. It is native intermediate storage, not a publishable NURBS
curve/surface; the endpoint adapters retain the nonempty side. Full-vector outer
endpoints remain excluded, and queries outside the active domain return errors.

Output objects and backing buffers must be distinct from the source and each
other. Object aliases are rejected. Null-stack calls require preallocated
outputs: capacities are checked, and oversized surface metadata is reset to the
actual result dimensions. Native errors can leave partial output, so callers
must honor status before publishing it.

The shared endpoint adapters now split directly in both directions, removing
their curve/surface reversal wrappers. Bezier preparation also uses these
splitters to remove inactive endpoint controls. The refinement adapter retains
its direct Euclidean-payload copy for already full-order endpoint groups:
homogenizing and projecting an unchanged control could round a coordinate.
This preserves exact user payloads without implementing geometric restriction.

Public point/parameter Split still uses the checked interval extractor for its
two fully clamped outputs. That representation policy differs from the native
splitter's retained outer padding; the two extractions are no longer a workaround
for an unsafe primitive. Closed seam assembly retains its existing policy.

The direct matrix covers 7,452 operations across curves and both tensor axes:
23 families, four charts, three allocation paths and nine representation/gauge
combinations. Degrees are `0`, `1`, `2`, `3`, `5` and `17`. Charts use `2^-360`,
`1`, `2^360` and origin `1e12`; weight gauges are `1`, `-1`, `2^-300`, `2^300`,
with homogeneous uniform/rational weights and native polynomial `NL_NOW` storage.
Near-group fixtures use an adjacent representable parameter when translation
would collapse their decimal offset onto a knot. A one-ULP final span is explicit.

Independent Bernstein polar products, uniform quadratic/cubic identities and
isolated control blocks define both expected outputs. Degree, dimensions and
all knots must be exact. Homogeneous differences divided by the common gauge
are bounded by `5e-13 * max(1, abs(expected/gauge))`; polynomial sentinels remain
exact. All 972 full-order configurations require bit-exact control payloads.
Source/output scalar addresses must be disjoint, and both outputs are reread
after source destruction. Allocated, exactly preallocated and oversized
preallocated outputs are tested separately; preallocated calls use a null stack.

Thirty-six rejection cases cover object aliases, missing allocation storage,
insufficient capacities, nonfinite parameters and excluded domain bounds. The
original-object control executes only 1,053 safe unit-chart cases, passing 738.
Unsafe parent full-order/active-end calls are excluded rather than rerun. Its
failures retain curve snapping and oversized surface metadata discrepancies.

Fresh public Rhino SDK evaluation covers 99 paired stations: 33 curve and 66
surface stations. It evaluates original and split definitions at identical
parameters, excluding empty native outer pieces. Maximum point difference is
below `5.7e-14`; derivatives and normals remain diagnostics. This is sampled
locus agreement, not direct Rhino Split definition parity or a continuous bound.

Release suites pass 417 bridge tests and 1,600 command tests, with zero failures
and four existing ignored tests. All 7,488 direct cases pass under
AddressSanitizer. Workspace lint and formatting pass. The 112 saved join
outcomes remain unchanged; their existing strict Rhino representation
differences are retained in the reports.

The [validation record](native-exact-splits-validation.json) retains complete raw
definitions, instrumented results, SDK requests/responses and completed checks.
Generated-source patches compose with the earlier extraction repairs; upstream
remains unchanged. No arbitrary conditioning, overlapping buffers through
distinct objects, extrapolation, kernel speedup or leak-free claim is made.
Sanitizer execution disables leak detection. Upstream USD, Python binding, OCCT,
GUI and `repo.sh`/`prog_test` harnesses were not run.

```sh
cargo test -p viboceros-smlib --features native \
  --test exact_split --test knot_refinement --test directed_segments
```
