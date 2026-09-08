# Native curve parameters

[Geometry architecture](architecture.md) · [Polycurves](polycurves.md)

`CurveRef` exposes one checked native-domain contract for `domain`, `parameter_at`,
`evaluate`, first/second derivatives, tangents, and arc-length sampling. `Curve3`
owns the same representations; `try_reparameterized` changes their intervals
without changing the locus. `CurveSample::parameter()` belongs to the source
curve's native interval, including negative reversed intervals.
[One-sided evaluation](curve-sided-evaluation.md) selects exact left/right limits
at knots, including knots inside composite leaves and stationary limiting tangents.

| Curve | Initial native interval | Parameterization |
| --- | --- | --- |
| Line | `[0,length]` | Linear |
| Circle | `[0,circumference]` | Uniform angle |
| Circular arc | `[0,arc length]` | Uniform angle |
| Ellipse | `[0,2π]` | Four rational quadratic spans, matching Rhino's NURBS ellipse |
| Polyline | `[0,vertex count−1]` | Linear within each stored vertex interval |
| NURBS | Active knot interval | Rational B-spline |
| Polycurve | Independent outer segment intervals | Native leaf evaluation through affine parameter maps |

Reversal maps `[a,b]` to `[-b,-a]` and satisfies `reversed(-t) = original(t)`.
Similarity transforms retain native intervals even when physical length changes.
NURBS promotion for nonuniform affine deformation also retains the interval.
Morphs retain native source domains, although nonlinear geometry may be fitted.

Angle-based drafting helpers remain explicit: circles/ellipses provide
`point_at_angle`, and arcs provide normalized `point_at`. A line's `point_at`
also uses normalized coordinates and permits extrapolation; `evaluate` checks
the native interval. These are geometric construction helpers, not alternative
sampling conventions.

## Conversion and numerical policy

`to_nurbs` retains the native interval. Converting a circle or arc preserves its
exact locus but changes angular to rational parameterization: equal numeric
parameters generally do not identify equal interior points. Ellipse evaluation
already uses its rational parameterization and agrees with its NURBS form.
The [`ToNURBS` command](commands/to-nurbs.md) preserves polyline vertex parameters.
The previous native chord-length reparameterization policy was incorrect.
`nurbs_parameter` and `parameter_from_nurbs` provide the checked correspondence
between the two parameterizations, including independent polycurve leaf domains.
See [native curve cutting](curve-cutting.md) for numerical policy and oracle evidence.

Analytic derivatives use oriented frames, not world-space subtraction from a
rounded evaluated point. Analytic and polyline tangents do not divide by parameter-span width, so an
extremely small interval need not invalidate a well-defined direction. Requesting
a first derivative does not require a representable second derivative.
Line/arc/circle/polyline spans have direct arc-length inversion, including inside
polycurves. Rational and elliptical spans use controlled numerical integration.

The `curve/arc_length` module owns span lengths, repeated-query prefix tables,
distance-to-parameter inversion, and one-sided kink samples. Standalone NURBS
curves use a checked `[0,1]` integration copy, shared with the preparation for
[full-curve length](nurbs-numerics.md#arc-length-integration). Points and tangents
are evaluated in that internal frame; parameters are returned in the original
native domain. Input native parameters are converted before querying distance.
Already unit-domain NURBS do not allocate a normalization copy.

Standalone polylines also use a temporary `[0,1]` frame for distance sampling.
Their vertices are copied exactly, without revalidating short segments against
a different model tolerance. This prevents intermediate division points from
rounding to segment endpoints solely because the original domain is subnormal
or narrowly translated. Tests use exact L-shaped midpoints, native corner
parameters, and both standalone and composite polylines.

Polycurve sampling likewise normalizes the outer domain before mapping leaf
spans into it. Shared `polycurve/integration_frame` preparation also normalizes
each NURBS or polyline leaf using its checked frame. The temporary copy retains native
segment classes, controls, weights, and independent parameterizations; it does
not merge them into one NURBS or edit junction endpoints. Already prepared
composites are borrowed without allocating another copy.
This prevents a valid multi-span leaf from disappearing into an outer interval
with no representable interior parameter. Cached and uncached circle samples
are checked against analytic points, and piecewise-line corner tests verify
one-sided tangents and native junction parameters. Arch regressions additionally
cover adjacent-float, subnormal, tiny, and huge independent NURBS leaf domains.
Unrepresentable relative span sizes and extreme analytic leaf domains remain
separate limitations.

Preparation checks normalized outer intervals before copying leaf geometry.
If distinct breaks collapse, it reports a numerical integration failure rather
than classifying the unchanged source as an invalid polycurve. NURBS leaf
checks also reject lost intervals even when the resulting repeated knots would
still form a valid NURBS representation; accepting that representation would
silently discard part of the source. Regressions cover both rejection paths
and verify that source geometry remains unchanged.

Repeated-query tables can integrate to a slightly different total than the
original span estimate. Both query directions use the original span's distance
scale: a prefix integral is multiplied by `span_length / table_length`, while
inversion maps its target and tolerance in the opposite direction. This keeps
cached distance queries and inversion consistent without changing the reported
total length. Regressions cover all prefix nodes and the analytic midpoint of a
symmetric arch at tiny, unit, and huge geometry scales, using a loose tolerance
that exposes the independently rounded integration totals.

Lookup preparation requires positive subdivisions and allows at most 1,048,576
nodes across all variable-speed spans, including each span's two endpoints.
This bounds node storage to 16 MiB per prepared table set (excluding container
metadata). Checked count arithmetic rejects overflow and over-budget requests
before allocating nodes. Replacement tables are staged, so rejection or an
integration error leaves an existing cache usable. Budget tests exercise limits
without allocating maximum-size tables, and verify retained sample results
after rejected replacement requests.

Fitting, rebuilding, refitted tweening, and sweep sampling use budget-aware
preparation because the cache is optional. They lower the preferred subdivision
count to fit the aggregate cap; if even two nodes per variable-speed span do
not fit, they retain uncached integration. Thus cache capacity alone does not
reject these operations. Genuine numerical integration errors still propagate.
The strict preparation API remains available for explicit-density requests.

An interval with only two representable floats cannot encode an interior native
parameter. In such domains sampled geometry remains accurate, but the returned
parameter rounds to a source-domain value and re-evaluating it can produce a
different point. Native parameter/distance roundtrips are tested on well-resolved
tiny and huge domains, not promised beyond floating-point resolution. The
normalization applies to standalone NURBS/polylines, polycurve outer domains,
and NURBS/polyline leaves. Extremely disparate relative span sizes remain
unresolved, as do extreme analytic leaf and ellipse parameter scales.

Analytic and polyline intervals must have finite positive width. A standalone
circle needs a representable default circumference interval; an arc may still
use a larger supporting circle when its own interval is representable. Supporting
circle frames are separate from complete circular curves. Existing NURBS evaluation
and normalized parameter mapping retain support for wider finite-endpoint domains.

Polycurve NURBS conversion matches adjacent homogeneous scales when every resulting
weight is representable. Coincident endpoints share their midpoint and a degree-multiple
junction knot, matching OpenNURBS within the fixed curve-coincidence tolerance.
If a scaled weight would overflow or vanish, full-order knots preserve independent
scales. The multiplication/division order avoids forming overflowing weight ratios.
NURBS seam relocation and wrapped subcurves share that rescaling policy.
[Rational numerical policy](nurbs-numerics.md) describes local-coordinate evaluation,
nonzero degree-one rational acceleration, and within-span floating-point limits.

Native [trimming, splitting, seam relocation, and command reparameterization](curve-domain-editing.md)
retain this parameter contract. NURBS affine maps preserve exact interior knots when
representable, preventing collapsed trim slivers caused by needless normalization.

## Evidence

`curve_native_parameters.json` contains 38 Rhino comparisons covering all seven
curve families: native intervals, points, derivatives, tangents, equal-length
division parameters, rational definitions, reversal, reparameterization, and transforms.
The observed maximum numeric difference is below `4.8e-11`, with comparison limits
`1e-8` absolute and `1e-10` relative. Exact shears explicitly convert Rhino's curves
to deformable NURBS before transforming; direct analytic shear has different behavior.

Unit tests additionally cover large translations, tiny parameter spans, supporting
circle overflow, wide NURBS intervals, full-domain ellipse jets, and command-level
Flip/Scale/ToNURBS/export domain preservation. This is bounded compatibility evidence,
not proof of equivalence for every curve or numerical extreme.
