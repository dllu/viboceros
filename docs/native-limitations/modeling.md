# Native modeling, interpolation and fitting

[Limitations ledger](../usd-brep-limitations.md) · [Native modeling](../native-modeling.md)

- **Tapered extrusion tests slivers on unrelated parameter intervals.**
  The ruling helper measured its two rails over intervals from the first two
  source segments. A rectangle parameterized over `[0,4]` consequently lost two
  side faces while the producer returned success. The
  [generated-source patch](../../third_party/patches/usd-brep-tapered-sliver-domains.patch)
  measures each actual rail over its own natural interval and avoids indexing a
  second source segment when there is only one. Circular frusta, sharp and round
  rectangle extrusions, open sheets and shifted-domain rectangles pass afterward.
  See [command scope and evidence](../commands/extrude-crv-tapered.md).

- **Off-surface normal drops can lose boundary-crossing intervals.**
  `SmSurfaceDropCurve::FindBoundaryStartPoints` used intersection seeds at two
  interval-start queries even when the input was marked off the surface. A line
  above a plane consequently returned success with no curves at tight tolerances.
  The [generated-source patch](../../third_party/patches/usd-brep-surface-drop-start-normal.patch)
  uses normal seeds for off-surface inputs and retains intersection seeds for
  on-surface inputs. Three crossing cases recover their expected intervals;
  the other nine direct recipes remain unchanged. See the
  [surface pull foundation](../smlib-surface-pull.md) for baseline evidence, image
  certification and outstanding branch/seam qualification.

- **Whole surface blending misaligns unequal rail domains and can corrupt the heap.**
  `SmBrep::CreateEdgeEdgeBlend` applies division to only part of its affine
  offset numerator. `[2,5]` is mapped to `[90,110]` when the first rail is
  `[10,30]`, then evaluated over `[10,30]`. Eight isolated unequal-domain
  G1/G2 probes abort or crash in the unmodified release. It also reverses the
  second rail twice for explicit end-to-start correspondence, while the ruled
  branch ignores that flag; successful baseline outputs miss the requested
  boundary correspondence by two model units. The native build now applies a
  [bounded generated-source patch](../../third_party/patches/usd-brep-blend-rail-parameterization.patch)
  for the offset, single reversal and ruled flag, leaving tolerances and the
  pinned submodule unchanged. All 24 cases pass with independent Hermite and
  boundary checks, including an AddressSanitizer build of both complete native
  libraries. Fixed curved/rational extrusions are additionally covered by the
  [derivative-scaling qualification](../smlib-blend-derivative-scaling.md);
  extreme domains, automatic direction and whole-command shape parity remain
  unqualified. See the
  [direct qualification](../smlib-surface-blend-producer.md) and
  [retained outcomes](../smlib-surface-blend-producer-probe.json).

- **Surface-boundary blends rescale the tangent without rescaling higher jets.**
  `SmDerivSurfDefinition::GetSampleValues` changes takeoff length while keeping
  the original second/third directional derivatives. Curved extrusion fixtures
  report native success but a cross normal curvature error as large as `7.13`.
  The [generated-source repair](../../third_party/patches/usd-brep-blend-derivative-scaling.patch)
  applies the chain rule's squared/cubed speed factors. Independent direct
  rational jets agree within `1e-10` in 144 polynomial/rational constructions;
  fitted cross normal curvature errors fall below `3.3e-8`. Reverse-direction
  mixed curvature retains a `4.9e-5` skinning residual, so this repair is not a
  certificate of whole-surface G2 continuity. See the
  [qualification and reproduction](../smlib-blend-derivative-scaling.md) and
  [all retained outcomes](../smlib-blend-derivative-scaling-comparison.json).

- **The available curve-offset sampling producer ignores the binormal in its
  circle branch.** For a counterclockwise radius-two XY circle, distance `0.125`
  and normal `+Z`, the documented `B × T` side independently requires radius
  `1.875`. `N_CrvOffsetPtSampling` returns radius `2.125`; changing the normal
  to `-Z` still returns `2.125`. The branch constructs `r + d` without using
  `B`, while the free-form and line branches use that vector. The
  [direct probe](../../tools/usd_brep/curve_offset_probe.cpp) and
  [retained output](../smlib-curve-offset-probe.json) reproduce both orientations.
  Source: [NL_CrvOffset.cpp](../../third_party/usd-brep/source/SMLib/src/NL_CrvOffset.cpp).

- **Inherited curve-offset sampling is not qualified for the shared pointwise
  tolerance check.** On `C(t)=(2t,4t(1-t),0)` with `+Z` normal and distance
  `0.125`, `N_CrvOffsetPtSampling` with degree three, inherited parameters,
  endpoint tangents and tolerance `1e-5` returns success and 36 controls.
  Independent de Boor evaluation of the stored net at 1,001 stations differs
  from the analytic offset by up to `5.45e-5` at corresponding parameters.
  This does not establish failure of a geometric closest-distance bound;
  the API has no separate pointwise-error parameter. The same probe records
  native and independent evaluations, respecting unit weights for polynomial
  output. Native smooth offsets instead use `SmOffsetCurve` with the generic
  `SmCurve::ApproximateCurve` parameter-matching mode, followed by shared
  independent pointwise acceptance checks. The convenience sampling producer
  is not substituted. See [qualification](../smlib-curve-offset-fit-comparison.json).

- **General adaptive curve-offset declarations are absent from the default
  kernel build.** `N_CrvOffset`, `N_CrvOffsetApprox` and the constant/variable-
  direction functional producers are inside `#if NLIB_UNUSED`. The default
  shared library exports `N_CrvOffsetPtSampling` and none of those general
  producers; an attempted direct call also fails to link. Enabling the unused
  blocks has not been qualified. This is separate from the sampling defects
  and limits above. The adapter uses the available generic curve approximator
  plus checked native trim/split edits with shared corner policy. See
  [curve-offset integration](../smlib-curve-offset.md).
  `SmOffsetCurve` itself treats normalized distances below `SM_EFF_ZERO`
  (`1e-12`) as zero, and its derivative evaluator's zero-distance branch
  continues after evaluating the base curve. Such distances use the portable
  fitter. Source: [SmOffsetCurve.cpp](../../third_party/usd-brep/source/SMLib/src/SmOffsetCurve.cpp).

- **Banded LU does not pivot around zero diagonals.** On the independently
  invertible matrix `[[0,1],[1,0]]`, `N_RealMatrixLuDecompose` with banded storage
  returns status `1`; native full partial-pivot LU succeeds. The
  [direct probe](../../tools/usd_brep/grid_band_probe.cpp) then solves RHS
  `[1e-280,2e-280]` exactly as `[2e-280,1e-280]` using
  `N_RealMatrixRightForBack`; [retained output](../smlib-point-grid-band-probe.json)
  records both statuses. This is an algorithm restriction, not a claim that
  no-pivot LU should solve every invertible matrix. Closed point grids now try
  the existing native banded producer with alternate-end cyclic ordering when
  `count > 4 * degree + 1`, then retry the original full pivoted producer if
  exact support, factorization, back solving or input residuals fail. Other
  basis/span and invalid-input errors still propagate. A 64-row exact cyclic
  permutation regression exercises the retry with every diagonal zero.
  Source: [NL_Math.cpp](../../third_party/usd-brep/source/SMLib/src/NL_Math.cpp).

- **Fixed: point interpolation supplied clamped endpoint constraints for unclamped bases.**
  `N_FitCrvInterpGivenParams` accepts the quadratic basis
  `[-2,-1,0,0.8,2,3,4]` and samples of `X(u)=u` at `[0,0.4,1.4,2]`,
  but its old endpoint identity rows produced actual endpoint limits
  `0.17511520737327191` and `1.778801843317972` instead of `0` and `2`.
  Linear precision independently gives controls `[-0.5,0.4,1.4,2.5]`;
  the old producer returned `[0,0.31520737327188947,1.5133640552995387,2]`.
  The [direct probe](../../tools/usd_brep/rebuild_interpolation_probe.cpp) evaluates
  the endpoint limits with independent quadratic coefficients, avoiding the
  then-unrepaired native endpoint evaluator's identical clamping assumption.
  The [basis evaluator repair](../native-basis-evaluation.md) fixed evaluation.
  The [interpolation repair](../native-point-interpolation.md) now supplies every
  actual basis row, orders constraints privately and retains a degree-wide
  native band. The same witness now recovers the independent controls and both
  endpoint limits. Degree-one input is copied directly only at knot stations;
  other parameter layouts use the shared native basis and solver.
  [Retained output](../smlib-curve-rebuild-probe.json) records the result.
  Periodic Rebuild instead uses the shared native derivative-basis and matrix
  adapter with cyclic control constraints, which are separate from this
  convenience producer's ordinary interpolation. Source:
  [NL_CrvFit.cpp](../../third_party/usd-brep/source/SMLib/src/NL_CrvFit.cpp).

- **The real-vector pivot solver skips small nonzero RHS values.**
  `N_RealMatrixForBackPivot` advances its first active row only when the RHS
  exceeds `NL_LUDT`. For `[[2,1],[1,2]]` and RHS `[1e-18,0]`, it returns
  `[5e-19,0]` instead of the independent inverse solution
  `[2e-18/3,-1e-18/3]`. The
  [direct probe](../../tools/usd_brep/loft_solver_probe.cpp) reproduces this;
  [retained output](../smlib-loft-interpolation-probe.json) records both results.
  Native periodic Loft and point interpolation instead apply the LU row
  permutation to homogeneous targets, then use `N_RealMatrixForBack`, which
  does not skip small channels. The
  factorization is shared across all control rows, and normalized residual
  checks retain every input constraint. Kernel tolerances are unchanged.
  Source: [NL_Math.cpp](../../third_party/usd-brep/source/SMLib/src/NL_Math.cpp).

- **Some declared fitting producers are absent from the default kernel build.**
  `NL_CrvFit.h` declares `N_FitCrvClosedDerivsParamsKnots` and
  `N_FitPeriodicCubic`, but their implementation blocks are guarded by
  `#if NLIB_UNUSED`. The default release archive exports neither symbol;
  `nm --defined-only` confirms this while finding `N_FitCrvInterpGivenParams`
  and `N_FitCrvLstSqEnds`. A header declaration alone is therefore insufficient
  to select a usable fitting API. Enabling the unused implementation blocks
  has not been qualified. Source:
  [NL_CrvFit.h](../../third_party/usd-brep/source/SMLib/inc/NL_CrvFit.h) and
  [NL_CrvFit.cpp](../../third_party/usd-brep/source/SMLib/src/NL_CrvFit.cpp).

- **Fixed: minimal surface fillets included extrapolated curved rails.**
  `SmFilletExecutive::SurfaceSurfaceFillet` previously left the solver's
  default half-span extension active for `SM_BT_MINIMAL`. A plane/quarter-cylinder
  fillet then contained points off its analytic torus. Minimal mode now uses
  zero extension; other modes accept an explicit search factor. The native
  adapter uses beveled end caps and expands its private search for unequal
  finite parent lengths. A controlled original-policy comparison fails 24 of
  56 independent geometry cases; the repair passes all 56, including under
  AddressSanitizer. See [the qualification](../native-surface-fillets.md).
  Curved `Extend=Yes` remains incomplete: the retained plane/quarter-cylinder
  SDK recipe succeeds in Rhino; the retained native baseline returns status
  1001. The subsequent contact repair does not complete curved extension.

- **Default ruled/Coons knot matching averages nearby parameter knots.**
  `N_CrvsMakeCompatible` uses domain length divided by 1,000 as its default
  threshold. `N_GetCompatibleKnotVectorToTol` limits it by neighboring knot gaps
  and averages matching groups across curves. This is a parameter-space policy,
  independent of model tolerance; it is not exact knot union. The edge adapter
  first uses existing `N_CrvsMakeCompatibleAdjKnots` with explicit tolerance
  `1e-300`, so subsequent tensor construction sees identical pair bases.
  General charts near that threshold and extreme finite offsets remain unqualified.
  Source: [NL_CrvTool.cpp](../../third_party/usd-brep/source/SMLib/src/NL_CrvTool.cpp)
  and [NL_BasisBasic.cpp](../../third_party/usd-brep/source/SMLib/src/NL_BasisBasic.cpp).

- **Public sweep wrapper selects rotation and a fixed approximation tolerance
  (source audit).** `SmApiCreateSweep` calls `CreateSweepSurface` with
  `bDoTranslationalSweep=False` and `1e-5`. This is unsuitable for Rhino's
  fixed-orientation ExtrudeCrvAlongCrv sum surface. The adapter uses the same
  existing constructor's exact `True` mode, with a privately translated path.
  This is an API mode limitation, not evidence that its rotating sweep is
  mathematically invalid. See [curve-path extrusion](../smlib-translational-sweep.md)
  and [SmApiSurfaces.cpp](../../third_party/usd-brep/source/SM_API/src/SmApiSurfaces.cpp).

- **Revolution wrapper can discard profile pieces (source audit).**
  `SmApiCreateSurfaceRevolution` revolves a profile through a producer that can
  split its kinks, then returns only the first surface from the resulting
  B-rep. The adapter instead uses the existing
  `SmBSplineSurface::CreateSurfOfRevolution` tensor constructor, retaining all
  profile spans. See [native revolution](../smlib-revolution.md).

- **Revolution UV flag documentation is inverted in the pinned release.**
  The tensor constructor's header describes `bSwapUV=True` as circular U, but
  its implementation and bridge regressions produce circular U with `False`.
  The adapter explicitly selects that behavior and reparameterizes the result
  with the native operation. Source:
  [SmBSplineSurface.cpp](../../third_party/usd-brep/source/SMLib/src/SmBSplineSurface.cpp).

- **Revolution radial degeneracy depends on coordinate magnitude.**
  The native tensor constructor compares each control's radius with
  `SM_EFF_ZERO * (1 + maximum coordinate magnitude)`. The command translates
  to its axis origin before construction and restores world placement afterward.
  A regression covers a radius near `1e-4` at an origin of `1e9`. General aspect
  ratio and scale extremes remain unqualified.

- **Linear-sweep wrapper lacks explicit creation tolerance.**
  `SmApiCreateLinearSweep` constructs a new B-rep with its default tolerance.
  The native ExtrudeCrv adapter invokes the same existing producer on a private
  B-rep configured with the document tolerance. The pinned old tolerance model's
  `SmBrep::SetTolerance` changes that B-rep, despite header commentary describing
  a context-wide change; setting a temporary unrelated B-rep does not configure
  the wrapper's output. See [extrusion integration](../smlib-extrusion.md).

- **Extrusion parameter axes differ from Rhino's convention.**
  `SmApiCreateExtrude` creates the linear direction in U and profile in V, with a
  unit extrusion interval. Native NLib transpose and `Reparameterize` align the command
  output to profile U and physical-distance V without another extrusion builder.
  Signed/oblique sheet tests and three complete raw Rhino captures qualify this.
