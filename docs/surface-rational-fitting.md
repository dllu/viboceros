# Rational candidates for nonlinear surface fits

[Surface morphing](surface-morphing.md) · [B-rep morphing](brep-morphing.md) · [3DM validation](brep-3dm-interchange.md)

The surface fitter checks a rational composition candidate between its
mapped-control check and adaptive polynomial bicubic fallback. This reduces
unnecessary approximation of rational geometry that a nonlinear map preserves.
The point map is still arbitrary: the candidate is accepted only after the
existing independent Euclidean validation grid meets 80% of the requested
absolute tolerance.

## Candidate space

Write the source as `S = (X, Y, Z) / W`, where all four homogeneous components
are tensor polynomial splines of degrees `(p, q)`. A polynomial in XYZ of total
degree at most three, composed with S, has denominator `W³` and numerators of
degrees at most `(3p, 3q)`. This supplies a useful candidate space without having
to inspect or symbolically recognize the supplied point map.

The current optional candidate is limited to source degrees at most three in
each direction and nonconstant weights of one common sign. Constant weights
keep the ordinary polynomial path; mixed signs and higher degrees also retain
the existing fallback. Weights are normalized by the largest absolute source
weight, so common signs and scales do not change the candidate geometry.
Underflow that erases a normalized weight or its sampled cube rejects the
candidate rather than introducing a zero denominator.

The output knot multiplicity at an interior source knot of multiplicity m is
`3p - p + m` (and similarly in V). The active endpoints are clamped. Thus
structural source continuity and full-order independent limits remain represented;
periodic sources may become clamped, as in the existing fitter. Output degree is
at most nine. Either axis exceeding the existing 256-control ceiling skips the
candidate before any candidate point-map calls.

`morph/denominator` builds the scalar source weight function.
`spline_collocation/axis` constructs the candidate knot vectors and sided
Greville stations. `surface_fit/tensor` interpolates the centered homogeneous
targets `((mapped_point - origin) W³, W³)`, solving U and V independently.
Cubic axes keep the shared banded solver; higher-degree axes use bounded faer
full-pivot solves. Each solved weight must be finite and strictly positive,
and each dehomogenized control must be finite. Numerical construction failure
discards the optional candidate. Source mapping failures propagate, not retry
through an alternative point map. All paths share the original cache and its
one-million-sample ceiling.

The validation grid, native domains, and absolute tolerance are unchanged.
Rejected inaccurate candidates continue into adaptive bicubic refinement.
Passing finite samples is not a continuous error or self-intersection certificate;
arbitrary maps can have unsampled features, and extreme ranges can still fail.

## Reproduced sphere limit and fix

The regression uses a radius-0.4 sphere and the cubic lift
`(x, y, z) → (x, y, z + x² + xy/4 + y³)`. A B-rep absolute tolerance of `1e-6`
allocates `2.5e-7` to its underlying surface fit.

The prior polynomial fitter reached 73×37 controls and approximately 614,000
cached samples with error `1.61e-6`. Its next 137×67 grid exhausted one million
samples. An investigated bicubic candidate retaining only W reduced the error
but still exhausted the budget; that experimental path was not retained.

The implemented W³ candidate uses 25×13 degree-six controls and 2,238 cached
source samples. Its validation error is about `5.7e-16`. The regression requires
fewer than 5,000 total point-map calls (including mapped controls), checks an
offset grid at `2e-12`, and checks first/second partials against an independently
differentiated lift at `5e-11`, including all sides of source knot intersections.
These are case-specific numerical observations, not a general performance claim.

Additional tests cover nonseparable weights, common signs and scales, reproduction
of W³, four limits at crossing positional jumps, exact large constant targets,
map failure propagation, candidate limits, and rejection of a non-cubic wave
followed by successful adaptive fitting.

The tighter fits also exercised an existing UV-evaluation defect in conforming
meshing: an exactly constant trim coordinate `0.4` could evaluate just outside
the surface domain. The [local-coordinate UV evaluator](nurbs-numerics.md) fixes
that arithmetic, without clamping invalid inputs or changing domain tolerance.
Cylinder and cone meshes retain their trims and shared boundaries, and the
sphere's 3DM fixture advanced from its temporary `1e-5` to `1e-6`. The subsequent
[rational curve candidate](curve-rational-fitting.md) removes the next edge-fit
bottleneck, and the fixture now requires `1e-11`.

The surface-fitting checkpoint passed 1,339 Rust tests, 29 Python tests and strict
Clippy. All 120 native fixtures (1,210 operations), 369 recorded curve comparisons,
and the existing curve/surface/B-rep morph, surface-jet and mesh comparisons retain
their documented tolerances. Eight fresh Rhino 3DM comparisons pass at absolute
`2e-12`, relative `1e-14`, with maximum numeric difference `5.33e-15`.
