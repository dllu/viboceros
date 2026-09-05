# Surface morphing

[Transforms](commands/transforms.md) · [Curve morphing](curve-morphing.md) · [Surface evaluation](surface-evaluation.md)

`PointMorph::morph_nurbs_surface` fits the nonlinear image of an untrimmed NURBS
surface using the supplied absolute tolerance. Moving its control points is only
a candidate: the fitter checks the resulting surface against mapped source
points before accepting it. This preserves the original rational net for affine
maps without treating control-point mapping as a general nonlinear solution.
The map need not be defined at control points outside the actual source surface.

## Fitting and numerical policy

The adaptive path constructs a non-rational bicubic tensor surface in the source's
native U/V domains. Source knot multiplicities retain their structural continuity
up to C2; full-order jumps have independently evaluated left/right limits in each
direction. Crossing jumps therefore have four independent point values, with no
averaging across the break. A periodic source can produce a clamped fitted net;
periodic control topology and exact derivative matching at the seam are not
promised.

`morph/surface_fit/tensor` interpolates mapped source points at sided Greville
parameters. It solves the two axis systems separately, using the shared cubic
banded solver in `morph/interpolation` and `faer` multiple-right-hand-side matrices.
World coordinates are centered before solving, with a finite fallback for
overflowing coordinate differences. Interpolated knot-intersection point values
are pinned exactly; constant large-coordinate targets retain their exact controls.

Each nonempty knot rectangle is checked on the tensor product of uniform and
cosine-spaced stations, including all sided boundary values. Fourth differences
of the uniform-grid residuals guide independent U/V bisection: an error in U
multiplied by a cubic function of V does not automatically require more V
controls. When those differences nearly vanish, all-grid residual variation
provides a fallback against uniform-grid aliasing. These are refinement
heuristics, not error bounds. Refinement targets 80% of the requested
tolerance. At exhausted control or parameter resolution, a result is accepted
only if its measured error meets the actual requested tolerance.

The public limits are 256 controls per axis (`MAX_MORPH_SURFACE_AXIS_CONTROLS`)
and one million cached point-map evaluations (`MAX_MORPH_SURFACE_SAMPLES`).
Cache keys include both parameters and both sides. Excessive initial structure,
exhausted sampling, and failure to converge have explicit errors; no knowingly
out-of-tolerance fitted surface is returned. Document in-place and copy operations
remain atomic on failure, including attributes, selection, groups, and history.

These finite checks are not a continuous error certificate for an arbitrary
black-box mapping. Unsampled sharp features, mapping-induced discontinuities,
ill-conditioned rational sources, or sub-resolution tolerances can defeat a fit.
The resource limits can reject otherwise valid complicated images.
[B-rep morph assembly](brep-morphing.md) now combines these fits with shared
edge fits and exact UV trims, then checks interior edge/trim correspondence.

## Validation

Native tests cover exact cubic images, quartic refinement in one or both
directions, retained affine rational nets, maps undefined at off-surface controls,
four limits at crossing positional jumps, periodic source seams, large-coordinate
constant targets, aliasing detection, and explicit resource failures.
`OrientOnSrf` tests additionally check surface interiors at document tolerance
and preserve copy/in-place group, selection, and undo/redo behavior.

`surface_surface_morph.json` contains five independent-construction oracle cases:
bilinear and rational sources, scaled/rotated placement with changed domains,
a warped target, and a periodic source. Each records 1,089 direct point-map samples
and 1,089 fitted-surface samples. Interior checking stations are offset from the
dyadic refinement grid, avoiding an apparently exact result caused by checking
only interpolation points. Every native fitted sample is checked against its
own direct map at the requested tolerance, separately from Rhino comparisons.

The Rhino probe uses [Surface.ToBrep](https://developer.rhino3d.com/api/rhinocommon/rhino.geometry.surface/tobrep?version=8.x)
and [SpaceMorph.Morph](https://mcneel.github.io/rhinocommon-api-docs/api/RhinoCommon/html/M_Rhino_Geometry_SpaceMorph_Morph.htm)
on that single-face B-rep, then evaluates its sole face in the original domains.
`SplopSpaceMorph` is configured with explicit `Tolerance`, `QuickPreview=false`,
and `PreserveStructure=false`. A separate diagnostic applying `Morph` directly
to raw NURBS surfaces produced errors up to `0.052` against its own point map;
that path is not used as the reference for tolerance-driven surface fitting.
The B-rep path removes that large discrepancy without relaxing the comparison
to accommodate it. All owned source, target, morph, and fitted objects are
disposed on success and failure.

On Rhino 8.32 under Wine/FEX, the five direct maps agree within `7.2e-15` per
coordinate (checked at absolute `2e-12`, relative `1e-12`). For the first three
cases requesting `1e-7`, native sampled Euclidean fit errors are below `4.2e-8`;
the last two request `1e-6` and stay below `5.0e-7`. Rhino's B-rep fit errors reach
`2.6e-6` against its own direct map, and the largest fitted-output coordinate
difference is `2.39e-6`. The fitted comparison therefore uses absolute `3e-6`,
relative `1e-12`, separately from the strict direct-map check:

```sh
tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/surface_surface_morph.json \
  --absolute-epsilon 3e-6 --relative-epsilon 1e-12
```

Illustrative release fit times range from roughly `2–17 ms` natively and
`2–91 ms` in this translated Rhino run. Neither engine wins every case; the
different achieved errors and B-rep-versus-surface work make these observations
unsuitable for claiming native-Rhino performance parity.

At the initial surface-fitting checkpoint, 1,301 Rust tests, 19 Python tests,
strict Clippy, and 117 native fixtures (1,193 operations) passed. See
[B-rep morphing](brep-morphing.md) for the subsequent combined regression run.
This is bounded evidence, not complete Rhino compatibility.
