# USD BRep limitations and bugs

This ledger tracks findings relevant to Viboceros's pinned usd-brep 1.0.0
(`c9c5979f04cea8c38213004d4f04df0dcfb40d05`). It distinguishes upstream
behavior from restrictions in our adapter. See [integration](usd-brep.md) for
setup and [upstream known issues](../third_party/usd-brep/KNOWN_ISSUES.md) for
issues reported by NVIDIA that have not all been reproduced here.

## Native API behavior verified during integration

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
  Extrusion's swap returned `SM_ERR` (1001) on the tiny-chart regression before
  a working chart was used. A private binary-expanded parameter chart preserves
  those groups through the existing producers, followed by exact output basis
  restoration. See [modeling bases](smlib-modeling-basis.md).
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
  unit extrusion interval. Native `SwapUV` and `Reparameterize` align the command
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

- **Numeric extrusion direction is still an application policy difference.**
  The live short-span XZ profile is extruded along positive Y by Rhino, while
  the application's numeric ExtrudeCrv path uses global Z. The low-level native
  sheet producer passes explicit-vector interpolation checks. This is an app
  command-default gap, not a demonstrated usd-brep geometry bug. See
  [new modeling observations](smlib-modeling-basis.md).
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
