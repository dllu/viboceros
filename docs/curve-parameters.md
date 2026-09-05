# Native curve parameters

[Geometry architecture](architecture.md) · [Polycurves](polycurves.md)

`CurveRef` exposes one checked native-domain contract for `domain`, `parameter_at`,
`evaluate`, first/second derivatives, tangents, and arc-length sampling. `Curve3`
owns the same representations; `try_reparameterized` changes their intervals
without changing the locus. `CurveSample::parameter()` belongs to the source
curve's native interval, including negative reversed intervals.

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
The `ToNURBS` command's chord-length reparameterization of polylines remains an
explicit command policy, separate from native NURBS conversion.

Analytic derivatives use oriented frames, not world-space subtraction from a
rounded evaluated point. Tangents do not divide by parameter-span width, so an
extremely small interval need not invalidate a well-defined direction. Requesting
a first derivative does not require a representable second derivative.
Line/arc/circle/polyline spans have direct arc-length inversion, including inside
polycurves. Rational and elliptical spans use controlled numerical integration.

Analytic and polyline intervals must have finite positive width. A standalone
circle needs a representable default circumference interval; an arc may still
use a larger supporting circle when its own interval is representable. Supporting
circle frames are separate from complete circular curves. Existing NURBS evaluation
and normalized parameter mapping retain support for wider finite-endpoint domains.

Polycurve NURBS conversion shares identical homogeneous endpoint controls with
degree-multiple junction knots. Otherwise it retains full-order knots and independent
weights: no endpoint averaging or potentially overflowing ratio of unrelated weights.
General minimal-knot simplification remains incomplete.

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
