# Contour curve parameters

The original five surface-domain failures are fixed. All 52 recorded captures
match ordered normalized samples, actual domains, names, layers, color sources,
source preservation and groups at 1e-9 absolute + 1e-12 relative epsilon.
The corpus contains 40 distinct recipes and three repeats of four off-axis
recipes. Domain comparison is strict throughout.

## Representation policy

Natural straight edges retain their original surface parameter interval,
including negative intervals after reversal. Straight rational edges become
affine degree-one curves over that interval, matching native samples. Keeping
the source weights would give different intermediate parameters despite an
identical line locus.

For a bilinear surface, fixed-U or fixed-V candidates come from the two opposite
weighted signed-distance equations. Both equations must share exactly the same
root. BigRational arithmetic on the finite binary64 coordinates and weights
certifies that equality. The fixed parameter is reconstructed in the original
UV domain; local knot-origin normalization is deliberately avoided here.

A cutting-plane isocurve exists when the straight output follows one of its
orthonormal axes. Its fixed parameter is the projection onto the other axis.
When source and plane isocurves compete, the captured native rule chooses the
smaller absolute fixed parameter, with a tie favoring the plane. The plane
representation uses signed unit-distance parameters measured from its origin
along the output's traversal direction. General planar lines also use these
signed unit-distance parameters.

This rule is inferred from observable Rhino 8.32 outputs. Eight independent
experiments shifted, negated and centered the fixed UV range, changed the
varying UV range and switched X/Y cutting directions. They distinguish this
rule from an on-axis-only policy. In particular, translating a fixed UV range
changes which representation wins without changing the surface's locus.
The original asymmetric height-0.5 case repeats identically in all three runs.

## Whole-curve admission

Line simplification uses a control-hull certificate. All rational weights must
have the same sign, and every control point must lie in the finite segment's
closed tolerance capsule. Squared distances and capsule comparisons use exact
dyadic rational arithmetic, avoiding cancellation, overflow and rounded
threshold decisions. Rational basis functions then place the entire curve
in that convex capsule. Continuity and the two endpoints cover every axial
position on the segment, bounding both directions of the locus comparison.

This conservative check rejects collinear overshoots and transverse lobes even
when the endpoints or midpoint look linear. It does not infer linearity from
sample stations. Ambiguous or unsupported representations retain the existing
intersection result. All parameter/geometry work completes before output
admission, preserving command transaction behavior.

## Evidence and limits

[Comparison](contour-domains-comparison.json) ·
[Provenance](contour-domains-provenance.json) ·
[Original 21-case replay](contour-command-comparison.json)

Raw type/control/knot captures, repeat captures and UV-ranking experiments live
in `contour-domains-capture-source`. The fixture/observation pair removes only
extra native instrumentation fields; geometry, domains and model metadata are
preserved. Historical failing reports and implementation bytes remain archived
under `contour-domains-prior-source` with their original source pins.

These measurements qualify the recorded surface families and parameter ranges.
Arbitrary trimmed B-reps, broader curved families, near-tie/extreme parameter
ranges, closed seams, hatches and performance parity remain open. Full Rhino
command and geometry compatibility is still in progress.

References: [McNeel Contour](https://docs.mcneel.com/rhino/8/help/en-us/commands/contour.htm),
[public Brep contour API](https://developer.rhino3d.com/api/RhinoCommon/html/M_Rhino_Geometry_Brep_CreateContourCurves.htm).
