# Exact piecewise curves

[Architecture](architecture.md) · [Curve commands](commands/curves.md)

`PolyCurve3` is the kernel's flat, exact piecewise curve representation.
`CurveSegment3` retains native lines, circular arcs, polylines, and NURBS curves,
including their local domains, vertex parameters, and rational control structure.
An independent increasing array assigns each segment its interval in the composite.
There is no fitting, tessellation, or endpoint averaging during construction.

The kernel supports evaluation and first/second derivatives, explicit left/right
segment choice at junctions, length, trimming, splitting, reversal, affine transforms,
flattened concatenation, and conversion to a single piecewise NURBS curve with
the bounded junction policy described below.
`CurveRef::PolyCurve` enables shared arc-length sampling, planarity checks, curvature,
and curve-rebuilding/fitting algorithms. Document objects preserve segments through
affine transforms and nonlinear morphs. Viewports draw and pick each segment; endpoint
snaps include interior junctions. 3DM interchange retains the composite object type.
[Mixed-curve joining and closure](curve-editing.md) can move eligible endpoints
or append an exact closing line while retaining the original segment intervals.

`Length`, `Divide`, curve selection, `Flip`, and `ToNURBS` accept polycurves.
`ExtractPt` shares coincident junction grips; `ExtractControlPolygon` creates
one connected polygon through the original segment controls without degree elevation.
`Explode` returns native segment types in composite parameter intervals, preserving
attributes, group membership, and undo. Polyline leaves are further exploded into
native lines with their corresponding intervals. Closed polycurves are unchanged by `CloseCrv`.

Duplicate comparison retains segment structure and relative parameter intervals.
Affine changes to the overall domain are ignored; length-based redistribution can
change the geometry value even though the locus is unchanged. Open composites compare
in either direction; tested closed composites retain segment order and direction.
Cross-representation equivalence with a polyline or a single merged NURBS is not
implemented for this type.

## Parameterization and numerical policy

For outer interval `[a,b]` and local domain `[c,d]`, evaluation maps linearly between
the intervals. Derivatives acquire factors `(d-c)/(b-a)` and its square. Endpoint
maps are exact; interior maps use the nearer endpoint to reduce rounding error.
Nonfinite intervals, collapsed parameter spans, noncoincident adjacent endpoints,
invalid indices, and out-of-domain evaluation return errors.

Analytic arcs use angular parameterization. Their exact rational NURBS forms
generally evaluate to different interior points at the same numeric parameter;
conversion preserves the locus, not angular speed. Arc trims, reversals, and
similarity transforms retain the analytic frame and native interval. Arbitrary
affine transforms promote only the arcs that cannot remain circular to NURBS.
`try_deformable` explicitly converts all arc leaves, like Rhino's `MakeDeformable`.
This is distinct from directly calling Rhino's polycurve `Transform` under a
shear: that API approximates circular segments and can adjust neighboring
endpoints. Viboceros' affine operation preserves the exact transformed locus;
the shear oracle explicitly uses `MakeDeformable` before Rhino's transform.

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
output controls. A closed segment may only be the entire composite, matching
OpenNURBS validity rules. Conversion elevates degrees, matches representable homogeneous
scales, and shares coincident endpoint midpoints with degree-multiple knots. Endpoint
movement is bounded by the fixed curve-coincidence predicate. If rescaling would overflow
or erase a nonzero weight, full-order knots preserve independent scales. No overflowing
ratio is required merely to rescale representable weights. This fallback need not match
Rhino's minimal control structure.

## Division and validation

Shared evaluation and returned sampling parameters follow the
[native curve parameter contract](curve-parameters.md).

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

`polycurve_native.json` additionally checks all four native leaf types, native
point/derivative samples, analytic trims/splits, reversal, similarity/reflection,
and exact shear. The Rhino Domain setter's internal segment-length estimates
produce about `1.1e-8` of interval drift in the curved mixed fixture; this batch
uses absolute comparison epsilon `2e-8`, relative `1e-10`. The kernel's requested
length tolerance is not relaxed. Analytic endpoint editing is covered separately
by `polycurve_analytic_editing.json`.
`polycurve_native_document.json` compares actual extraction, recursive Explode,
duplicate checks, and 3DM round trips in both directions. It passes at absolute
epsilon `1e-8`, relative `1e-10`, with maximum observed difference below `1.6e-12`.
Unordered extracted points are sorted using rounded keys to prevent floating-point
noise from swapping nominally equal X coordinates; the compared coordinates
themselves are never rounded.

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

`polycurve_document.json` additionally checks actual Rhino extraction/explode
commands, duplicate comparison, and 3DM round-trip segment definitions. Native probes
verify command undo. These timings include command dispatch, file I/O, and validation;
they are not measurements of kernel-only performance.

All seven document cases passed against Rhino 8 at absolute `1e-8`, relative `1e-10`.
Six cases matched numerically exactly; the length-redistributed case retained the
same maximum `9.68e-9` parameter difference noted above.
