# Curve morphing

[Transforms](commands/transforms.md) · [Surface evaluation](surface-evaluation.md) · [Oracle](oracle.md)

`PointMorph` defines a deterministic point mapping. Its line, polyline, and
NURBS helpers share a tolerance-driven native-parameter cubic fitter in
`morph/curve_fit`. `morph_line` and `morph_polyline` explicitly take the document
tolerance, including when called on polycurve leaves. Native vertex parameters
are preserved; explicit polyline-to-NURBS conversion's chord-length map is not
used for fitting.

The initial knot vector retains source span boundaries and their guaranteed
continuity, up to the cubic approximation's C2 limit. Corners remain C0 joins,
C1 knots retain their acceleration breaks, and full-order positional jumps have
independent point limits. Endpoints and interpolated joins are fixed exactly to
the corresponding point-map values. Closed polyline seams are retained exactly.
There is no smoothing or averaging across a positional jump.

## Approximation and numerical policy

The fitter interpolates mapped source points at cubic Greville parameters.
It checks each nonempty span on both uniform and cosine-spaced grids, including
one-sided endpoints. The second grid catches, among other cases, a tested
oscillation that aliases both thirds-based interpolation and sixteenths-based
validation. Failing spans are bisected, with the worst sampled errors prioritized
when the remaining control budget is small.

Refinement normally targets 80% of the requested absolute tolerance, providing
some sampling headroom. At the work or parameter-resolution limit, a curve is
returned only if the measured error meets the actual requested tolerance.
Otherwise `CurveMorphDidNotConverge` reports the tolerance, measured deviation,
and control limit. Excessive initial structure returns
`TooManyMorphCurveControlPoints`. The public `MAX_MORPH_CURVE_CONTROL_POINTS`
limit is 512. The former behavior of returning a knowingly inaccurate fit when
the budget ran out has been removed.

The interpolation right-hand sides use a local world-space origin, with a
fallback when subtraction would overflow. This avoids solving for tiny shape
variations on top of a large common coordinate. Constant mappings retain their
exact controls, including the tested `(1e12, -2e12, 3e12)` case. Point-map values
are cached across refinements by parameter and side; opposite limits never share
a cache entry.

Cubic Greville rows have only two off-diagonals on each side. The specialized
banded solver uses linear factorization work and storage, with `faer` matrices
for right-hand sides. Its no-pivot elimination relies on B-spline collocation's
total nonnegativity; see [de Boor and Pinkus' analysis](https://cris.technion.ac.il/en/publications/backward-error-analysis-for-totally-positive-linear-systems/).
Nonpositive/nonfinite pivots and unrepresentable solutions are errors. Tests
compare the banded result with a full-pivot `faer` solve across twelve nonuniform
knot families with multiplicities one through four. This is not a general
banded-system solver.

For the largest tested warped cubic, the same `1e-9` fit took approximately
`35 ms` with the initial dense solver and `11 ms` after banded solving and
point-map caching; fitted coordinates changed by less than `2e-15`. These are
illustrative host timings, not a controlled native-Rhino benchmark. Comparing
runtime against Rhino's looser fitted output would mix accuracy and speed.

For a black-box mapping these finite samples are not a continuous error proof.
Unsampled sharp features can still evade detection; arbitrary discontinuities
introduced by the mapping itself are not located analytically. Extremely narrow
native spans, ill-conditioned rational sources, or tolerances below representable
coordinate resolution can prevent a fit. Surface helpers still move surface
controls; adaptive surface/B-rep morph fitting is not supplied by this module.

## Rhino validation

`curve_surface_morph.json` has eight public API cases: short and long lines,
scaled/rotated lines, native-parameter polylines, and cubic curves on rational
cylindrical and warped surfaces. It records 257 direct point-map samples and
257 fitted-curve samples per case. The native probe independently checks each
fitted sample against its own direct map at the requested fitting tolerance.
Rhino uses [SplopSpaceMorph](https://developer.rhino3d.com/api/rhinocommon/rhino.geometry.morphs.splopspacemorph/splopspacemorph)
with `QuickPreview=false`, `PreserveStructure=false`, and explicit `Tolerance`.
Construction and output sampling are outside the fit timing loops.

In Rhino 8.32 under Wine/FEX, all eight direct point maps agreed within
`3.6e-15` per coordinate; a separate comparison passes at absolute `2e-12`,
relative `1e-12`. Seven cases request `1e-9` fit tolerance: their native sampled
Euclidean errors stay below `7.5e-10`, whereas Rhino's fitted curves have sampled
errors up to `9.9e-6` against its own point map. The eighth case requests `1e-3`;
both fitted curves agree within `4e-15` and have error approximately `5.18e-4`.
These observations do not establish a universal Rhino fitting-tolerance floor.

The combined fitted-output comparison therefore uses absolute `1e-5`, relative
`1e-12`, separately from the strict direct-map comparison:

```sh
tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/curve_surface_morph.json \
  --absolute-epsilon 1e-5 --relative-epsilon 1e-12
```

`surface_orient.json` additionally runs six actual command scenarios, preserving
selection, grouping, object count, domain, degree, and rationality checks. Its
curve records now contain 257 **unrounded** samples instead of rounded control
nets. Native command tests check the fitted geometry against the direct map at
the document's `1e-9` tolerance and retain undo/redo checks. The Rhino command
comparison passes at absolute `4e-6`, relative `1e-10`; the largest coordinate
difference is `3.34e-6`, consistent with the independently observed Rhino fit
error. It is not a claim of `1e-9` agreement with Rhino's approximate output.

The point-map probes, native analytic regressions, fit-error assertions, and
command-state checks serve different purposes. None should be replaced by
control-count equality or a single relaxed global comparison epsilon.

The checkpoint passes 1,289 Rust tests, 18 Python tests, and strict Clippy.
All 116 native fixtures (1,188 operations) execute successfully; 369 recorded
curve comparisons and 20 recorded surface-jet comparisons retain their
documented epsilons. This is bounded regression evidence, not full Rhino parity.
