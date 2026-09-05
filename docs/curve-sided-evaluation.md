# One-sided curve evaluation

[Native parameters](curve-parameters.md) · [Rational numerics](nurbs-numerics.md) · [Oracle](oracle.md)

`ParameterSide::Left` and `Right` select exact limits at a native knot or
junction. All seven `CurveRef` families support `evaluate_on_side`,
`evaluate_with_derivative_on_side`, `evaluate_with_second_derivative_on_side`,
and `evaluate_with_tangent_on_side`. The ordinary methods retain their right-side
default. At either domain endpoint both choices use the interior span, even for
closed curves; selecting a side does not wrap or extrapolate the parameter.
Nonfinite and out-of-domain parameters are rejected.
The same `ParameterSide` enum selects independent U/V limits for
[rational surfaces](surface-evaluation.md).

NURBS evaluation selects the appropriate nonempty knot span without inserting
knots, splitting, or replacing the parameter with a nearby floating-point value.
Polyline vertices select their incoming or outgoing edge. Ellipses retain their
four rational quarters: a quarter knot can have different second derivatives
despite a smooth geometric locus. Polycurves propagate the choice through the
outer parameter map to knots **inside** each leaf, not only between leaves.

Reversal swaps left and right, negates the first derivative, and retains the
second derivative at the corresponding negated parameter. One-sided first
derivatives and tangents do not require the second derivative to be representable.
Full-order NURBS knots can have distinct point limits; the point returned by a
one-sided jet belongs to its selected span.

## Stationary points and kink detection

A zero first derivative does not necessarily mean that no oriented tangent exists.
At such a point, NURBS tangent evaluation examines successive homogeneous
derivative polygons through the curve degree. If the first nonzero Euclidean
derivative has order `k`, the right tangent points along that derivative and
the left tangent acquires sign `(-1)^(k-1)`. The homogeneous denominator's sign
is retained. A locally constant span still returns a degeneracy error.

For example, `C(t)=(t-1)^3` pauses at `t=1` but continues with the same tangent;
`C(t)=(t-1)^2` reverses direction there. Actual first and second derivative
queries still return their mathematical zeros: only tangent evaluation takes
the limiting direction. This is not a general stationary-curvature solver.

Arc-length kink detection uses exact one-sided tangents and
`atan2(|T_left × T_right|, T_left · T_right)`. Comparing cosines loses small
angles: near zero, their difference from one is quadratic in the angle.
The angle test now resolves the tested `1e-8`-radian kink at a `1e-10` threshold.
`FitCrv` and refit/sample-based tweening consume these arc-length samples.
NURBS knot-removal and B-rep kink-angle checks also use the direct side evaluator
instead of allocating and refining two split curves merely to inspect tangents.
Existing callers that explicitly restrict candidates to degree-multiple knots
retain that restriction; this is not an implementation of every discontinuity search.

## Validation and compatibility boundaries

`curve_sided_evaluation.json` contains 45 Rhino comparisons covering all curve
families, rational degrees one through three, C0/C1/C2 joins, unclamped and
periodic inputs, composite leaf knots, reversed intervals, parameter origins
at `1e10`, and stationary quadratic endpoints. Its `sided_parameters` field
requests both limits at explicit native parameters.

Rhino's public [side-aware DerivativeAt API](https://mcneel.github.io/rhinocommon-api-docs/api/RhinoCommon/html/M_Rhino_Geometry_Curve_DerivativeAt_1.htm)
supplies point and derivative arrays. A nonzero first derivative supplies the
unit tangent. For a stationary point, the worker trims to the selected side and
asks Rhino's `TangentAt` at that endpoint, disposing the temporary curve afterward.
The four stationary cases also compare complete arc-length divisions; the other
cases use differential-only records to isolate evaluation from length inversion.

All 45 cases passed in the licensed Rhino 8 Wine/FEX installation at absolute
`1e-8`, relative `1e-10`; the observed maximum numeric difference was below
`2.6e-12`. Kernel tests independently cover analytic rational jets, positional
jumps, reversal identities, knots one representable parameter apart, tiny-angle
kinks, smooth stalls versus cusps, higher-order stationary endpoints through
degree six, negative global weight scales, and constant-span rejection.

```sh
tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/curve_sided_evaluation.json \
  --absolute-epsilon 1e-8 --relative-epsilon 1e-10
```

Internal full-order knot groups exceed OpenNURBS' allowed multiplicity. Rhino's
input builder rejected the positional-jump case, so full-order tests remain
native analytic tests, not passing single-NURBS Rhino references. The
[3DM cross-reader probe](curve-3dm-interchange.md) tests exported decompositions
of these representations instead. Periodic oracle inputs use
the protocol's duplicated artificial outer knots; those two entries are not
stored in OpenNURBS' shortened knot array.

Floating-point range and active-weight normalization limits in the
[rational numerical policy](nurbs-numerics.md) still apply. Stationary detection
uses exact computed zeros, not an arbitrary speed threshold. These sampled
comparisons and analytic tests do not prove equivalence for every curve.
