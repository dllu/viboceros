# Regular surface normals

`NurbsSurface::normal_at(u, v)` returns the natural U×V unit direction, separately
from point and derivative representability. Model-length tolerance is not an
appropriate cutoff for this direction: rescaling U or V changes derivative
magnitudes without changing the surface normal. The former tolerance argument
has therefore been removed. Parameter validation and right-sided knot selection
remain strict; outer endpoints select the interior span.

The previous implementation also crossed already-rounded first derivatives.
For an affine patch with exact `Su=(2^53+1,2^53,0)` and `Sv=(2^53,2^53,0)`, both
returned derivatives round to the same vector, but the exact cross is `(0,0,2^53)`.
Regression tests reproduced this false degeneracy before the fix.

## Bounded arithmetic and recovery

The normal query has its own outward-rounded arithmetic filter, rather than
assuming that finite rounded jets certify their cross. It centers and scales the
active net, normalizes weights, evaluates the homogeneous point and partials,
and bounds their cross and unit-length normalization. Positive span-width factors
keep derivative speeds bounded without changing orientation. Each arithmetic
operation rounds its interval outward; a denominator interval containing zero,
unbounded result, or insufficient final enclosure selects exact recovery.
The filter accepts only when every final component is within `1e-12` of the exact
regular normal. Interval operations are checked against independent exact rational
results, including subnormal arithmetic and overflow.

The exact path uses the original binary64 inputs as rationals. With homogeneous
point `H` and weight `W`, it crosses `Hu W − H Wu` with `Hv W − H Wv`; the omitted
denominator `W^4` is positive. Scaling that exact cross before rounding avoids
overflow and underflow, without a guessed degeneracy epsilon. Exact poles and
zero first-order crosses remain errors. Limiting normals at singular surface
parameterizations are still unsupported. This work does not change `frame_at`,
curvature, or implement a spatial B-rep orientation classifier.

## Evidence

The [20-case source request](../tools/rhino_oracle/fixtures/surface_normals.json)
and [Rhino capture](../tools/rhino_oracle/observations/surface_normals.json) vary
plane size, U/V domain scale, transposition, and rational weight gauge. The worker
now calls the public [Surface.NormalAt API](https://mcneel.github.io/rhinocommon-api-docs/api/RhinoCommon/html/M_Rhino_Geometry_Surface_NormalAt.htm),
rather than repeating the native implementation's old derivative cross and cutoff.
It records zero API results verbatim and disposes surfaces on success or failure.
Older records are not rewritten or retroactively treated as public normal queries.
The [provenance](surface-normal-provenance.json) records hashes and capture scope.

Separately, `tools/numerics/generate_surface_normal_reference.py` supplies 95
independent Fraction/Bernstein cases, with 100-digit Decimal unit normalization.
They cover degree through `(5,4)`, signed and extreme weights, extreme domains,
rounded-tangent cancellation, poles, and exact degeneracies. The filter accepts
71 cases; the remaining 24 exercise exact recovery. Other regressions check
unclamped polynomial reproduction, creases, endpoints, invalid parameters, and
model/parameter scales from `1e-200` through `1e200`.

```sh
tools/rhino_oracle/run_headless.sh rhino \
  tools/rhino_oracle/fixtures/surface_normals.json --timeout 900
python3 -m tools.rhino_oracle replay \
  tools/rhino_oracle/fixtures/surface_normals.json \
  --observations tools/rhino_oracle/observations/surface_normals.json
cargo test -p viboceros-geometry normal
python3 -m unittest tools.rhino_oracle.test_surface_normals
```

Oracle timings still measure derivative evaluation, **not** normal queries. An
ignored native `regular_normal_query_microbenchmark` compares the filtered and
exact paths; neither set of timings establishes a Rhino normal-speed comparison.
