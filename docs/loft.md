# Loft

[Surface commands](commands/surfaces.md) · [Architecture](architecture.md) · [Oracle](oracle.md)

```text
Loft [Type=Normal|Loose|Tight|Straight|Uniform] [Closed=Yes|No]
```

Select at least two curves, or three for `Closed=Yes`. Defaults are `Normal`
and `Closed=No`. The result is one B-rep on the current layer, with fresh
attributes, no groups, and no selection. Source objects, attributes, groups,
and selection remain intact. The command is one atomic undo/redo operation.
Unsupported options and invalid geometry fail before document mutation.

Open profiles retain selection order. Parallel planar closed profiles are sorted
by plane offset along the first profile's oriented area normal; opposite profile
directions are aligned. Their existing seams are retained. Curved closed profiles
and smooth loft styles reverse the resulting surface's V direction. Rhino's
straight, degree-one polyline branch instead retains the bilinear patch direction.
These are observed command policies, not guarantees of outward orientation for
every possible input.

## Geometry and modules

`loft/parameterize` first matches curvature-discontinuous, full-multiplicity
section boundaries when raw knot vectors differ. It uses a coarse length estimate
and monotone boundary correspondence, then preserves relative knots inside each
matched region. Simple smooth knots retain their correspondence; changing them
piecewise would deform the profile. Matching degree/knot layouts take the existing
structural path, as do segmented inputs with positional jumps.

`loft/compatible` constructs a shared normalized basis through exact clamping,
degree elevation and knot insertion. It then normalizes endpoint weights through
the shared [projective normalizer](nurbs-numerics.md#endpoint-weight-normalization),
without Rhino's near-equal-weight approximation.
Linux `native-smlib` builds now delegate those exact section edits to usd-brep
while sharing the same weight and interpolation policy. See
[native section matching](smlib-section-bases.md).
**Unequal endpoint-weight ratios can move later profiles:**
after normalization, the first section's knots are reused for all sections.
Consequently this policy is not unconditional interpolation of the original
curves, even for interpolating loft styles. Tests explicitly demonstrate this
displacement instead of assuming shape preservation.

`loft/interpolate` shares the homogeneous targets and section policy across
backends. Portable builds use faer's full-pivot LU; `native-smlib` uses
[usd-brep basis evaluation and pivoted LU](smlib-loft-interpolation.md), factoring
once for all channels. U runs through sections; V follows each compatible section.

| Style | Section spacing | U construction |
| --- | --- | --- |
| Normal | Maximum corresponding-control Euclidean chord | Cubic interpolation |
| Tight | Square root of that chord | Cubic interpolation |
| Uniform | Uniform | Cubic interpolation |
| Straight | Maximum corresponding-control chord | Degree-one ruled spans |
| Loose | Uniform | Compatible controls become the loft controls |

Open cubic end handles come from the parabola through the first/last three
sections in the complete homogeneous control space. This endpoint metric is
distinct from the knot-spacing metric. Two sections use linear homogeneous
handles. Closed interpolating styles use periodic cubic systems; closed Loose
is cubic even with three sections. Open Loose uses degree `min(3, n-1)`.
Final domains use the longest Euclidean control polygons in each direction,
matching the tested Rhino surface-size policy.

The ordered `try_loft_nurbs_curves` API does not sort, reverse, or move seams.
`Brep::try_loft` adds the command policies and splits sampled isocurve-tangent
kinks larger than 0.1 degrees. Smooth, fully multiple circle knots stay intact.
The tangent predicate samples transverse span endpoints and midpoints, like
OpenNURBS; it is not a continuous maximum-angle certificate.

`brep/surface_grid` partitions tensor surfaces exactly at ordered native UV
parameters. Neighboring patches share vertices and isocurve edges; closed seams
and singular sides retain their topology. Unrelated coincident points are not
welded. Full-order positional breaks are rejected, not sewn across jumps.
Every constructed B-rep passes the existing topology and edge/trim validator.
Tests also exercise open grids and split spheres with sewn seams and poles.

## Validation and limits

The [validation record](loft-validation.md) describes native fixtures, replay
commands, sampled error bounds, and the remaining native comparison gaps.

Limits are 256 sections, 512 compatible controls per section, 65,536 attempted
length-integration steps per preparation, and 4096 grid faces. Finite-input checks and resource limits precede large fitting allocations.
Common weight scales and local-coordinate solves avoid avoidable reciprocal
overflow and world-offset cancellation. Coincident consecutive compatible
control polygons are rejected. No general self-intersection, denominator-pole,
or continuous approximation certificate is claimed.

Not implemented: point-end lofts, general spatial profile/seam matching,
interactive seam editing, start/end tangent matching, Rebuild/Refit simplification,
`SplitAtTangents`, and configurable global `CreaseSplitting`. Nonparallel or
nonplanar closed sections currently keep input order. Native parameter preservation
of analytic input curves is not promised: loft deliberately builds a normalized
compatible basis. Nearly symmetric profiles can select a different boundary from Rhino when
coarse length comparisons tie at machine precision. The command does not cap its ends.

API timing measures repeated tensor construction only. Command timing includes
document setup, execution, recording, and cleanup. One small-fixture run took
0.05–0.67 ms per native command; Rhino runs through Wine/FEX, so this is not
an original-speed Rhino benchmark or a general kernel performance guarantee.
