# Loft

[Surface commands](commands/surfaces.md) · [Architecture](architecture.md) · [Oracle](oracle.md)

```text
Loft [Type=Normal|Loose|Tight|Straight|Uniform] [Closed=Yes|No]
     [SplitAtTangents=Yes|No] [Natural] [StartPoint=x,y,z] [EndPoint=x,y,z]
     [Rebuild=0..512]
```

Select at least two curves, or one with a point end; `Closed=Yes` requires three.
Defaults are `Normal`
and `Closed=No`. The result is one B-rep on the current layer, with fresh
attributes, no groups, and no selection. Source objects, attributes, groups,
and selection remain intact. The command is one atomic undo/redo operation.
Unsupported options and invalid geometry fail before document mutation.

[Section rebuilding](loft-rebuild.md) constructs clamped profiles through the
shared arc-length rebuild before lofting.

Optional [point ends](loft-points.md) taper either side to a pole. A single curve
suffices with a point end; Uniform and `Closed=Yes` do not accept point ends.

Open profiles retain selection order. Parallel planar closed profiles are sorted
by plane offset along the first profile's oriented area normal; opposite profile
directions are aligned by consecutive seam tangents. Each later seam moves to
the knot nearest the preceding seam; `Natural` retains the supplied seams while
still aligning traversal. General spatial seam matching remains incomplete.

`CreaseSplitting` controls tangent-break partitioning at 0.1 degrees.
`SplitAtTangents=Yes` additionally splits curvature breaks at 1 degree,
independently of that global preference; its default is `No`. Both create exact
shared-topology patches without changing the tensor geometry. Preferences live
outside document undo; undo/redo restores the Loft result with its original splits.

Open lofts retain the established UV-direction policy. Closed solid lofts
reverse face sense when all six outside closest-point probes point inward;
conflicting or unsupported probes retain the supplied sense. This sampled policy
does not certify outward orientation, particularly for self-overlapping lofts.

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
`Brep::try_loft` adds the default command policies. `LoftOptions` and
`Brep::try_loft_with_surface_options` expose the seam and splitting controls
with a delegated tensor backend. `brep/loft/profiles` owns section ordering and
seams; `brep/loft/orientation` owns the sampled closed-solid orientation pass.
Smooth, fully multiple circle knots stay intact when only crease splitting is on.
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

Not implemented: general spatial profile/seam matching,
interactive seam editing, start/end tangent matching, and Refit simplification. Nonparallel or nonplanar closed sections currently keep input
order. Native parameter preservation
of analytic input curves is not promised: loft deliberately builds a normalized
compatible basis. Nearly symmetric profiles can select a different boundary from Rhino when
coarse length comparisons tie at machine precision. The command does not cap its ends.

API timing measures repeated tensor construction only. Command timing includes
document setup, execution, recording, and cleanup. One small-fixture run took
0.05–0.67 ms per native command; Rhino runs through Wine/FEX, so this is not
an original-speed Rhino benchmark or a general kernel performance guarantee.
