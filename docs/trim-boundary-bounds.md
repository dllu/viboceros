# Bounds of exact surface trim images

[Surface bounds](surface-bounds.md) · [B-rep validation](brep-validation.md) · [Oracle](oracle.md)

`NurbsSurface::parameter_curve_bounds(&NurbsCurve2, Tolerance)` bounds the
complete spatial image `S(u(t), v(t))` of a parameter-space curve.
`BrepFace::trim_boundary_bounds(Tolerance)` unions these images over every
outer and inner loop, reusing surface-span extraction and one resource budget.
Neither query fits a spatial edge, samples a display mesh, or substitutes the
UV control polygon for its surface image.

These are **boundary bounds, not complete trimmed-face bounds**. A paraboloid
disk of radius 0.8 has boundary Z = 0.64 but reaches Z = 0 at its interior
center. A unit test explicitly preserves this distinction. The separate
[trimmed-face query](trimmed-face-bounds.md) adds trim-aware interior extrema
and supplies complete B-rep bounds for array layouts.

## Homogeneous composition

The `bounds/bezier/compose` module composes a rational tensor Bernstein patch
of degree `(p,q)` with a rational UV curve of degree `m`. Its result is a
homogeneous curve of degree `m*(p+q)`. Bernstein polynomial products include
their binomial factors; compensated accumulation reduces cancellation. All
four coordinates remain homogeneous, including intermediate zero weights.
There is no interpolation or least-squares approximation.

`bounds/parameter_curves` normalizes native UV domains and prepares original
surface-knot rectangles. A UV span contained in one rectangle is composed
directly, then bounded by the shared [rational subdivision kernel](curve-bounds.md).
At knot crossings, neighboring polynomial extensions provide conservative
candidate hulls. They are accepted only within tolerance of attained image
coordinates; otherwise the UV span is subdivided. Extrapolated values are not
treated as attained points. Non-dyadic crossings do not require snapping trim
parameters to fitted knot intersections.

Sampling for refinement uses the homogeneous composition itself, not rounded
native UV coordinates. This retains continuous image geometry even when a UV
domain has adjacent representable endpoints. Constant UV coordinates are kept
exactly, including curves on or within a few ULPs of fully multiple knots. A
curve lying on a knot uses the native right-hand patch (left at domain end).
Near a fully multiple knot, uncertain nonconstant UV samples are not used to
choose an attained spatial branch, and both neighboring hulls remain eligible.
This can reject a nearly tangent image rather than invent a distant branch
from sub-ULP rounding. Exact constant-coordinate curves retain their native side.

## Limits and failure policy

The shared budget allows 131,072 visited nodes, depth 64, 1,048,576 initial
coefficients, and 33,554,432 estimated work units. Composition degree is capped
at 256 before polynomial allocation. A face's loops share these limits rather
than receiving independent unbounded allowances. Normalization that collapses
a nonempty surface span is rejected.

UV curves must remain in the active surface domain. Even a tiny detected
outward excursion is an error, not permission to clamp the input to a different
curve. Poles on the image, normalization underflow, unresolved hulls, and
exhausted budgets return errors. A surface pole elsewhere does not invalidate
a regular image that avoids it. This remains floating-point,
tolerance-controlled refinement, not interval-certified arithmetic; difficult
but regular inputs can be rejected.

## Verification

Independent Rust tests compare composition against direct tensor evaluation
for surface degrees 1–5 by 1–4 and UV degrees 1–4. Other tests cover quadratic
interior extrema along diagonal trims, rational circles on paraboloids,
negative/extreme common gauges, non-dyadic knot crossings, fully multiple
surface knots, very narrow UV domains, holes, true poles, out-of-domain trims,
huge off-domain controls whose curve remains in-domain, spherical seams and
singular poles, and resource limits. Boundary-only bounds are never asserted to cover a face's
interior.
A regression matrix perturbs rational weights one ULP across analytically
known tangencies to a surface with a 10-unit jump. A successful query must
include exactly the realized branches; unresolved cases must return an error.

`parameter_curve_bounds.json` contains 20 passing Rhino comparisons, including
an oblique paraboloid, an unclamped multi-span paraboloid, signed rational
surfaces, and a regular curve avoiding a surface pole. Maximum observed error
is `7.35e-10` at absolute `1e-8`, relative `1e-12`.
`trim_boundary_bounds.json` adds six exact disk/annulus/capped/reversed/rotated
B-reps; maximum observed error is `3.56e-15`.

The reference curves are independently supplied analytic constructions, not
curves fitted by the new implementation. Rhino bounds those spatial curves or
shared B-rep edges. Both workers additionally compare 65 surface-image samples
per trim; the parameter-image probe also verifies and records 65 samples on its
spatial reference curve. Because these are different algorithms, both probes
deliberately report zero elapsed time rather than imply a composition-speed
comparison with Rhino.

`parameter_curve_bounds_diagnostics.json` preserves a signed-rational reference
whose Rhino box is too small. Its X coordinate is
`(-0.2*t + 0.6*t²)/(1 - 4*t + 5*t²)` and Z is X². The exact X range is
`0.1 ± 0.1*sqrt(2)`, but the tested Rhino box reports `[0, 0.2]`; the native box
agrees with the analytic extrema. This is a retained diagnostic, not a passing
reference or a reason to widen the comparison epsilon.
Rhino's recorded reference-curve samples also lie more than 0.04 outside its
own box, while agreeing with its surface-image samples within `1e-12`.
