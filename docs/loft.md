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

`loft/compatible` constructs a shared normalized basis through exact clamping,
degree elevation and knot insertion. It then applies Rhino's endpoint-weight
normalization policy. **Unequal endpoint-weight ratios can move later profiles:**
after normalization, the first section's knots are reused for all sections.
Consequently this policy is not unconditional interpolation of the original
curves, even for interpolating loft styles. Tests explicitly demonstrate this
displacement instead of assuming shape preservation.

`loft/interpolate` solves all homogeneous control channels together with faer's
full-pivot LU. U runs through sections; V follows each compatible section.

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

Permanent fixtures contain 34 API loft cases, 33 command cases, and four
same-file 3DM interchange cases. Exploratory comparisons cover 90 API cases:
two through six sections, all five styles, closed loops, rational circles,
mixed degrees and knots, polylines, common weight scales, and unequal endpoint
weights. On the tested Rhino 8.32 build, all 90 API and 33 command cases match at
`1e-9 + 1e-12 * max(|a|, |b|)`; largest numeric differences are respectively
`1.25e-13` and `7.11e-15`. Command records include face directions, validity,
edge/vertex counts, and document state. Faces are compared in UV-domain order,
not implementation-specific allocation order.

A combined live run of the 34 permanent API cases, all 33 commands, four loft
3DM cases, and eight existing morphed 3DM cases also passes the stricter
`2e-12 + 1e-14 * max(|a|, |b|)` bound. Both readers validate and mesh the same
exported loft B-reps. Periodic 3DM records canonicalize only the two outer knot
entries that OpenNURBS does not store; all stored coefficients remain checked.

```sh
tools/rhino_oracle/run_headless.sh compare tools/rhino_oracle/fixtures/loft.json \
  --absolute-epsilon 1e-9 --relative-epsilon 1e-12
tools/rhino_oracle/run_headless.sh compare tools/rhino_oracle/fixtures/loft_command.json \
  --absolute-epsilon 1e-9 --relative-epsilon 1e-12
tools/rhino_oracle/run_headless.sh compare tools/rhino_oracle/fixtures/loft_3dm_interchange.json \
  --absolute-epsilon 2e-12 --relative-epsilon 1e-14
```

Limits are 256 sections, 512 compatible controls per section, and 4096 grid
faces. Finite-input checks and resource limits precede large fitting allocations.
Common weight scales and local-coordinate solves avoid avoidable reciprocal
overflow and world-offset cancellation. Coincident consecutive compatible
control polygons are rejected. No general self-intersection, denominator-pole,
or continuous approximation certificate is claimed.

Not implemented: point-end lofts, general spatial profile/seam matching,
interactive seam editing, start/end tangent matching, Rebuild/Refit simplification,
`SplitAtTangents`, and configurable global `CreaseSplitting`. Nonparallel or
nonplanar closed sections currently keep input order. Native parameter preservation
of analytic input curves is not promised: loft deliberately builds a normalized
compatible basis. The command does not cap its ends.

API timing measures repeated tensor construction only. Command timing includes
document setup, execution, recording, and cleanup. One small-fixture run took
0.05–0.67 ms per native command; Rhino runs through Wine/FEX, so this is not
an original-speed Rhino benchmark or a general kernel performance guarantee.
