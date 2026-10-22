# Native NURBS bases and evaluation

[Limitations ledger](../usd-brep-limitations.md) · [Native modeling](../native-modeling.md)

- **Fixed: orientation rebuilt and damaged valid surface parameter charts.**
  `SmBSplineSurface::SwapUV` passed through canonical knot cleanup, collapsing
  `[1e12,1e12+1]` and merging narrow spans. `SmFace::SwapUV` mirrored UV curves
  through a rounded diagonal frame. The generated
  [surface](../../third_party/patches/usd-brep-surface-transpose-basis.patch) and
  [trim](../../third_party/patches/usd-brep-face-transpose-uv.patch) repairs use the
  existing NLib tensor transpose and an axis-aligned coordinate transform.
  Source domains, knots and homogeneous controls survive the permutation.
  Some natural-boundary sheets remain unjoined despite exact imported
  coefficients. See [qualification and retained failures](../native-uv-transpose.md).

- **Fixed: NURBS copies merged valid source knot groups.** Both B-spline copy
  constructors ran tolerance-based knot fixup after copying. In the direct
  curve/surface probe, 16 of 24 valid inputs consequently produce changed and
  invalid copied bases. The generated-source
  [curve](../../third_party/patches/usd-brep-copy-curve-basis.patch) and
  [surface](../../third_party/patches/usd-brep-copy-surface-basis.patch) repairs retain
  full source knots in copies; raw-data constructors keep their existing
  sanitization. B-rep import reuses the checked basis adapter before constructing
  topology. All 24 repaired copies preserve knots and homogeneous controls
  exactly. See [the probe and pole-transfer qualification](../native-pole-transfer.md).

- **Fixed: endpoint evaluation assumed clamped basis coefficients.**
  `N_BasisEval` previously supplied identity rows at every active endpoint;
  `N_CrvEvalPt` returned `[8,3,2]` at the same unclamped end. Evaluation now
  selects exact nonzero spans and computes the actual inward limit, retaining
  identity shortcuts only at clamped ends. Shared basis derivatives and curve
  derivatives use the same span rules, including full-order breaks and small
  positive spans. Curve and tensor point queries are directly exercised by
  the [368-station qualification](../native-basis-evaluation.md), supplemented by
  88 fresh SDK evaluations. The separate earlier
  [isocurve extraction repair](../native-isocurve-limits.md) corrected the
  constant-axis producer's similar boundary-row shortcut.
  Sources: [NL_BasisBasic.cpp](../../third_party/usd-brep/source/SMLib/src/NL_BasisBasic.cpp),
  [NL_CrvAdv.cpp](../../third_party/usd-brep/source/SMLib/src/NL_CrvAdv.cpp), and
  [NL_SrfTool.cpp](../../third_party/usd-brep/source/SMLib/src/NL_SrfTool.cpp).

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
  [NL_CrvNurb.cpp](../../third_party/usd-brep/source/SMLib/src/NL_CrvNurb.cpp) and
  [qualification](../smlib-knot-removal.md).

- **Boundary span search skips valid narrow spans.** `N_BasisFindSpan` uses
  an absolute `1e-12` threshold while advancing past allegedly degenerate
  endpoint spans. On the rational quadratic with knots
  `[-2, -1, 0, 0.3, 0.7, 1, 2, 3] * 2^-360`, start/end splitting dropped both
  interior breaks while returning success. Native refinement now shares an
  exactly reversible binary chart with fitting, restores the original chart,
  and rejects changed interior knots or domains. Independent clamping and
  removal regressions retain the source shape across chart/weight scales.
  Source: [NL_BasisBasic.cpp](../../third_party/usd-brep/source/SMLib/src/NL_BasisBasic.cpp)
  and [qualification](../smlib-knot-removal.md). Arbitrary extreme span ratios
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
  [NL_CrvNurb.cpp](../../third_party/usd-brep/source/SMLib/src/NL_CrvNurb.cpp) and
  [qualification](../smlib-degree-change.md).

- **NLib reversal assumes clamped endpoint padding.** `N_CrvReverse` and
  `N_SrfReverse` retain endpoint knot blocks and reverse only interior knots
  using the sum of the outermost values. On a partially clamped vector
  `[0,0,0,1.2,2,3,4]`, this produced nonmonotone interior value `2.8` before
  active end `2`. Endpoint adapters now use repaired splits directly and no
  longer invoke these reversal producers. The producers themselves remain
  unqualified on arbitrary padding; modeling imports already clamp their
  active intervals. See
  [NL_CrvNurb.cpp](../../third_party/usd-brep/source/SMLib/src/NL_CrvNurb.cpp)
  and [qualification](../smlib-knot-refinement.md).

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
  chart. See [edge surfaces](../smlib-edge-surfaces.md).

- **Surface-control getter stride documentation is reversed.**
  `GetControlPointsExpert` documents U as the inner dimension, but validates
  U stride against the full V row and enumerates V inside U. Following the
  header returned `SM_ERR_INVALID_INPUT` (1007) in all four new sheet fixtures.
  The adapter follows the implementation for both getter and setter; nonsquare
  extrusion, apex and revolution grids check the arrangement. Source:
  [SmBSplineSurface.cpp](../../third_party/usd-brep/source/SMLib/src/SmBSplineSurface.cpp)
  and [qualification](../smlib-modeling-basis.md).

- **Canonical reconstruction can lose short-span control rows.**
  Revolution reads `GetCanonical`, and surface `SwapUV` reconstructs through
  it. Their unique-knot grouping can turn close groups into an overmultiple
  interior knot; native IGES sanitation can then average/drop controls or fail.
  Extrusion's public swap returned `SM_ERR` (1001) on the tiny-chart regression.
  It now uses the existing NLib transpose without canonical reconstruction.
  Revolution retains a private binary-expanded parameter chart, followed by
  exact output basis restoration. See [modeling bases](../smlib-modeling-basis.md).

- **Curve constructors modify nearby knots without adjusting controls.**
  `sm_FixupKnotVector` coalesces knots below scaled zero and spreads other near
  knots. It collapsed the tested `2^-360` active interval during canonical
  creation. The importer now checks the exact clamped basis and restores it
  using existing `SetExpert` around native-generated controls. A short-span
  regression also retains a ten-unit control bulge between nearby knot groups.
  Direct input copies and four standalone sheet producers now share the
  preservation adapter. General capped and downstream small-chart modeling
  remains unqualified. Source:
  [SmNurbsCrv.cpp](../../third_party/usd-brep/source/SMLib/src/SmNurbsCrv.cpp) and
  [qualification](../smlib-curve-import.md).

- **Unique-knot queries are tolerance grouped, not exact serialization.**
  `sm_GetKnots` combines neighbors satisfying `SM_IS_ZERO`. After restoring
  a tiny chart, expanding those multiplicities still failed to recover the
  actual knot vector. The adapter now reads `GetKnotsAll` for curves and copies
  the full surface knot arrays, removing its manual expansion helper. Source:
  [SmNurbsCrv.cpp](../../third_party/usd-brep/source/SMLib/src/SmNurbsCrv.cpp).

- **Point evaluation can reject tiny native parameter charts.** An imported
  curve with restored exact knots at `2^-360` returned `SM_ERR` (1001) on native
  evaluation. Its normalized native query chart matches the original curve
  samples while the exported definition retains the source interval. This
  qualifies the retained fixtures, not all extreme knot-spacing ratios.
  See [curve import](../smlib-curve-import.md).

- **Canonical curve construction clamps the active interval automatically.**
  Although the header asks for clamped end multiplicities, `CreateCanonical`
  passes full knot data to the existing `N_Iges126Crv` import routine, which
  clamps its active interval. Direct polynomial, rational, partially clamped
  and periodic imports now pass native evaluation checks. The bridge no longer
  rejects these inputs or duplicates the endpoint extraction in Rust. Source:
  [SmBSplineCurve.cpp](../../third_party/usd-brep/source/SMLib/src/SmBSplineCurve.cpp)
  and [qualification](../smlib-curve-import.md).

- **Canonical curve degree limit is lower than its header claims.** The header
  allows degrees through 32, but the pinned NLib `NL_MAXDEG` and default
  `NL_DMAX` are 17. A degree-32 construction returned `SM_ERR` (1001) during
  integration; the boundary regression qualifies degree 17 and rejects higher
  inputs. The previous bridge documentation's degree-64 claim was incorrect.
  The adapter now uses the kernel build constant. Source:
  [NL_Nurbsdef.h](../../third_party/usd-brep/source/SMLib/inc/NL_Nurbsdef.h).

- **Canonical import validates explicit weights before clamping.** Default
  `NL_WMIN=0.001` and `NL_WMAX=100` reject unequal weights outside that range,
  including weights on inactive source controls. Explicit equal weights also
  encounter those checks before their irrelevant gauge is removed. The adapter
  now uses the existing optional unweighted form for polynomial curves, covering
  positive/negative common gauges between `1e-308` and `1e308`. General rational
  gauge conditioning remains open. Source:
  [NL_Iges.cpp](../../third_party/usd-brep/source/SMLib/src/NL_Iges.cpp) and
  [regression](../../crates/viboceros-smlib/tests/curve_import.rs).
