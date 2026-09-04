# Exact piecewise curves

[Architecture](architecture.md) · [Curve commands](commands/curves.md)

`PolyCurve3` is the kernel's flat, exact piecewise NURBS representation. Segments
retain their degrees, rational weights, control points, knots, and local domains.
An independent increasing array assigns each segment its interval in the composite.
There is no fitting, tessellation, or endpoint averaging during construction.

The kernel supports evaluation and first/second derivatives, explicit left/right
segment choice at junctions, length, trimming, splitting, reversal, affine transforms,
flattened concatenation, and exact conversion to a single piecewise NURBS curve.
`CurveRef::PolyCurve` enables shared arc-length sampling, planarity checks, curvature,
and curve-rebuilding/fitting algorithms. Document objects, viewport handling, 3DM
polycurve interchange, and mixed-curve `Join`/`CloseCrv` integration are still pending.

## Parameterization and numerical policy

For outer interval `[a,b]` and local domain `[c,d]`, evaluation maps linearly between
the intervals. Derivatives acquire factors `(d-c)/(b-a)` and its square. Endpoint
maps are exact; interior maps use the nearer endpoint to reduce rounding error.
Nonfinite intervals, collapsed parameter spans, noncoincident adjacent endpoints,
invalid indices, and out-of-domain evaluation return errors.

Two reparameterizations are deliberately distinct:

- `try_reparameterized` affinely rescales the existing outer intervals.
- `try_reparameterized_by_length` distributes the new interval in proportion to
  segment arc lengths, preserving each segment's internal parameterization.

The latter matches the observed Rhino 8 polycurve Domain setter. Its public
[RhinoCommon wrapper](https://github.com/mcneel/rhino3dm/blob/main/src/librhino3dm_native/on_curve.cpp)
routes polycurves through Rhino's reparameterization operation, whereas standalone
OpenNURBS `SetDomain` performs affine rescaling. Length-based redistribution is not
a constant-speed parameterization of a curved segment.

The constructor uses the kernel's fixed curve-coincidence predicate, not document
model tolerance. Bridging or editing larger gaps belongs to a joining operation.
Composites allow up to 65,536 segments; NURBS conversion allows at most one million
output controls. Conversion elevates degrees and uses full-order junction knots,
preserving independent homogeneous scales without dividing adjacent weights. Its
control structure need not be Rhino's minimal merged NURBS structure.

## Division and validation

`CurveRef::divide_by_count` follows Rhino's endpoint topology: `include_ends=true`
includes both open endpoints or one closed seam; false returns only the interior
stations. `sample_equal_length_points` instead always retains the final boundary,
including a repeated closed seam for algorithms that need it. Point/tangent sample
arrays and tween interpolation also retain that explicit boundary convention.

Analytic tests check line/circular-arc composites, both derivatives under domain
scaling, reversal, trimmed intervals, periodic and unclamped input, homogeneous
scales spanning `1e-200` to `1e200`, and large translations. The permanent
`polycurve.json` oracle compares segment definitions/domains, point and derivative
samples, length, closure, and division through reversed, trimmed, split, closed,
and nonplanar mixed-degree cases. Sampling is evidence at those stations, not a
continuous error proof.

The oracle checks `DivideByCount` point counts directly. Coordinate comparisons
use the exact NURBS form and the public tolerance-bearing length solver, because
the composite division APIs retain coarser internal length inversions. Probe
timings include validation and serialization and are not kernel-only benchmarks.

```sh
tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/polycurve.json \
  --absolute-epsilon 1e-8 --relative-epsilon 1e-10

tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/curve_division_contract.json \
  --absolute-epsilon 1e-9 --relative-epsilon 1e-10
```

The 14 polycurve cases passed against Rhino 8. Most maximum differences were
around `1e-12`; length-based domain redistribution differed by up to `9.68e-9`.
The native redistribution is independently checked against analytic segment lengths.
