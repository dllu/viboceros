# USD BRep limitations and bugs

This ledger tracks findings relevant to Viboceros's pinned usd-brep 1.0.0
(`c9c5979f04cea8c38213004d4f04df0dcfb40d05`). It distinguishes upstream
behavior from restrictions in our adapter. See [integration](usd-brep.md) for
setup and [upstream known issues](../third_party/usd-brep/KNOWN_ISSUES.md) for
issues reported by NVIDIA that have not all been reproduced here.

## Native API behavior verified during integration

- **Interior full-order curve splitting shares the control-buffer underflow
  (source audit).** With degree two and full knots
  `[0,0,0,0.5,0.5,0.5,1,1,1]`, `N_CrvSplit` at `0.5` obtains span `5` and
  multiplicity `3`. It allocates three left controls but copies four, and its
  right copy starts at index `-1`. The same indexing defect was reproduced
  at a shifted active-start group during endpoint integration. This interior
  call was audited, not executed. Native single-parameter splits now extract
  two checked intervals with `N_CrvExtractCrvSeg`; full-order sides keep their
  independent controls. See [directed segments](smlib-directed-segments.md),
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
- **Exact degree elevation rejects the tested full-order interior break.**
  `N_CrvElevateDegree` failed on a degree-two curve with six controls and knots
  `[0, 0, 0, 0.5, 0.5, 0.5, 1, 1, 1]` when raised to degree four. The native
  adapter returns an error without changing the source; the regression retains
  the complete input. This is an observed native operation limitation, not
  evidence that its accepted output is mathematically invalid. General
  discontinuous degree elevation remains unsupported by this adapter. See
  [degree-change regressions](../crates/viboceros-smlib/tests/degree_change.rs).
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
