# Native curve domain editing

[Parameters](curve-parameters.md) · [Curve commands](commands/curves.md)

`Curve3` provides native-domain `try_trimmed`, `try_subcurve`,
`try_split_at_parameters`, and `try_change_closed_seam`. `CurveRef::closest_parameter`
returns a parameter on the original curve: circular curves use analytic projection,
and composites inspect their native leaves. Point-based edits therefore avoid an
angular-to-rational parameter change before trimming.

Increasing trims retain lines, arcs, polylines, NURBS, and polycurve leaves. Partial
circles become arcs; ellipses use their parameter-equivalent rational form.
Decreasing open subcurves reverse the trimmed portion. Decreasing closed subcurves
cross the original seam and become polycurves containing the two trimmed portions,
as in Rhino. A multi-split traverses sorted, distinct interior stations cyclically
for closed curves; one closed station retains both seam pieces in a full-loop result.

Seam relocation differs from splitting: it rotates an analytic circle or polyline
without making a composite. Existing polycurves retain their native leaves; smooth
periodic NURBS retain periodic topology when possible (Rhino uses clamped output
for out-of-domain periodic seam requests). Finite out-of-domain seam
parameters wrap by a period, and the new interval still starts at the requested value.
Polyline seams within eight machine epsilons of a vertex avoid adding sliver segments.

`SubCrv`, point/parameter `Split`, `CrvSeam`, and `Reparameterize` use these native
operations. Object attributes, source groups, selection policies, and undo remain
document operations. [Cutting-object commands](curve-cutting.md) use span-aware
native/rational parameter correspondence and preserve the source representation,
including across a closed seam; their output policy differs from low-level split APIs.

## Numerical safeguards

Affine knot mapping first uses a representable direct map, with scaled arithmetic
for wide or extreme domains. Reparameterizing to the same interval is a no-op.
This preserves exact integer/midpoint knots and prevents zero-derivative trim slivers.
Explicitly integer-uniform curve-through construction creates integer knots directly.

NURBS closest-point searches use bounded multi-start refinement with the exact
squared-distance curvature term, clamping, and monotone backtracking. A
projected-tangent step remains available when the Hessian is nonpositive,
curvature is unrepresentable, or Newton backtracking fails. Derivatives are
scaled before products to avoid squaring a large or tiny speed. This fixes a
near-boundary sweep case where tangent-only steps oscillated through the entire
iteration budget. Independent tests use a parabola with a unique analytically
known minimum and domains of length `1e±200`, including finite first derivatives
with unrepresentable second derivatives. These searches are not a certified
global minimizer on arbitrary curves.

Ellipse NURBS controls use exact quadrant and corner frame coordinates. Arc conversion
uses Rhino's one-, two-, or four-span rational layout, including four spans above 180°.
Polycurve conversion averages only coincident junction endpoints and matches homogeneous
scales when all weights remain representable; otherwise independent full-order seams
retain the geometry. See [conversion policy](polycurves.md).

## Validation

`curve_native_editing.json` contains 86 Rhino comparisons exercising native trims, directed and cyclic subcurves,
splits, seam relocation, reversed sources, periodic curves, and unequal rational
seam weights. The oracle compares representation, native points and derivatives,
length, rational control definitions, and parameter-bearing division stations.
The observed maximum geometry/derivative/control difference is `1.3e-11`; the
maximum difference including division stations is `9.7e-8`.

Rhino's polycurve equal-length inversion can differ from a high-accuracy result even
when its requested fractional tolerance is `1e-12`. An independent 40-digit rational
de Boor evaluation and arc-length quadrature checked three wrapped-ellipse stations:
Viboceros' fractional residuals were below `1.1e-16`, while Rhino's were approximately
`3.4e-9` to `4.9e-9`. The full record comparison uses `1e-7` absolute and `1e-10`
relative limits; a separate geometry/derivative/control comparison retains `1e-8`
absolute. This does not lower the kernel's integration accuracy.

Unit tests cover exact knots, small circles, near-vertex seams, overflowing weight
ratios, genuinely unrepresentable weight rescaling, invalid edits, command metadata,
and viewport-driven edits. Full Rhino command and numerical parity remain incomplete.
