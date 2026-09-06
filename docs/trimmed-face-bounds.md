# Tight trimmed-face and B-rep bounds

[Trim images](trim-boundary-bounds.md) · [Surface bounds](surface-bounds.md) · [Arrays](plane-arrays.md) · [Oracle](oracle.md)

`BrepFace::tight_bounds(Tolerance)` bounds the complete retained surface,
including interior extrema and holes. `Brep::tight_bounds(Tolerance)` unions
faces with one shared resource budget. `Geometry::tight_bounds` dispatches
curves, surfaces, and B-reps through their corresponding kernels; vertex-based
geometry uses its exact vertex bounds.

Array Fill uses this query in construction-plane coordinates; nonrotating
ArrayPolar uses its world-space center. Failure leaves geometry, selection,
and undo history unchanged. The inexpensive display/control `bounds()` methods
and the BoundingBox command remain separate and unchanged.

## Interior search

The search starts with the [exact surface images of all trims](trim-boundary-bounds.md),
not fitted shared edges. Boundary enclosures and attained witnesses remain
separate, so chaining queries does not accumulate their tolerance errors.

Each original surface-knot rectangle is a homogeneous rational Bézier patch.
For a coordinate `X/W`, the sign of a partial derivative is the sign of
`X' W - X W'`. Centered numerator controls and conservative product ranges,
with floating-point cancellation guards, can establish a strict sign. That
rules out the corresponding interior extremum. Original patch sides remain
eligible when the derivative points toward them: C0 ridges and independent
full-order knot limits must not be mistaken for artificial subdivision edges.
Inconclusive signs never discard candidates.

Remaining patches are subdivided until their relevant coordinate hulls lie
within tolerance of attained geometry. Points are evaluated on homogeneous
patches, avoiding loss of continuous geometry when native UV-domain endpoints
are adjacent representable numbers.

## Conservative trim regions

`bounds/trim_region` classifies rectangles as inside, outside, or uncertain.
It uses even/odd membership in the outer loop minus the union of inner-loop
interiors. A regular same-sign rational Bézier curve lies in its control hull.
If that hull is disjoint from the query rectangle, replacing the curve by its
chord cannot change membership there. Entirely right-hand hulls contribute ray
parity using endpoint Y signs; left/above/below hulls contribute nothing.
Overlapping hulls are refined and cached. No fixed polygon approximation or
model-tolerance merging can erase a narrow hole.

UV padding depends on extraction/subdivision depth, degree, input magnitude,
and denominator conditioning, not model or stored trim tolerances. Ambiguous
rectangles cannot be discarded, and ambiguous points cannot serve as interior
witnesses. Queries may refine up to 12 new curve nodes and inspect 256 nodes
before returning uncertain; later queries can reuse and further refine them.

## Failure policy and limits

The face must have a valid trim arrangement with closed, continuous UV loops.
Actual endpoint gaps and full-order trim jumps are rejected rather than
silently closed, even if general B-rep validation accepts them within tolerance.
Tiny differences between floating-point extractions of mathematically shared
span endpoints receive explicit connecting uncertainty boxes.

All faces, trim images, region tests, and surface refinement share the bounds
budget: 131,072 visited nodes, depth 64, 1,048,576 initial coefficients, and
33,554,432 estimated work units. The trim-image composition degree cap is 256.
Poles in retained geometry, unresolved ambiguity, collapsed normalized spans,
and exhausted budgets return errors. A pole strictly outside the retained face,
including inside a hole, need not invalidate its box.

This is tolerance-controlled floating-point refinement, not interval-certified
arithmetic. Some regular inputs can fail. In particular, a varying trim starting
exactly on a fully multiple surface jump can retain an unresolved branch in the
boundary-image query. A regression preserves that error instead of returning
an incorrect branch or a loose two-branch box.

## Verification

Rust tests cover disks, annuli, thin and tiny holes, concave polygons, oblique
quadratic extrema on every spatial axis, a C0 knot ridge, full-order knot
branches, signed/extreme weight gauges, spherical seams and singularities,
adjacent-representable UV domains, and a hole excluding an isolated rational
surface pole. Derivative signs are checked against independent rational surface
derivatives over degrees 1–5 by 1–4 and repeated tensor subdivision. Command
tests check placement, original underlying domains and trims, undo/redo, and
atomic failure for tolerance-closed input gaps.

`trimmed_brep_bounds.json` has 11 passing complete B-rep comparisons at absolute
`1e-8`, relative `1e-12`; observed maximum error is below `5.01e-10`.
Both workers time complete box queries after warmup, with surface-image witnesses
recorded separately. Wine/FEX and native timings vary by case; these small
measurements do not establish general performance parity.

`trimmed_brep_bounds_diagnostics.json` retains five discrepancies:

| Reference case | Retained discrepancy |
| --- | --- |
| Oblique paraboloid disk | Exact maximum Z is `4.0390625`; Rhino reports about `4.03125`. |
| Negative common surface gauge | Interior Z=0 is excluded by Rhino's minimum Z=0.64. |
| Unclamped multi-span paraboloid | Interior Z=0 is excluded by Rhino's minimum Z≈0.0004. |
| UV weights scaled by `1e-200` | Box agrees, but recorded trim-image samples differ by up to 0.797. |
| UV weights scaled by `1e200` | Box agrees, but recorded trim-image samples differ by up to 0.797. |

The first three include direct Rhino surface-point witnesses outside Rhino's
own boxes. The last two remain trim-evaluation diagnostics, not demonstrated
box failures. Native boxes also match independently derived quadratic extrema;
none of these cases is declared a passing reference or hidden by a larger epsilon.

## Actual arrays and remaining compatibility

`trimmed_brep_array_bounds.json` retains 32 real Rhino command cases: disk,
annulus, capped disk, and oblique disk on Top, Front, Right, and oblique planes.
Each records original/copy identity, selection, groups, every native face
domain, 25 underlying-surface samples per face, and nine image samples per trim.

13 nonrotating polar cases pass at absolute `1e-8`, relative `1e-12`.
The other 19 cases remain explicit failures:

| Cases | Count | Largest placement difference |
| --- | ---: | ---: |
| Ordinary disk/annulus/cap Fill on world planes | 9 | `2.39e-8` |
| Oblique disk Fill on world planes | 3 | `0.001563` |
| Fill on the oblique plane | 4 | `0.000358` |
| Oblique disk polar on Front/Right/oblique planes | 3 | `0.001563` |

Original geometry samples agree within `1.12e-15`; residual variation after
removing each copy's translation is below `1.43e-14`. This distinguishes layout
differences from deformation or UV-parameter changes. The command's placement
does not establish which internal Rhino box algorithm it uses. Full Rhino
array compatibility is not claimed, and the comparison epsilon is unchanged.

A private-display release UI check imported a trimmed quadratic B-rep, ran
Front-plane Fill, undo/redo, and Right-plane nonrotating polar copies, then
exported 3DM. All six B-reps retained their exact UV loops and full underlying
surface domains; 441 samples per object matched analytic placements within
`3.56e-15`.
