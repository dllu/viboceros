# USD BRep limitations and bugs

This ledger tracks findings relevant to Viboceros's pinned usd-brep 1.0.0
(`c9c5979f04cea8c38213004d4f04df0dcfb40d05`). It distinguishes upstream
behavior from restrictions in our adapter. See [integration](usd-brep.md) for
setup and [upstream known issues](../third_party/usd-brep/KNOWN_ISSUES.md) for
issues reported by NVIDIA that have not all been reproduced here.

## Native API behavior verified during integration

- **Off-surface normal drops can lose boundary-crossing intervals.**
  `SmSurfaceDropCurve::FindBoundaryStartPoints` used intersection seeds at two
  interval-start queries even when the input was marked off the surface. A line
  above a plane consequently returned success with no curves at tight tolerances.
  The [generated-source patch](../third_party/patches/usd-brep-surface-drop-start-normal.patch)
  uses normal seeds for off-surface inputs and retains intersection seeds for
  on-surface inputs. Three crossing cases recover their expected intervals;
  the other nine direct recipes remain unchanged. See the
  [surface pull foundation](smlib-surface-pull.md) for baseline evidence, image
  certification and outstanding branch/seam qualification.

- **Knot removal reports a handled extremum-search failure as fatal.**
  `N_CrvRemoveKnot` explicitly substitutes the conservative bound one when
  `N_BasisFindGlobalMax` fails, then leaves `error` set while returning a valid
  curve. Distant polynomial extensions consequently report convergence or
  parameter errors. The [generated-source patch](../third_party/patches/usd-brep-knot-removal-bound-error.patch)
  clears the handled status in all three fallback sites, retaining the same
  bound, removal tolerance and acceptance criterion. The
  [qualification](smlib-boundary-extension.md) retains failing baseline results,
  16 corrected direct cases and 30 real C ABI cases, including instrumented
  native libraries and adapter code under AddressSanitizer.
- **End-span extension underallocates arrays at full-order interior breaks.**
  Both `N_CrvExtendByParamDist` and `N_CrvExtendByDist` size the output with
  `m + 2*p - mult`, allowing multiplicity `p+1` to subtract a nonexistent
  insertion. Quadratic parameter extension returns an error at either end;
  distance extension writes outside its control/knot arrays, confirmed by
  AddressSanitizer. The [generated-source repair](../third_party/patches/usd-brep-extension-break-allocation.patch)
  counts only missing copies of the interior knot. All eight isolated
  ordinary/full-break, start/end and parameter/distance probes pass analytic
  position/domain checks afterward, including a complete instrumented kernel.
  See the [qualification](smlib-extension.md) and
  [retained baseline and sanitizer outcomes](smlib-extension-producer-comparison.json).
- **Whole surface blending misaligns unequal rail domains and can corrupt the heap.**
  `SmBrep::CreateEdgeEdgeBlend` applies division to only part of its affine
  offset numerator. `[2,5]` is mapped to `[90,110]` when the first rail is
  `[10,30]`, then evaluated over `[10,30]`. Eight isolated unequal-domain
  G1/G2 probes abort or crash in the unmodified release. It also reverses the
  second rail twice for explicit end-to-start correspondence, while the ruled
  branch ignores that flag; successful baseline outputs miss the requested
  boundary correspondence by two model units. The native build now applies a
  [bounded generated-source patch](../third_party/patches/usd-brep-blend-rail-parameterization.patch)
  for the offset, single reversal and ruled flag, leaving tolerances and the
  pinned submodule unchanged. All 24 cases pass with independent Hermite and
  boundary checks, including an AddressSanitizer build of both complete native
  libraries. Fixed curved/rational extrusions are additionally covered by the
  [derivative-scaling qualification](smlib-blend-derivative-scaling.md);
  extreme domains, automatic direction and whole-command shape parity remain
  unqualified. See the
  [direct qualification](smlib-surface-blend-producer.md) and
  [retained outcomes](smlib-surface-blend-producer-probe.json).
- **Surface-boundary blends rescale the tangent without rescaling higher jets.**
  `SmDerivSurfDefinition::GetSampleValues` changes takeoff length while keeping
  the original second/third directional derivatives. Curved extrusion fixtures
  report native success but a cross normal curvature error as large as `7.13`.
  The [generated-source repair](../third_party/patches/usd-brep-blend-derivative-scaling.patch)
  applies the chain rule's squared/cubed speed factors. Independent direct
  rational jets agree within `1e-10` in 144 polynomial/rational constructions;
  fitted cross normal curvature errors fall below `3.3e-8`. Reverse-direction
  mixed curvature retains a `4.9e-5` skinning residual, so this repair is not a
  certificate of whole-surface G2 continuity. See the
  [qualification and reproduction](smlib-blend-derivative-scaling.md) and
  [all retained outcomes](smlib-blend-derivative-scaling-comparison.json).
- **Intersection-curve trimming can accept an off-curve vertex projection.**
  `SmEdgeDefinition::IsInteriorPoint` checked that nearest-point projection
  succeeded but ignored its model-space distance. A boundary-crossing lamp
  cutter returned native edge/vertex gaps up to `4.7084e-3` against stored zone
  radii near `1e-9`, before copying or portable extraction. The merged
  [lamp-well regression](native-lamp-well.md) verifies a bounded
  [source patch](../third_party/patches/usd-brep-interior-point.patch) requiring
  finite distance within the existing tolerance before authorizing a trim.
  CMake applies it to a generated source copy; the pinned submodule is unchanged.
  Both Boolean postprocessing modes pass original native endpoint-zone checks,
  strict portable validation and 3DM roundtrip. Legal additive native zone radii
  are translated into local portable bounds and checked against every original
  incident edge; geometry and validation tolerances are not moved globally.
- **The available curve-offset sampling producer ignores the binormal in its
  circle branch.** For a counterclockwise radius-two XY circle, distance `0.125`
  and normal `+Z`, the documented `B × T` side independently requires radius
  `1.875`. `N_CrvOffsetPtSampling` returns radius `2.125`; changing the normal
  to `-Z` still returns `2.125`. The branch constructs `r + d` without using
  `B`, while the free-form and line branches use that vector. The
  [direct probe](../tools/usd_brep/curve_offset_probe.cpp) and
  [retained output](smlib-curve-offset-probe.json) reproduce both orientations.
  Source: [NL_CrvOffset.cpp](../third_party/usd-brep/source/SMLib/src/NL_CrvOffset.cpp).
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
  is not substituted. See [qualification](smlib-curve-offset-fit-comparison.json).
- **General adaptive curve-offset declarations are absent from the default
  kernel build.** `N_CrvOffset`, `N_CrvOffsetApprox` and the constant/variable-
  direction functional producers are inside `#if NLIB_UNUSED`. The default
  shared library exports `N_CrvOffsetPtSampling` and none of those general
  producers; an attempted direct call also fails to link. Enabling the unused
  blocks has not been qualified. This is separate from the sampling defects
  and limits above. The adapter uses the available generic curve approximator
  plus checked native trim/split edits with shared corner policy. See
  [curve-offset integration](smlib-curve-offset.md).
  `SmOffsetCurve` itself treats normalized distances below `SM_EFF_ZERO`
  (`1e-12`) as zero, and its derivative evaluator's zero-distance branch
  continues after evaluating the base curve. Such distances use the portable
  fitter. Source: [SmOffsetCurve.cpp](../third_party/usd-brep/source/SMLib/src/SmOffsetCurve.cpp).
- **Banded LU does not pivot around zero diagonals.** On the independently
  invertible matrix `[[0,1],[1,0]]`, `N_RealMatrixLuDecompose` with banded storage
  returns status `1`; native full partial-pivot LU succeeds. The
  [direct probe](../tools/usd_brep/grid_band_probe.cpp) then solves RHS
  `[1e-280,2e-280]` exactly as `[2e-280,1e-280]` using
  `N_RealMatrixRightForBack`; [retained output](smlib-point-grid-band-probe.json)
  records both statuses. This is an algorithm restriction, not a claim that
  no-pivot LU should solve every invertible matrix. Closed point grids now try
  the existing native banded producer with alternate-end cyclic ordering when
  `count > 4 * degree + 1`, then retry the original full pivoted producer if
  exact support, factorization, back solving or input residuals fail. Other
  basis/span and invalid-input errors still propagate. A 64-row exact cyclic
  permutation regression exercises the retry with every diagonal zero.
  Source: [NL_Math.cpp](../third_party/usd-brep/source/SMLib/src/NL_Math.cpp).
- **Point interpolation supplies clamped endpoint constraints for unclamped bases.**
  `N_FitCrvInterpGivenParams` accepts the quadratic basis
  `[-2,-1,0,0.8,2,3,4]` and samples of `X(u)=u` at `[0,0.4,1.4,2]`,
  but its endpoint identity rows produce actual endpoint limits
  `0.17511520737327191` and `1.778801843317972` instead of `0` and `2`.
  Linear precision independently gives controls `[-0.5,0.4,1.4,2.5]`;
  the producer returns `[0,0.31520737327188947,1.5133640552995387,2]`.
  The [direct probe](../tools/usd_brep/rebuild_interpolation_probe.cpp) evaluates
  the endpoint limits with independent quadratic coefficients, avoiding the
  native endpoint evaluator's identical clamping assumption.
  [Retained output](smlib-curve-rebuild-probe.json) records the result.
  Periodic Rebuild instead uses the shared native derivative-basis and matrix
  adapter with cyclic control constraints; existing basis edits only supply
  clamped targets to the fitting convenience producer. Source:
  [NL_CrvFit.cpp](../third_party/usd-brep/source/SMLib/src/NL_CrvFit.cpp).
- **The real-vector pivot solver skips small nonzero RHS values.**
  `N_RealMatrixForBackPivot` advances its first active row only when the RHS
  exceeds `NL_LUDT`. For `[[2,1],[1,2]]` and RHS `[1e-18,0]`, it returns
  `[5e-19,0]` instead of the independent inverse solution
  `[2e-18/3,-1e-18/3]`. The
  [direct probe](../tools/usd_brep/loft_solver_probe.cpp) reproduces this;
  [retained output](smlib-loft-interpolation-probe.json) records both results.
  Native periodic Loft and point interpolation instead apply the LU row
  permutation to homogeneous targets, then use `N_RealMatrixForBack`, which
  does not skip small channels. The
  factorization is shared across all control rows, and normalized residual
  checks retain every input constraint. Kernel tolerances are unchanged.
  Source: [NL_Math.cpp](../third_party/usd-brep/source/SMLib/src/NL_Math.cpp).
- **Isocurve extraction copies unclamped boundary rows and rejects full-order
  fixed-axis breaks.** `N_SrfExtractIsoCrv` at V end `2` on a surface with
  V degree two, Y control coordinates `[0,1,2,3]` and knots
  `[-2,-1,0,0.8,2,3,4]` returns Y `3`; the independent de Boor endpoint limit
  is `28/11`. With V knots `[0,0,0,0.5,0.5,0.5,1,1,1]`, extraction at `0.5`
  instead returns an error because multiplicity exceeds the degree.
  The [direct probe](../tools/usd_brep/isocurve_probe.cpp) reproduces both
  behaviors without relying on the native endpoint evaluator;
  [retained output](smlib-isocurves-probe.json) records them. The adapter reuses
  checked one-sided clamping at an unclamped endpoint, and directly selects the
  exact separated row/column at full-order knots (right limit inside/at start,
  left limit at the active end). Interior extraction uses the existing producer,
  with exact span/multiplicity and varying-basis checks. Source:
  [NL_SrfTool.cpp](../third_party/usd-brep/source/SMLib/src/NL_SrfTool.cpp).
- **Patch extraction snaps near knots and changes the geometric restriction.**
  `N_SrfExtractPatch` snaps all four boundaries using a fixed `1e-8` parameter
  tolerance, then reparameterizes the output to the originally requested box.
  On the exact bilinear surface `S(u,v)=(u,v,0)`, requesting U `[1e-9,0.75]`
  produces a left boundary at X `0` while reporting U start `1e-9`.
  The [direct probe](../tools/usd_brep/patch_extraction_probe.cpp) reads the
  extracted controls and compares against the independent bilinear definition;
  [retained output](smlib-surface-seams-probe.json) records the discrepancy.
  The producer also clamps both axes even when the other direction is kept
  whole, and its active-end branches share the clamped-padding assumptions
  of curve extraction (source audit). SrfSeam therefore uses native tensor
  refinement or batches the existing checked curve extractor per control
  row/column, preserving the complete unselected basis. Source:
  [NL_SrfTool.cpp](../third_party/usd-brep/source/SMLib/src/NL_SrfTool.cpp).
- **Interior full-order curve splitting shares the control-buffer underflow
  (source audit).** With degree two and full knots
  `[0,0,0,0.5,0.5,0.5,1,1,1]`, `N_CrvSplit` at `0.5` obtains span `5` and
  multiplicity `3`. It allocates three left controls but copies four, and its
  right copy starts at index `-1`. The same indexing defect was reproduced
  at a shifted active-start group during endpoint integration. This interior
  call was audited, not executed. Native single-parameter splits now extract
  two checked intervals with `N_CrvExtractCrvSeg`; full-order sides keep their
  independent controls. [Closed-curve seam preparation](smlib-curve-seams.md)
  also reuses this checked split adapter rather than calling the unsafe splitter.
  See [directed segments](smlib-directed-segments.md),
  [NL_CrvTool.cpp](../third_party/usd-brep/source/SMLib/src/NL_CrvTool.cpp), and
  [regressions](../crates/viboceros-smlib/tests/directed_segments.rs).
- **Segment extraction assumes a clamped active end.** For the rational
  quadratic with knots `[-2,-1,0,0.8,2,3,4]`, `N_CrvExtractCrvSeg(0.25,2)`
  retains the last source control as its endpoint `[8,3,2]`, rather than the
  actual endpoint `[7.25,1.75,1.25]`. Its active-end branch sets the right span
  to the full vector's last index without extracting the unclamped end.
  The independent de Boor limit and the retained Rhino trim definition agree.
  The adapter privately clamps only a requested active end through existing
  reverse/split/reverse operations, or slices off an inactive full-order suffix.
  It verifies the complete resulting interval and interior knot basis. See
  [native segment extraction](smlib-curve-segments.md),
  [direct probe](../tools/usd_brep/segment_extraction_probe.cpp), and
  [NL_CrvTool.cpp](../third_party/usd-brep/source/SMLib/src/NL_CrvTool.cpp).
- **Endpoint evaluation assumes clamped basis coefficients.** `N_BasisEval`
  supplies identity rows at active endpoints regardless of padding.
  `N_CrvEvalPt` therefore returns `[8,3,2]` at the same unclamped active end,
  and cannot independently validate the segment-extraction result above.
  `N_SrfExtractIsoCrv` similarly copies the first/last control row at constant-axis
  active boundaries (source audit); this requires that axis to be clamped.
  Curve endpoint behavior is reproduced by the same direct probe; the tensor
  convenience path was inspected, not executed as a new adapter operation.
  Sources: [NL_BasisBasic.cpp](../third_party/usd-brep/source/SMLib/src/NL_BasisBasic.cpp),
  [NL_CrvAdv.cpp](../third_party/usd-brep/source/SMLib/src/NL_CrvAdv.cpp), and
  [NL_SrfTool.cpp](../third_party/usd-brep/source/SMLib/src/NL_SrfTool.cpp).
- **Some declared fitting producers are absent from the default kernel build.**
  `NL_CrvFit.h` declares `N_FitCrvClosedDerivsParamsKnots` and
  `N_FitPeriodicCubic`, but their implementation blocks are guarded by
  `#if NLIB_UNUSED`. The default release archive exports neither symbol;
  `nm --defined-only` confirms this while finding `N_FitCrvInterpGivenParams`
  and `N_FitCrvLstSqEnds`. A header declaration alone is therefore insufficient
  to select a usable fitting API. Enabling the unused implementation blocks
  has not been qualified. Source:
  [NL_CrvFit.h](../third_party/usd-brep/source/SMLib/inc/NL_CrvFit.h) and
  [NL_CrvFit.cpp](../third_party/usd-brep/source/SMLib/src/NL_CrvFit.cpp).
- **Tensor Bezier decomposition uses negative control indices at full-order
  interior breaks (source audit).** `N_SrfDecomposeToBez` sets `ru = p - mlu`
  and `rv = q - mlv`. At an interior multiplicity `degree + 1`, those values
  are `-1`; subsequent copies index `NBw[-1]` or `NQw[k][-1]`. The corresponding
  curve producer `N_CrvDecomposeBez` explicitly raises negative `r` to zero
  before its copy loop. Native Bezier/single-span conversion therefore composes
  the existing curve producer in U then V, retaining homogeneous intermediate
  controls until final projection. A quadratic 6×6 tensor with full-order `0.5`
  breaks in both axes is retained as an adapter regression. The unsafe tensor
  call itself was not executed. Source:
  [NL_SrfTool.cpp](../third_party/usd-brep/source/SMLib/src/NL_SrfTool.cpp),
  [NL_CrvTool.cpp](../third_party/usd-brep/source/SMLib/src/NL_CrvTool.cpp), and
  [regression](../crates/viboceros-smlib/tests/bezier.rs).
- **Full-order active-start splitting writes outside both control buffers.**
  A valid quadratic with five controls and full knots
  `[-2, 0, 0, 0, 0.5, 1, 2, 3]` has active interval `[0, 1]` and an inactive
  first control. `N_CrvSplit` at `0` obtains span `3`, multiplicity `3`.
  It allocates one left control but copies two, then copies the inactive control
  into right index `-1`. Before the workaround, forcing that split aborted with
  `free(): invalid size`; gdb located the abort in native stack cleanup.
  The previous adapter skipped this split because the endpoint already had
  full multiplicity, then its outer-knot rewrite produced an invalid fourth
  zero. Full multiplicity and fully clamped padding are now checked separately.
  The adapter slices off the inactive prefix or suffix and corresponding full
  knots exactly, preserving every retained Euclidean control and weight; no
  interpolation is required. Other endpoint extraction still uses NLib.
  The shared portable representation policy also avoids adding a repetition
  to an already full-order group beside an outer knot. Regression coverage
  includes both ends, both tensor axes, insertion/no-ops, exact elevation,
  homogeneous degree fitting, degrees through 17, binary chart scales and
  common weight gauges.
  These arbitrary full vectors are qualified independently, rather than
  claimed as OpenNURBS/Rhino serialization parity cases. Source:
  [NL_CrvTool.cpp](../third_party/usd-brep/source/SMLib/src/NL_CrvTool.cpp) and
  [regression](../crates/viboceros-smlib/tests/inactive_endpoint_controls.rs).
- **Repeated endpoint averages can round outside the source interval.**
  For a cubic fit, `N_CrvGetGrevilleAbscissa` averages three copies of `0.8`
  into `0.8000000000000002`. `N_CrvEvalPt` then rejects the sample. Native
  knot removal reproduced this on ordinary `[0, 0.1]` and `[-0.1, 0]` domains
  after binary chart scaling. The shared fitting adapter supplies exact
  clamped endpoint parameters and homogeneous controls to the native fitter,
  retaining native Greville/evaluation calls for interior samples. This adds
  no interpolation solver. The decimal-domain regression compares complete
  output definitions with the portable backend.
  The existing portable mean helper had the same repeated-value rounding
  issue; it now preserves an identical finite knot value exactly. Source:
  [NL_CrvNurb.cpp](../third_party/usd-brep/source/SMLib/src/NL_CrvNurb.cpp) and
  [qualification](smlib-knot-removal.md).
- **Boundary span search skips valid narrow spans.** `N_BasisFindSpan` uses
  an absolute `1e-12` threshold while advancing past allegedly degenerate
  endpoint spans. On the rational quadratic with knots
  `[-2, -1, 0, 0.3, 0.7, 1, 2, 3] * 2^-360`, start/end splitting dropped both
  interior breaks while returning success. Native refinement now shares an
  exactly reversible binary chart with fitting, restores the original chart,
  and rejects changed interior knots or domains. Independent clamping and
  removal regressions retain the source shape across chart/weight scales.
  Source: [NL_BasisBasic.cpp](../third_party/usd-brep/source/SMLib/src/NL_BasisBasic.cpp)
  and [qualification](smlib-knot-removal.md). Arbitrary extreme span ratios
  remain unqualified; conditioning is not a blanket native validity certificate.
- **Greville averages can overflow on finite knot vectors.**
  `N_CrvGetGrevilleAbscissa` sums the degree's knots before dividing and returns
  `NL_NO` even when the result is infinity. A cubic-to-quintic deformable
  ChangeDegree on `[0, 2^1022]` reproduced this, despite every knot and its
  mathematical mean being finite. With station overflow removed, native point
  evaluation also rejected this unscaled chart. The adapter now evaluates and
  interpolates in one privately binary-scaled source/target chart, restoring
  every original target knot around native-fitted controls. Every scaled knot
  must round-trip exactly. The rational interpolation regression covers this
  chart alongside degree reduction and extreme common weight gauges. Source:
  [NL_CrvNurb.cpp](../third_party/usd-brep/source/SMLib/src/NL_CrvNurb.cpp) and
  [qualification](smlib-degree-change.md).
- **Unclamped active-end splitting can overwrite native buffers.**
  `N_BasisFindSpanAndMult` with `NL_LEFT` reports multiplicity `degree + 1`
  whenever the query equals `U[m - degree]`, even for an unclamped vector.
  `N_CrvSplit` uses that value to size its output polygons but copies more
  controls into the left polygon. The quadratic fixture with knots
  `[-2,-1,0,0.8,2,3,4]`, split at active end `2`, aborted with heap corruption
  during `N_EndNurbs`. The debugger located the abort in stack cleanup; source
  inspection identified the undersized left-output polygon in this split path.
  The refinement adapter never takes that path: it reverses the private native
  curve/tensor, splits at its active start, then restores direction.
  The tensor splitter shares the same span query. Regression coverage includes
  both endpoints in both tensor axes. See [knot refinement](smlib-knot-refinement.md)
  and [NL_CrvTool.cpp](../third_party/usd-brep/source/SMLib/src/NL_CrvTool.cpp).
  [MakeNonPeriodic](smlib-nonperiodic.md) now shares this checked endpoint path;
  it does not call the failing active-end split directly.
- **NLib reversal assumes clamped endpoint padding.** `N_CrvReverse` and
  `N_SrfReverse` retain endpoint knot blocks and reverse only interior knots
  using the sum of the outermost values. On a partially clamped vector
  `[0,0,0,1.2,2,3,4]`, this produced nonmonotone interior value `2.8` before
  active end `2`. The adapter retains the existing native control reversal
  and supplies the complete negated/reversed parameter chart, matching the
  existing application convention. This affects definition-based editing;
  modeling imports already clamp their active intervals. See
  [NL_CrvNurb.cpp](../third_party/usd-brep/source/SMLib/src/NL_CrvNurb.cpp)
  and [qualification](smlib-knot-refinement.md).
- **Interior insertion and endpoint clamping are different native operations.**
  The NLib insertion entry points reject the outermost knot values. Unclamped
  active endpoints can be split to perform one-sided clamping; already-clamped
  target multiplicities are no-ops. The adapter uses these existing operations
  rather than canonical import, which would clamp both active ends before the edit.
  Periodic padding and the two unstored OpenNURBS outer knots are restored by
  the shared application representation policy. See the same qualification.
- **Public surface UV swapping rejects valid mixed-sign control weights.**
  `SmBSplineSurface::SwapUV` reloads through `SetCanonical`, whose IGES importer
  restricts weights. It returned `SM_ERR` (1001) for the captured rational
  triangle and quadrilaterals with middle boundary weights `0.2` and `0.1`.
  Their finite surface images agree with Rhino. The adapter uses existing
  NLib `N_SrfCopy` and `N_SwapUV`, then owned construction with full knot
  preservation. Extrusion now shares that path, removing its expanded working
  chart. See [edge surfaces](smlib-edge-surfaces.md).
- **Default ruled/Coons knot matching averages nearby parameter knots.**
  `N_CrvsMakeCompatible` uses domain length divided by 1,000 as its default
  threshold. `N_GetCompatibleKnotVectorToTol` limits it by neighboring knot gaps
  and averages matching groups across curves. This is a parameter-space policy,
  independent of model tolerance; it is not exact knot union. The edge adapter
  first uses existing `N_CrvsMakeCompatibleAdjKnots` with explicit tolerance
  `1e-300`, so subsequent tensor construction sees identical pair bases.
  General charts near that threshold and extreme finite offsets remain unqualified.
  Source: [NL_CrvTool.cpp](../third_party/usd-brep/source/SMLib/src/NL_CrvTool.cpp)
  and [NL_BasisBasic.cpp](../third_party/usd-brep/source/SMLib/src/NL_BasisBasic.cpp).
- **Point-set normal queries can replace a previous classification (source audit).**
  `SmPointSet3d::IsPlanar` uses its sequential classifier, while `GetNormal`
  calls `SetUpAxes`, which reruns best-fit classification and replaces the
  cached type. `SetUpAxes` also ignores the fitter's return status. The extrusion
  adapter rechecks the type after requesting the normal and requires a finite,
  unitizable vector. No fixture inducing a hidden fitter failure has been retained;
  these convenience queries are not independent geometry-validity certificates.
  Source: [SmPointSet.cpp](../third_party/usd-brep/source/SMLib/src/SmPointSet.cpp)
  and [numeric extrusion](smlib-extrusion-direction.md).
- **Surface-control getter stride documentation is reversed.**
  `GetControlPointsExpert` documents U as the inner dimension, but validates
  U stride against the full V row and enumerates V inside U. Following the
  header returned `SM_ERR_INVALID_INPUT` (1007) in all four new sheet fixtures.
  The adapter follows the implementation for both getter and setter; nonsquare
  extrusion, apex and revolution grids check the arrangement. Source:
  [SmBSplineSurface.cpp](../third_party/usd-brep/source/SMLib/src/SmBSplineSurface.cpp)
  and [qualification](smlib-modeling-basis.md).
- **Canonical reconstruction can lose short-span control rows.**
  Revolution reads `GetCanonical`, and surface `SwapUV` reconstructs through
  it. Their unique-knot grouping can turn close groups into an overmultiple
  interior knot; native IGES sanitation can then average/drop controls or fail.
  Extrusion's public swap returned `SM_ERR` (1001) on the tiny-chart regression.
  It now uses the existing NLib transpose without canonical reconstruction.
  Revolution retains a private binary-expanded parameter chart, followed by
  exact output basis restoration. See [modeling bases](smlib-modeling-basis.md).
- **Curve constructors modify nearby knots without adjusting controls.**
  `sm_FixupKnotVector` coalesces knots below scaled zero and spreads other near
  knots. It collapsed the tested `2^-360` active interval during canonical
  creation. The importer now checks the exact clamped basis and restores it
  using existing `SetExpert` around native-generated controls. A short-span
  regression also retains a ten-unit control bulge between nearby knot groups.
  Direct input copies and four standalone sheet producers now share the
  preservation adapter. General capped and downstream small-chart modeling
  remains unqualified. Source:
  [SmNurbsCrv.cpp](../third_party/usd-brep/source/SMLib/src/SmNurbsCrv.cpp) and
  [qualification](smlib-curve-import.md).
- **Unique-knot queries are tolerance grouped, not exact serialization.**
  `sm_GetKnots` combines neighbors satisfying `SM_IS_ZERO`. After restoring
  a tiny chart, expanding those multiplicities still failed to recover the
  actual knot vector. The adapter now reads `GetKnotsAll` for curves and copies
  the full surface knot arrays, removing its manual expansion helper. Source:
  [SmNurbsCrv.cpp](../third_party/usd-brep/source/SMLib/src/SmNurbsCrv.cpp).
- **Point evaluation can reject tiny native parameter charts.** An imported
  curve with restored exact knots at `2^-360` returned `SM_ERR` (1001) on native
  evaluation. Its normalized native query chart matches the original curve
  samples while the exported definition retains the source interval. This
  qualifies the retained fixtures, not all extreme knot-spacing ratios.
  See [curve import](smlib-curve-import.md).
- **Canonical curve construction clamps the active interval automatically.**
  Although the header asks for clamped end multiplicities, `CreateCanonical`
  passes full knot data to the existing `N_Iges126Crv` import routine, which
  clamps its active interval. Direct polynomial, rational, partially clamped
  and periodic imports now pass native evaluation checks. The bridge no longer
  rejects these inputs or duplicates the endpoint extraction in Rust. Source:
  [SmBSplineCurve.cpp](../third_party/usd-brep/source/SMLib/src/SmBSplineCurve.cpp)
  and [qualification](smlib-curve-import.md).
- **Canonical curve degree limit is lower than its header claims.** The header
  allows degrees through 32, but the pinned NLib `NL_MAXDEG` and default
  `NL_DMAX` are 17. A degree-32 construction returned `SM_ERR` (1001) during
  integration; the boundary regression qualifies degree 17 and rejects higher
  inputs. The previous bridge documentation's degree-64 claim was incorrect.
  The adapter now uses the kernel build constant. Source:
  [NL_Nurbsdef.h](../third_party/usd-brep/source/SMLib/inc/NL_Nurbsdef.h).
- **Canonical import validates explicit weights before clamping.** Default
  `NL_WMIN=0.001` and `NL_WMAX=100` reject unequal weights outside that range,
  including weights on inactive source controls. Explicit equal weights also
  encounter those checks before their irrelevant gauge is removed. The adapter
  now uses the existing optional unweighted form for polynomial curves, covering
  positive/negative common gauges between `1e-308` and `1e308`. General rational
  gauge conditioning remains open. Source:
  [NL_Iges.cpp](../third_party/usd-brep/source/SMLib/src/NL_Iges.cpp) and
  [regression](../crates/viboceros-smlib/tests/curve_import.rs).
- **Public sweep wrapper selects rotation and a fixed approximation tolerance
  (source audit).** `SmApiCreateSweep` calls `CreateSweepSurface` with
  `bDoTranslationalSweep=False` and `1e-5`. This is unsuitable for Rhino's
  fixed-orientation ExtrudeCrvAlongCrv sum surface. The adapter uses the same
  existing constructor's exact `True` mode, with a privately translated path.
  This is an API mode limitation, not evidence that its rotating sweep is
  mathematically invalid. See [curve-path extrusion](smlib-translational-sweep.md)
  and [SmApiSurfaces.cpp](../third_party/usd-brep/source/SM_API/src/SmApiSurfaces.cpp).
- **End-cap convenience helper leaves final stitching status unchecked.**
  `SmBrep::CreateEndCapsFromSurface` can return success after calling
  `StitchAndOrient` without checking the result. The apex adapter follows the
  call with checked stitching and manifold status, then validated B-rep export.
  Curve-path extrusion now shares that adapter helper.
  No fixture inducing the ignored internal failure has been retained. Its cap
  planarity checks also use the default `IsPlanar` tolerance; general nearly
  planar cap behavior remains unqualified. See [apex extrusion](smlib-apex.md).
- **Planar-face tolerance argument is ignored by the kernel entry point.**
  `SmApiCreatePlanarFaces` passes a hard-coded `0.01`, but
  `SmBrep::CreatePlanarFacesWith3DCurves` ignores that argument and uses the
  receiving B-rep's tolerance. The bridge configures its private B-rep before
  calling the existing constructor. See [planar surfaces](smlib-planar-surfaces.md).
- **Curve-intersection wrapper fixes tolerance at `0.001` (source audit).**
  `SmApiIntersectCurves` also reports `SM_ERR` for empty successful solves.
  Callers needing model-specific tolerance must use the existing native
  `GlobalCurveIntersect` entry point. This wrapper was inspected during planar
  integration; the final planar command needs only native plane grouping and
  region construction. Source:
  [SmApiIntersectors.cpp](../third_party/usd-brep/source/SM_API/src/SmApiIntersectors.cpp).
- **Revolution wrapper can discard profile pieces (source audit).**
  `SmApiCreateSurfaceRevolution` revolves a profile through a producer that can
  split its kinks, then returns only the first surface from the resulting
  B-rep. The adapter instead uses the existing
  `SmBSplineSurface::CreateSurfOfRevolution` tensor constructor, retaining all
  profile spans. See [native revolution](smlib-revolution.md).
- **Revolution UV flag documentation is inverted in the pinned release.**
  The tensor constructor's header describes `bSwapUV=True` as circular U, but
  its implementation and bridge regressions produce circular U with `False`.
  The adapter explicitly selects that behavior and reparameterizes the result
  with the native operation. Source:
  [SmBSplineSurface.cpp](../third_party/usd-brep/source/SMLib/src/SmBSplineSurface.cpp).
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
  the wrapper's output. See [extrusion integration](smlib-extrusion.md).
- **Extrusion parameter axes differ from Rhino's convention.**
  `SmApiCreateExtrude` creates the linear direction in U and profile in V, with a
  unit extrusion interval. Native NLib transpose and `Reparameterize` align the command
  output to profile U and physical-distance V without another extrusion builder.
  Signed/oblique sheet tests and three complete raw Rhino captures qualify this.
- **Linear-sweep final stitching status is unchecked (source audit).**
  The analytic/simple sweep paths call `StitchAndOrient` without checking its
  return value before reporting success. This is not a validity certificate.
  The bridge rejects coplanar capped offsets, requires a native manifold solid
  for caps, and validates exported B-reps. No fixture inducing an ignored stitch
  failure has been retained. Source:
  [SmPrimitiveCreation.cpp](../third_party/usd-brep/source/SMLib/src/SmPrimitiveCreation.cpp).
- **Stitch distance includes entity tolerances.** `SmStitch::DoStitching` uses
  `max(vertex/edge tolerance sums, requested distance)`. An explicit distance
  alone does not impose a strict command gap cutoff. Its manifold option limits
  gluing to lamina edge pairs; it does not certify the resulting model.
  Gap diagnostics include considered pairs that were not glued. The integration
  retains input uncertainty rather than shrinking entity tolerances to enforce
  a Rhino cutoff. In the planar gap tests this admits a preselected gap of
  `1.85 * model tolerance`, which the default Rust backend rejects. SMLib also
  retains a candidate vertex instead of the Rust backend's averaged vertex.
  These are compatibility differences, not established invalid geometry.
  Source: [SmStitch.cpp](../third_party/usd-brep/source/SMLib/src/SmStitch.cpp).
- **Extreme parameter intervals break stitching despite spatial normalization.**
  An exploded box at scale `2^-360`, spatially normalized to unit extent while
  retaining its original knots, returned `SM_ERR_NULL_POINTER` (1006). Mapping
  edge and surface domains to `[0,1]`, with matching UV-trim mappings, allows
  that fixture to stitch. The
  [regression](../crates/viboceros-smlib/tests/stitching.rs) covers `2^-360`, unit
  scale and `2^360`; the command also verifies reversal, copying and Undo/Redo.
- **Solid-region nesting is unsuitable for some sheet assemblies.** Stitching
  a side face and two coincident copies of an adjacent side of a `2 × 3 × 5`
  box returned `SM_ERR` (1001) with automatic region nesting. Using the existing
  `SmStitch` option to disable nesting, followed by native orientation, succeeds.
  This is a mode-selection limitation; general coincident/zero-volume solid
  validity is unproven. See the same stitching regression.
- **Native UV trims can overshoot natural domains by roundoff.** Rational ruled
  sheets returned coordinates around `-1.79e-16` outside `[0,1]`; circular
  extrusions returned about `-2.25e-15` outside `[0,3]`. Strict Rust evaluation
  rejected these. Export uses the existing native `SnapToKnots` only for controls
  outside a natural boundary by at most 32 machine epsilons of the chart scale.
  Each changed trim must then continuously certify against its spatial edge
  within the output tolerance. Inconclusive certificates still fail. See
  [export](../crates/viboceros-smlib/native/brep_export.cpp) and the rational-sheet
  stitching regression.
- **Gap refresh can conceal earlier failures (source audit).**
  `SmBrep::RefreshGaps` checks only the last return value in each of its edgeuse
  and vertexuse loops; `RefreshTols` ignores the return value from `RefreshGaps`.
  A successful last item can therefore conceal a previous failure. No fixture
  inducing that failure sequence has been retained. These methods must not be
  treated as validity certificates. Source:
  [SmBrep.cpp](../third_party/usd-brep/source/SMLib/src/SmBrep.cpp).
- **Mass-property accuracy is clamped by SM_API.** The public B-rep property
  wrapper clamps requested relative accuracy to at least `1e-8`. Branch volume
  checks use the existing native face integrator with explicit accuracy instead.
  See [BooleanSplit integration](smlib-boolean-split.md) and
  [bridge](../crates/viboceros-smlib/native/bridge.cpp).
- **Shared context requires serialization.** Default-context calls share
  mutable state. Our C boundary holds a process-wide mutex, including handle
  destruction. This prevents parallel kernel work in one process; isolated
  worker processes remain an option. See
  [upstream concurrency guide](../third_party/usd-brep/.agents/operations/concurrency.md).
- **Standard ARM64 bootstrap dependency unavailable in the trial.** Upstream
  Packman's pinned `3.12.15+nv1-manylinux_2_35_aarch64` download failed. The
  kernel-only CMake build avoids that dependency. This observation does not
  establish current availability of upstream's package server. See
  [native trial](usd-brep.md#reproduce-the-native-trial).

## Observed compatibility differences

- **Curve intersection equation convergence and parameter merging can miss
  nearby crossings.** The original global solver reports no contacts for
  `C(t)=(2t,4t(1-t),0)` against `Y=1-1e-10` at tolerance `1e-12`. Small normal
  equations can stop before physical coincidence; generic duplicate comparison
  also uses `1e-4` of a parameter domain. Shallow off-center tangencies can exhaust
  endpoint/midpoint guesses. Generated scoped repairs add physical convergence,
  separate tangent retries, physical point comparison and quarter guesses.
  Shared source-space polishing removes rounded multiple-knot tangent families.
  [Qualification](smlib-curve-intersections.md) retains original and patched
  diagnostics, ASan, independent analytic cases and saved SDK replays. The pinned
  kernel stays unchanged; general event completeness remains unqualified.
  Source: [SmCurve.cpp](../third_party/usd-brep/source/SMLib/src/SmCurve.cpp).

- **Whole-domain curve trims preserve source outer entries in Viboceros.**
  The raw `trim-rational-whole` record has source outer values `-2` and `4` in
  Viboceros and padded neighboring values `-1` and `3` in Rhino. OpenNURBS
  omits those two full-vector slots; all stored knots and controls agree.
  The native adapter preserves the existing exact no-op policy. The primary
  raw comparison retains this mismatch; independent reference-definition
  sampling checks the same active locus. No comparison-time knot normalization
  hides it. See [qualification](smlib-curve-segments.md).
- **Dedicated knot removal differs from Rhino's interpolation rule.**
  `N_CrvRemoveKnot` uses a tolerance-bound removal algorithm. On the retained
  cubic-middle fixture, tolerance `1e10` removes one knot but leaves a maximum
  control-coordinate difference of `3.473170830538584` from Rhino. The adapter
  instead shares the existing native homogeneous interpolation producer with
  ChangeDegree. This is a command compatibility difference, not an upstream
  geometry-invalidity claim. See [qualification](smlib-knot-removal.md) and
  [direct native probe](../tools/usd_brep/knot_removal_probe.cpp).
- **Native edge-surface exact matching retains a near-knot distinction.**
  In the two-multispan ruled fixture, domain mapping produces knots `0.3` and
  `0.30000000000000004`. The native adapter retains both, giving five profile
  controls versus Rhino's four. All captured 13×13 surface coordinates agree
  within absolute `2e-12`, relative `1e-14`. The complete raw difference remains
  in [edge-surface qualification](smlib-edge-surfaces-comparison.json); no
  comparison-time knot merging or coefficient filtering is applied.
  This is an adapter representation policy, not an established upstream geometry bug.
- **Fixed: numeric extrusion direction was an application policy difference.**
  The live short-span XZ profile is extruded along positive Y by Rhino, while
  the application's numeric ExtrudeCrv path used global Z. The command now uses
  native classification of parallel profile directions, positive dominant-axis
  orientation and active CPlane fallback. This was an app command-default gap,
  not a demonstrated usd-brep geometry bug. See
  [direction qualification](smlib-extrusion-direction.md).
- **Short-span revolution has a different V domain.** The new live command
  uses V `[0, 10.837476096278881]` in Rhino and `[0, 1]` in Viboceros. The tensor
  controls, weights, U basis and full-degree V knot-group multiplicities match,
  describing the same rational Bezier patch images. Raw V differences remain
  in the primary comparison; parameter/derivative parity is not claimed. See
  [retained comparison](smlib-modeling-basis-comparison.json).
- **Portable planar overlap policy differs from current Rhino.** The live
  fixture with an outer rectangle, two competing inner rectangles and a separate
  inner hole produces areas 83 and 16 in both Rhino and usd-brep. The previous
  custom command policy detached both competing holes and produced areas 99, 16
  and 16. Native PlanarSrf now uses the kernel region constructor directly;
  portable builds retain the older policy pending correction. See
  [planar qualification](smlib-planar-surfaces.md).
- **Native Join is not raw Rhino parity.** The
  [92-case replay](smlib-join-comparison.json) has no native execution errors but
  only 14 complete raw matches. Stored uncertainty, UV domains, face senses,
  geometry and ordering still differ. Even disconnected fresh outputs can gain
  model-sized native tolerances and edge-aligned UV parameter domains while
  retaining their spatial definitions. Broader geometry parity remains open.
- **Closed-box wall offsets differ from Rhino's SDK.** SMLib constructs wall
  volumes for the tested closed-box Solid requests, while Rhino returned no
  bodies. Positive box offsets also have 12 edges versus Rhino's 36. Geometric
  measurements and partition differences are tracked separately in
  [offset qualification](smlib-offset-surfaces.md) and its
  [partition ledger](smlib-offset-partitions.json). Command-level semantics and
  broader self-intersecting/collapsing offset behavior remain unqualified.

## Adapter restrictions, not established upstream bugs

- **General surface contacts retain bounded curve and trim qualification.**
  Previously unsupported surface pairs can use `GlobalSurfaceIntersect` with
  independently owned XYZ and both UV curves. Original-surface certificates
  accept each complete curve; unknown classifications, missing witnesses and
  inconclusive proofs fail. Established analytic definitions remain in use.
  Approximate XYZ curves can miss exact spatial trim-edge contacts, reproduced
  on `z=x²+y³` and `z=1−xy` with a partial rectangular face. The adapter now
  intersects coupled UV witnesses with original trim loops through the native
  curve producer, retaining both face restrictions and holes. Every resulting
  XYZ/UV span is recertified on both original surfaces. Tiny corner spans become
  points only with a same-sign control-hull bound. Fitted traces that penetrate
  tangent boundaries can retain a span only with a continuous certificate
  against the original trim image; larger crossings remain excluded. Consecutive
  retained ranges use the same original curves. Contact discovery, trim
  topology, arbitrary conditioning and Rhino speed parity remain unqualified.
  See [surface qualification](smlib-surface-surface-intersections.md) and
  [trim qualification](smlib-surface-trim-intersections.md) and
  [tangency qualification](smlib-surface-trim-tangencies.md).

- **Native curve/surface contacts retain bounded preparation and acceptance.**
  Curve/B-rep queries share the underlying-surface producer and original-face
  trim policy. Unsupported degree, full-order interior breaks, weight ratios
  and collapsed frames retain the portable query. Original-geometry point
  residuals and per-span overlap samples reject invalid native proposals, but
  do not certify continuous coincidence, arbitrary conditioning or event
  completeness. See [qualification](smlib-curve-surface-intersections.md).

- **Curve self-contact search has bounded qualification.** IntersectSelf and
  SelSelfIntersectingCrv use the native global producer for eligible curves,
  preserving source-space acceptance and portable preparation fallback. A
  generated repair removes mirrored span pairs before solving; without it, a
  backtracking range can span both visits across its turnaround. Analytic
  contacts and basic command behavior are checked separately from the retained
  nested-collinear Rhino command/API differences. No completeness, continuous
  overlap or kernel-speed certificate is claimed. See
  [self-intersection qualification](smlib-curve-self-intersections.md).

- **Curve Match retains measured continuity policy around native preparation.**
  Trim-to-target, exact elevation, end-control refinement and six-control affine
  Greville interpolation now reuse native adapters. Shared G0/G1/G2 and rational
  curvature rules, far-end preservation and continuity audits remain intact.
  All 160 pre-change records match; 159 match retained Rhino captures completely.
  The existing average-position boundary tail discrepancy remains in the raw
  comparison. Native degree/chart limits and the 1,000-control affine budget
  apply, with broader arbitrary matching and fresh captures unqualified. See
  [qualification](smlib-curve-match.md).
- **TweenCurves retains direct blending/refit policy around native sampling.**
  Nonlinear sampled sources now use the shared native open cubic adapter,
  including 10,000 stations and 10,002 output controls at 9,999 divisions.
  The extended native allowance applies only to open handle systems with four
  channels; other interpolation layouts retain their existing limits. Public
  InterpCrv/curve-through point ceilings stay at 256. ControlPoint and adaptive
  Hermite Refit remain direct constructions, and linear sampled sources stay
  degree one. Arbitrary conditioning, continuous agreement and performance
  relative to Rhino remain unqualified. See [sampling qualification](smlib-curve-tween.md).
- **Sweep1 and Pipe retain model policy around native interpolation.**
  Rail clamping, section-site refinement, adaptive fitting and homogeneous
  section solving now use the existing native adapters. Equal parameter rows
  require two fixed identity constraints; arbitrary duplicate interpolation
  stations remain invalid. Frame transport, rational blending, rail-weight
  queries and independent profile/seam audits stay shared. The 4,096/512-axis
  and 262,144-surface control budgets remain in force. All 63 pre-change sweep
  records match, including existing diagnostic and closed-circle cases;
  broader Rhino basis/domain and off-surface query differences remain open.
  See [native sweep interpolation](smlib-sweep-interpolation.md).
- **FitCrv retains adaptive and fixed-handle policy around native solving.**
  Its endpoint and internal kink handles are control constraints, so the adapter
  supplies identity rows alongside native Greville basis rows. Cubic systems
  keep five-wide storage; other degrees use native banded LU instead of the
  application's reduced dense solve. The fixed-row adapter allows 4,096 stations,
  while other native interpolation jobs retain their previous bounds. FitCrv
  keeps the existing 4,096/512 control budgets and sampled acceptance policy;
  no continuous error bound or arbitrary-conditioning qualification is claimed.
  Internal Sweep1/Pipe fitting now shares the native producer. Fresh Rhino fitting captures remain
  pending Wine licensing startup. See [native FitCrv](smlib-curve-fit.md).
- **Prepared morph previews reuse checked editing/extraction adapters.**
  Curve and surface template elevation, standalone curve cages, B-rep isocurve
  extraction and wire-interval trimming now share usd-brep producers. Native
  elevation across independent full-order pieces admits the tested discontinuous
  low-degree curve and surface preview cases that portable preparation rejects.
  Shared sample, fixed-control, trim-hole, boundary and display budgets remain.
  Prepared cages retain their interpolation producer when the supplied point
  map has no override. Editing/extraction above degree 17 retains existing
  portable producers; the earlier mixed-degree interpolation limit remains.
  These are display approximations, not accepted or continuously certified
  model geometry. See [native preview preparation](smlib-morph-preview.md).
- **Curve offset retains admission/corner policy around native producers.** Offset
  and OffsetMultiple use native pointwise smooth fitting and checked trim/split adapters
  for NURBS corner construction, including cyclic seams, concave trims, sharp
  extension leaves and through-point candidates. The native-feature offset
  oracle selects the same producer. OffsetMultiple closed-region admission and
  nesting also delegate Bézier decomposition and subdivision; positive-weight
  hull, endpoint, intersection, orientation and containment predicates remain
  shared, including the 8,192-piece limit and ambiguous-boundary rejection.
  Source/region admission, side/nesting,
  analytic offsets, independent fit acceptance, cusp checks, intersection choice,
  corner joins and source attributes/history remain shared. Unsupported source
  degrees above 17 retain existing portable edits; errors at supported degrees
  propagate. Native global curve intersections supply join and region contacts,
  retaining source-space acceptance and shared local polishing. The convenience
  point-sampling offset producer is still excluded; no Rhino speed claim is implied.
  See [native offsets](smlib-curve-offset.md) and
  [native curve intersections](smlib-curve-intersections.md).
- **Morph fitting retains shared approximation policy around native solving.**
  Bend, Twist, Taper, Maelstrom and deformable OrientOnSrf reuse native fixed-row
  banded interpolation for adaptive cubic and rational degree-3/6/9 candidates,
  including prepared previews. Full-order sided limits use independent fixed
  rows at duplicate parameters. Native preparation skips the portable basis
  matrices; all point maps, candidate checks, budgets and assembled B-rep audits
  stay shared. The existing 512/256 control and eight-million sample ceilings
  remain. Mixed-degree preview axes above the pinned degree-17 limit return an
  explicit backend error. The same exact-span and chart guards apply; no
  arbitrary-conditioning, continuous-error or Rhino-speed guarantee is claimed.
  All 33 saved API/orientation/interchange records match the earlier portable producer;
  fresh Rhino captures remain pending Wine licensing startup. See
  [native morph interpolation](smlib-morph-interpolation.md).
- **Point-grid interpolation retains continuation and conditioning limits.**
  Open directions use the existing native banded solver; closed directions fold
  repeated control columns, reuse alternate-end cyclic band ordering for
  larger grids, and retain full partial-pivot LU for small/failed band jobs.
  Closed point-grid channels share `N_RealMatrixRightForBack` scalar solves (no
  small-RHS skip), retaining every channel solve and residual check before
  the shared constant-channel correction between axes. The native
  zeroth-derivative basis supports boundary-span polynomial continuation; the
  [direct probe](../tools/usd_brep/grid_continuation_probe.cpp) returns the exact
  independent quadratic coefficients `[9/8,-1/4,1/8]` at `-0.5` outside active
  domain `[0,5]`. [Retained output](smlib-point-grid-probe.json) records this
  capability, not an upstream defect. Exact-span guards, homogeneous solves,
  constant-channel preservation and independent tensor residuals remain in
  force. Counts remain capped at 256 per axis and degrees at 11. Ill-conditioned
  degree-9/11 continuation fixtures are retained separately from passing
  comparisons. The degree-nine native net differs from the portable full-pivot
  net by `1.18e-7`; independent 80-digit construction-site residuals are
  `9.92e-9` native versus `6.41e-10` portable. These are adapter qualification
  limits, not proof of an upstream algorithm defect. Guarded cyclic bands
  improve the measured 256×256 closed construction by about 5.3 times versus
  the prior native producer; measured closed cases still take 1.32–1.51 times
  the portable time. Qualification and timings are recorded in
  [native point grids](smlib-point-grid.md).
  A constant-channel solve bypass changed the 64×7 degree-11 diagnostic's
  extrapolated construction samples by `1.17e-8` versus the existing producer,
  exceeding its `1e-8` allowance even with the open-axis solver unchanged.
  That bypass is not used: every channel retains its native solve/residual
  check before shared constant correction. This is an adapter qualification
  limit, not proof that an upstream solver is incorrect.
  Arbitrary conditioning and Rhino performance remain unqualified.
  See [native point grids](smlib-point-grid.md) and the existing narrow-span and
  real-RHS solver findings above.
- **TweenSurfaces retains measured matching policy around native operations.**
  Compatible control nets bypass rebuilding; unequal nets repeat native
  forced-open Rebuild before every output. Refit uses native clamping,
  elevation and refinement around the exact shared knot union; the tested
  one-ULP-separated knots `0.3` and `0.30000000000000004` remain distinct.
  SamplePoints uses native open-grid basis/solves with shared mean-chord
  stations and independent evaluator residuals. Closed point-grid continuation
  now shares the native solver. Native elevation composes independent full-order
  pieces; portable Refit still rejects the tested discontinuous source. Early retained
  Rhino option changes leave extra source-shaped copies, an existing command
  discrepancy that remains separate from requested-net agreement. See
  [native TweenSurfaces](smlib-surface-tween.md).
- **Surface Rebuild shares native operations while retaining tensor policy.**
  Midpoint and boundary isocurves use the checked native extractor, and each
  direction shares one native banded factorization across all tensor rows.
  Exactly constant target channels are preserved between passes so later
  high-degree interpolation cannot amplify their first-pass roundoff. This
  fixes an existing application interpolation issue, not an upstream defect.
  Source sampling, arc-length stations, corners, poles and trim reconstruction
  retain application policy. Command degrees remain 1–11 and counts at most
  256 per axis; direct native calls also obey the pinned degree-17 limit.
  Higher portable degrees, arbitrary conditioning and continuous fit error
  are not native qualifications. See [surface Rebuild](smlib-surface-rebuild.md).
- **Curve Rebuild retains application sampling and tangent policy.**
  The native path uses the existing derivative-basis, LU and homogeneous solve
  adapter for uniform open and periodic targets. Periodic columns are aligned
  with Greville stations, then both rows and columns alternate the two ends of
  the cycle, keeping a band no wider than `2 * degree + 1`. The adapter restores
  the original control order, weights and repeated periodic controls. This
  removes the custom open elimination and Fourier solve from native command,
  preview and oracle paths; portable builds retain them. The existing degree
  1–11 and 1,000-point ceilings remain. Fresh Rhino capture, continuous fit
  error and performance relative to Rhino remain unqualified. See
  [native Rebuild](smlib-curve-rebuild.md).
- **Curve Blend shares the native exact endpoint interpolator with BlendSrf.**
  Native builds use derivative basis rows and homogeneous back solving for
  independent G0/G1/G2 constraints. Picking, tangent/curvature queries, default
  handles, bulges and domain policy remain shared; G0/G0 retains its direct line.
  Portable builds use the same common endpoint interface. The Python oracle can
  select `native_smlib=True`, and all 282 retained Rhino SDK definitions preserve
  degree, weights and control coordinates within `1e-12`, with the existing
  arc-length domain ceiling of `5e-8`. Native command history and readonly source
  behavior are checked. Interactive handles, G3/G4 and general conditioning
  remain unqualified. See [native curve Blend](smlib-curve-blend.md).
- **BlendSrf uses native parent edits and exact mixed endpoint interpolation.**
  Clamping, exact elevation, knot matching and rectangular parent restrictions
  now use the existing checked native adapters in command and GUI preparation.
  Native derivative basis rows and one shared homogeneous factorization now
  construct the exact control net for all nine G0/G1/G2 boundary pairs, including
  independent bulges. Boundary jets, target policy, budgets and diagnostics stay
  shared. Parent degrees above 17 retain portable editing; the blend's own
  degree remains 1–5, so its interpolation uses the native solver throughout.
  Native errors propagate before document mutation. Independent polynomial
  checks cover the full 4,096-column budget and coordinates at `1e-280`/`1e280`.
  The topology blend convenience producer has one operation-wide blend flag,
  and its derivative definition has one tangency/curvature type; the SM_API
  wrapper forwards a null definition to its G2 default. Whole-producer shape
  and mixed-target correspondence remain unqualified. This is an API/model
  difference verified from the headers and forwarding source, not a reproduced
  numerical bug. See [native BlendSrf preparation](smlib-surface-blend.md).
- **MakePeriodic smoothing uses the native nonuniform periodic solve.**
  Closed curve and surface control rows retain shared active knots, Greville
  phase, normalized homogeneous targets, repeated seam controls and command
  history policy. Native builds use existing `N_BasisDerivs`, guarded cyclic
  band LU with a retry through full pivoted storage, and scalar homogeneous
  channel back solving instead of portable basis rows and faer LU.
  Nonuniform Greville stations can start inside the domain; they do not use
  the uniform Rebuild column shift; their own starting control index aligns
  native columns. Systems above `4 * degree + 1` unique
  controls try a band at most `4 * degree + 1` entries wide; exact support and
  native solve/residual checks guard the retry. The adapter supports degrees 2–17
  and at most 1,000 unique controls; requests beyond either bound keep the
  portable producer, and supported native failures propagate. Surface rows
  now share a native factorization in batches of at most 512 curves, respecting
  the adapter's existing 2,048-channel limit without rejecting larger tensors.
  Each row keeps its own weight normalization; U-fast packing is restored
  between directions. Already-periodic knot bases retain the original row
  mapping policy. Smooth=No retains direct cyclic control edits.
  All 42 [pre-batch native definitions](smlib-make-periodic-batch-comparison.json)
  match with zero numeric difference. The tested rational tensors run about
  twice as fast in the [diagnostic benchmark](smlib-make-periodic-surface-performance.json);
  this is not a Rhino timing claim.
  Independent rational constraints cover both surface axes, extreme signed
  weight gauges and tiny nonzero channels. Fresh Rhino capture, arbitrary
  conditioning and performance relative to Rhino remain unqualified. See
  [MakePeriodic integration](smlib-make-periodic.md).
- **Point interpolation retains application policy around native basis and solves.**
  The missing periodic fit exports above and the different skin handle policy
  prevent a direct producer substitution. InterpCrv and interpolated
  CurveThroughPt/CurveThroughPolyline now share the existing Loft constraint
  system, native `N_BasisDerivs`, LU and homogeneous back solve. Open systems
  use five-wide banded storage; periodic systems retain full pivoted storage.
  Public point interpolation keeps the existing 256-point budget, knot/tangent
  policy and exact command coincidence rule. Large TweenCurves sets now share
  the native banded producer; portable builds keep their linear-memory solver.
  All 43 complete point/curve-through
  records match the portable backend; fresh Rhino interpolation capture remains
  pending the Wine licensing startup. Extreme span ratios, continuous parity
  and speed relative to Rhino are unqualified. See
  [qualification and comparison](smlib-curve-interpolation.md).
- **Loft section matching uses native edits, while interpolation retains Rhino
  policy.** The existing `N_CreateSkinSrfParams` open path sets its loft control
  index to `k` for `k+1` input profiles and requires that control count's knot
  length. Rhino's open cubic Loft policy instead uses two extra controls for
  automatic end handles. Its closed skin path also derives matching end
  derivatives from a rotated/chord-parameterized profile set, rather than our
  periodic interpolation policy. These are producer/representation differences,
  not demonstrated upstream geometry defects. Loft, Sweep1 and Pipe now reuse
  native clamping, exact degree elevation and knot refinement for common section
  bases, retaining their existing weight, frame and interpolation policies.
  General native skin replacement is not qualified. Source:
  [NL_SrfAdv.cpp](../third_party/usd-brep/source/SMLib/src/NL_SrfAdv.cpp) and
  [section matching](smlib-section-bases.md).
- **Fixed: shared chart conditioning omitted some knots and trim/interpolation
  queries.** Native annulus preview wires exposed subnormal source knot entries
  beside a moderate domain. The binary chart previously checked reversibility
  of source knots only when a scalar-query list was supplied; interpolation
  supplied no list. Curve trimming and refinement also omitted requested
  parameters from chart selection. The shared helper now always preserves every
  source knot, and interpolation supplies every station, trim supplies both
  boundaries, and refinement supplies its requested knot. Regressions retain
  exact subnormal domains and fixed-row targets, alongside independent trim
  samples and complete annulus preview wires. This repairs an adapter defect;
  upstream tolerances and exact-span guards are unchanged. It is not a general
  mixed-range or conditioning guarantee. See [preview preparation](smlib-morph-preview.md).
- **Fixed: isocurve chart conditioning rejected subnormal near-boundary queries.**
  Planar trimmed-face closest-point and density paths can supply nonzero
  subnormal parameters or knots next to zero. Scaling a moderate fixed-axis
  chart downward by its maximum knot then lost those exact values and rejected
  two ExtractIsocurve hole regressions. The isocurve adapter now chooses a
  smaller reversible binary scale that includes both knots and the requested
  parameter. This changes no source coordinates, spans or tolerances. Native
  subnormal tests and both picked/density hole regressions pass with the repair;
  arbitrary extreme span/weight ratios remain unqualified. See
  [native isocurve tests](../crates/viboceros-smlib/tests/isocurves.rs).
- **Fixed: isocurve Split edges kept local charts in public output.**
  Seven of eight fresh Split command captures differed only in their 3D edge
  domains: our rectangular-face builder intentionally used lossless local
  edge charts, while Rhino retained the original surface-axis intervals.
  Isocurve Split now uses a constructor variant that translates every edge knot
  back exactly, including reversed sides, while still evaluating in the local
  frame. Geometry, UV trims and tolerances are unchanged. The general builder
  retains its existing policy. This was command output metadata, not a usd-brep
  geometry defect. See [surface extraction](smlib-surface-segments.md).
- **Whole-domain surface trimming retains unused full outer knots.**
  The deliberate whole-domain trim preserves the original full U/V vectors
  `[-2,-1,0,0.8,2,3,4]`. Rhino reconstructs their artificial outer slots as
  `[-1,-1,0,0.8,2,3,3]`. Every stored knot and control matches, and independent
  9×9 evaluation grids match within `1e-10`. The complete raw definition
  discrepancy is retained in the surface extraction comparison, extending the
  already documented curve no-op padding difference. No comparison-time
  normalization hides it. See [surface extraction](smlib-surface-segments.md).
- **Fixed: exporting untouched trim endpoints rounded exact pole controls.**
  Checked native extraction copied clamped natural endpoint controls unchanged,
  but our conditioned homogeneous-to-Euclidean export projected them again.
  Independent surface columns could then differ by an ULP at a repeated pole.
  The surface-grid constructor correctly uses exact control equality for an
  exact singular side; that rounded pole gained a tiny boundary ring and the
  tapered Global round Pipe cap failed to close. Export now retains the complete
  original endpoint payload whenever the requested boundary is an unchanged,
  fully clamped natural end. It changes neither tolerances nor genuinely
  modified controls. The pole-payload regression and Local/Global Pipe volume
  test reproduce the fix. See
  [surface extraction tests](../crates/viboceros-smlib/tests/surface_segments.rs)
  and [Pipe tests](../crates/viboceros-command/src/pipe.rs).
- **Surface seam closure retains its existing homogeneous tolerance policy.**
  Both backends reject the tested periodic cubic surface after a common weight
  scale of `1e-300`: its homogeneous interior samples are indistinguishable
  under the inherited absolute coincidence tolerance. This happens before
  native refinement or extraction. The new adapter retains that validation,
  including rejecting nets that close only projectively. This is a shared
  closure-policy restriction, not evidence of a usd-brep arithmetic defect.
  The native regressions qualify common scales `1`, `1e100` and `-1e100`; wider
  gauges are not certified by surface seam integration. See
  [surface seam regressions](../crates/viboceros-smlib/tests/surface_seams.rs).
- **Numeric-length extraction retains the existing sampler's accuracy policy.**
  Native SubCrv length/midpoint extraction uses usd-brep trimming, while length
  inversion and closed-seam preparation retain the shared policy. All 20 signed
  SDK records match at the prior `1e-6` comparison ceiling; five differ at a
  stricter `1e-8` ceiling, with residuals up to `7.99e-8`. Native versus portable
  piece tests retain the same endpoints and geometry. The raw diagnostic is
  preserved separately, rather than attributed to an upstream trimming bug.
  General length inversion is not continuously certified. See
  [qualification](smlib-length-segments.md).
- **Native Bezier decomposition uses bounded gauges and resources.**
  ConvertToBeziers and ConvertToSingleSpans accept degrees through 17, with
  one million input/aggregate output controls. Directional strips normalize
  each independent control line, preserving gauges such as `1e-280` and
  `1e280` on different rows. Both keeps intermediate controls homogeneous
  under one common binary gauge; wider overall weight ratios remain unsupported.
  Final controls at infinity, nonfinite coordinates and underflowed restored
  weights return the existing typed unrepresentable-control error. These are
  adapter restrictions, rather than established upstream geometry defects.
  See [qualification](smlib-bezier.md).
- **Multiple-knot removal uses ordered refits and sampled crease selection.**
  RemoveMultiKnot shares native homogeneous fitting with RemoveKnot rather than
  adding another control solver. Groups are refitted in descending parameter
  order; surfaces finish U before reevaluating V candidates. Roundoff can
  accumulate through long sequences, and the shared transverse angle samples
  do not certify a continuous maximum. Approximation deviation is also
  uncertified. These are adapter/command-policy qualifications, not reproduced
  upstream geometry bugs. See [qualification](smlib-multiple-knot-removal.md).
- **Native raw NURBS edits have bounded input and output resources.**
  InsertKnot, MakeNonPeriodic, ChangeDegree, RemoveKnot and RemoveMultiKnot use
  NLib directly, with the
  pinned degree-17 limit and a one-million-control cap. A common binary weight
  scale conditions homogeneous inputs; every exported Euclidean coordinate
  and restored weight must be finite and every weight nonzero. Extreme weight
  ratios can still fail, and degree reduction/deformable interpolation has no
  certified deviation bound. No-op conversions preserve the original payload.
  These are adapter limits, not evidence that the upstream mathematics is
  invalid. See [refinement](smlib-knot-refinement.md),
  [degree changes](smlib-degree-change.md) and their native regressions.
- **Upstream exact elevation rejects full-order interior breaks; the adapter composes pieces.**
  `N_CrvElevateDegree` failed on a degree-two curve with six controls and knots
  `[0, 0, 0, 0.5, 0.5, 0.5, 1, 1, 1]` when raised to degree four. The native
  [direct probe](../tools/usd_brep/piecewise_elevation_probe.cpp) retains status
  `1` for that whole-curve call and status `0` for both independent pieces.
  The adapter now reuses native elevation per contiguous full-order block,
  keeps intermediate controls homogeneous, and verifies the assembled target
  basis. Native-owned copying respects the constructor's borrowed arrays.
  Curve, tensor and native TweenSurfaces Refit cases now succeed; the portable
  elevation path still rejects the tested discontinuity. This is an upstream
  producer restriction with an adapter workaround, not evidence of invalid
  accepted geometry. See [qualification](smlib-piecewise-elevation.md) and
  [regressions](../crates/viboceros-smlib/tests/degree_change.rs).
- **Fixed: whole-chart pole certification rejected a valid mixed-sign interior.**
  The rational triangular EdgeSrf has a positive-weight collapsed natural
  boundary while unrelated interior controls have mixed signs. The general
  surface-image certificate cannot handle that chart and previously rejected
  its missing pole connector. For clamped natural boundaries, export now proves
  the complete stored row/column lies within the vertex tolerance using the
  existing exact distance predicate and rational convex-hull property.
  Nonclamped boundaries retain the general certificate. No sampled acceptance
  or blanket tolerance inflation is added. See the native edge regression.
- **Canonical modeling imports change curve-path extrusion representation.**
  The live unclamped profile/path fixture retains an unclamped path knot vector
  in Rhino, while native canonical creation clamps the active interval.
  The earlier duplicate Rust clamp has been removed. Six of seven complete raw sheet definitions match; this one has
  different outer knots and controls. All seven captured 9×9 parameter grids
  match, with maximum point-coordinate error `7.11e-15`. Independent native
  grids also check preparation against the original unclamped curves. This is
  an adapter representation difference, not an established usd-brep geometry
  bug or a continuous-surface parity certificate. See
  [qualification](smlib-translational-sweep.md) and
  [retained raw comparison](smlib-translational-sweep-comparison.json).
- **Fixed: tolerance snapshots preceded native UV creation.**
  `GetOrCreateUVTrimCurve` can increase edge and vertex tolerances. The exporter
  previously copied those fields first, then copied the UV trims, producing
  inconsistent uncertainty and a rejected planar gap join. Export now copies
  tolerances after creating every trim; it uses the native values directly.
  The gap-stitching regression covers this without blanket tolerance inflation.
- Native features currently require Linux and system oneTBB. The USD scene
  layer and upstream Python packaging are not part of our kernel-only build.
- B-rep transfer requires clamped, continuous NURBS and manifold topology.
  Mixed open/closed imports, nonmanifold sheets and unsupported singular or
  periodic topology can be rejected. Native face reversal can swap UV axes.
  See [transfer](smlib-brep-transfer.md) and [sheet import](smlib-offset-surfaces.md).
- Exact export rejects native surface/curve classes requiring approximation.
  Offset export requests an explicit native NURBS approximation bound and checks
  the achieved bound; this is not a proof of the entire output boundary.

Keep new findings tied to a source, reproducible test or retained capture.
Do not label a Rhino partition/representation difference as an upstream geometry
bug without evidence that the mathematical output is invalid.
