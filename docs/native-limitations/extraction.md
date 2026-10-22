# Native splitting, extraction and knot removal

[Limitations ledger](../usd-brep-limitations.md) · [Native modeling](../native-modeling.md)

- **Knot removal reports a handled extremum-search failure as fatal.**
  `N_CrvRemoveKnot` explicitly substitutes the conservative bound one when
  `N_BasisFindGlobalMax` fails, then leaves `error` set while returning a valid
  curve. Distant polynomial extensions consequently report convergence or
  parameter errors. The [generated-source patch](../../third_party/patches/usd-brep-knot-removal-bound-error.patch)
  clears the handled status in all three fallback sites, retaining the same
  bound, removal tolerance and acceptance criterion. The
  [qualification](../smlib-boundary-extension.md) retains failing baseline results,
  16 corrected direct cases and 30 real C ABI cases, including instrumented
  native libraries and adapter code under AddressSanitizer.

- **End-span extension underallocates arrays at full-order interior breaks.**
  Both `N_CrvExtendByParamDist` and `N_CrvExtendByDist` size the output with
  `m + 2*p - mult`, allowing multiplicity `p+1` to subtract a nonexistent
  insertion. Quadratic parameter extension returns an error at either end;
  distance extension writes outside its control/knot arrays, confirmed by
  AddressSanitizer. The [generated-source repair](../../third_party/patches/usd-brep-extension-break-allocation.patch)
  counts only missing copies of the interior knot. All eight isolated
  ordinary/full-break, start/end and parameter/distance probes pass analytic
  position/domain checks afterward, including a complete instrumented kernel.
  See the [qualification](../smlib-extension.md) and
  [retained baseline and sanitizer outcomes](../smlib-extension-producer-comparison.json).

- **Fixed: native isocurve construction changed the basis and UV partner.**
  `SmBSplineSurface::CreateIsoParametricCurve` used a knot-repairing constructor
  for an already extracted tensor curve. `SmIsoCurve::CreateUVIsoLine` repeated
  the U branch, used the constant-axis interval and double-counted its origin.
  Generated-source repairs preserve the extraction allocation and form the
  correct parameter-coordinate line. DivideAlongCreases now requests the native
  pair directly. Sixteen complete-coefficient/UV cases and an instrumented
  lifetime check cover this; native rejection retains empty outputs.
  See [scope and control runs](../native-isocurve-pairs.md).

- **Fixed: isocurve extraction copied unclamped boundary rows and rejected full-order
  fixed-axis breaks.** `N_SrfExtractIsoCrv` at V end `2` on a surface with
  V degree two, Y control coordinates `[0,1,2,3]` and knots
  `[-2,-1,0,0.8,2,3,4]` returns Y `3`; the independent de Boor endpoint limit
  is `28/11`. With V knots `[0,0,0,0.5,0.5,0.5,1,1,1]`, extraction at `0.5`
  instead returns an error because multiplicity exceeds the degree.
  The [direct probe](../../tools/usd_brep/isocurve_probe.cpp) reproduces both
  behaviors without relying on the native endpoint evaluator;
  [retained output](../smlib-isocurves-probe.json) records them. Production now uses
  a generated-source repair selecting exact one-sided spans and isolated rows.
  Unclamped endpoints use the existing homogeneous reduction instead of the
  outermost row. The adapter's endpoint-clamping workaround is removed; exact
  full-order payload copies and numerical conditioning remain. See the
  [288-case qualification](../native-isocurve-limits.md). Source:
  [NL_SrfTool.cpp](../../third_party/usd-brep/source/SMLib/src/NL_SrfTool.cpp).

- **Fixed: patch extraction snapped near knots and changed the geometric restriction.**
  `N_SrfExtractPatch` snaps all four boundaries using a fixed `1e-8` parameter
  tolerance, then reparameterizes the output to the originally requested box.
  On the exact bilinear surface `S(u,v)=(u,v,0)`, requesting U `[1e-9,0.75]`
  produces a left boundary at X `0` while reporting U start `1e-9`.
  The [direct probe](../../tools/usd_brep/patch_extraction_probe.cpp) reads the
  extracted controls and compares against the independent bilinear definition;
  [retained output](../smlib-surface-seams-probe.json) records the discrepancy.
  The [generated repair](../native-surface-patches.md) removes snapping/remapping
  and uses exact knot groups with actual endpoint multiplicities. The owning
  object path also retains the extracted basis without constructor cleanup.
  Rectangular extraction still clamps both axes. SrfSeam uses native tensor
  refinement or checked directional curve extraction to preserve the complete
  unselected basis. Source:
  [NL_SrfTool.cpp](../../third_party/usd-brep/source/SMLib/src/NL_SrfTool.cpp).

- **Fixed: interior full-order curve splitting shared the control-buffer underflow.**
  With degree two and full knots `[0,0,0,0.5,0.5,0.5,1,1,1]`, the original
  `N_CrvSplit` at `0.5` obtained span `5` and multiplicity `3`.
  It allocated three left controls but copied four, and its
  right copy starts at index `-1`. The same indexing defect was reproduced
  at a shifted active-start group during endpoint integration. This interior
  parent call was audited, not rerun. The [split repair](../native-exact-splits.md)
  now uses exact groups and disjoint full-order control blocks for curves and
  both tensor axes. Public single-parameter splits retain two checked interval
  extractions to clamp both output bases. [Closed-curve seam preparation](../smlib-curve-seams.md)
  shares that representation policy.
  See [directed segments](../smlib-directed-segments.md),
  [NL_CrvTool.cpp](../../third_party/usd-brep/source/SMLib/src/NL_CrvTool.cpp), and
  [regressions](../../crates/viboceros-smlib/tests/directed_segments.rs).

- **Fixed: segment extraction assumed a clamped active end.** For the rational
  quadratic with knots `[-2,-1,0,0.8,2,3,4]`, `N_CrvExtractCrvSeg(0.25,2)`
  previously retained the last source control as its endpoint `[8,3,2]`, rather than the
  actual endpoint `[7.25,1.75,1.25]`. Its active-end branch set the right span
  to the full vector's last index without extracting the unclamped end.
  The independent de Boor limit and the retained Rhino trim definition agree.
  A generated-source repair now uses actual endpoint multiplicities and exact
  knot groups with the existing homogeneous restriction. The adapter's private
  end clamp and inactive-suffix removal are deleted; conditioning, complete
  basis checks and exact isolated endpoint payloads remain. See
  [512-case producer qualification](../native-curve-segment-limits.md),
  [native segment extraction](../smlib-curve-segments.md),
  [direct probe](../../tools/usd_brep/segment_extraction_probe.cpp), and
  [NL_CrvTool.cpp](../../third_party/usd-brep/source/SMLib/src/NL_CrvTool.cpp).

- **Fixed: tensor Bezier decomposition indexed before full-order control blocks.**
  `N_SrfDecomposeToBez` used `ru = p - mlu` and `rv = q - mlv`, which
  become `-1` at multiplicity `degree + 1`. The next-block copies now start
  at zero. The producer also clamps a private surface through the repaired
  splitters and completes degree-zero spans. Viboceros now uses the tensor
  producer directly for Both-direction conversion. The
  [qualification](../native-tensor-bezier.md) retains 1,008 independent expected
  control nets and sanitizer runs. The 396 safe parent cases retain identical
  raw outcomes; known unsafe/uninitialized parent paths remain excluded.
  The separate object-layer `sm_DecomposeSrf` already clamps negative copy
  indices upstream; its unclamped/degree-zero behavior is not newly qualified.
  Source: [NL_SrfTool.cpp](../../third_party/usd-brep/source/SMLib/src/NL_SrfTool.cpp),
  [patch](../../third_party/patches/usd-brep-tensor-bezier.patch), and
  [bridge regressions](../../crates/viboceros-smlib/tests/bezier.rs).

- **Fixed: full-order active-start splitting wrote outside both control buffers.**
  A valid quadratic with five controls and full knots
  `[-2, 0, 0, 0, 0.5, 1, 2, 3]` has active interval `[0, 1]` and an inactive
  first control. The original `N_CrvSplit` at `0` obtained span `3`, multiplicity `3`.
  It allocated one left control but copied two, then copied the inactive control
  into right index `-1`. Before the workaround, forcing that split aborted with
  `free(): invalid size`; gdb located the abort in native stack cleanup.
  The previous adapter skipped this split because the endpoint already had
  full multiplicity, then its outer-knot rewrite produced an invalid fourth
  zero. Full multiplicity and fully clamped padding are now checked separately.
  The [native split repair](../native-exact-splits.md) now copies those full-order
  blocks safely; Bezier preparation uses it directly. Refinement retains its
  exact Euclidean prefix/suffix payload copy to avoid homogeneous projection
  roundoff. Other endpoint clamping uses the repaired native producers directly.
  The shared portable representation policy also avoids adding a repetition
  to an already full-order group beside an outer knot. Regression coverage
  includes both ends, both tensor axes, insertion/no-ops, exact elevation,
  homogeneous degree fitting, degrees through 17, binary chart scales and
  common weight gauges.
  These arbitrary full vectors are qualified independently, rather than
  claimed as OpenNURBS/Rhino serialization parity cases. Source:
  [NL_CrvTool.cpp](../../third_party/usd-brep/source/SMLib/src/NL_CrvTool.cpp) and
  [regression](../../crates/viboceros-smlib/tests/inactive_endpoint_controls.rs).

- **Fixed: unclamped active-end splitting could overwrite native buffers.**
  `N_BasisFindSpanAndMult` with `NL_LEFT` reports multiplicity `degree + 1`
  whenever the query equals `U[m - degree]`, even for an unclamped vector.
  The original `N_CrvSplit` used that value to size its output polygons but copied more
  controls into the left polygon. The quadratic fixture with knots
  `[-2,-1,0,0.8,2,3,4]`, split at active end `2`, aborted with heap corruption
  during `N_EndNurbs`. The debugger located the abort in stack cleanup; source
  inspection identified the undersized left-output polygon in this split path.
  The repaired splitters use actual multiplicities and exact parameters.
  Endpoint refinement now calls them directly, removing both reversal wrappers.
  The surface producer also creates full-order outputs instead of returning
  success without them, and checks/updates null-stack preallocated capacities.
  [Qualification](../native-exact-splits.md) covers both endpoints and tensor axes.
  See [knot refinement](../smlib-knot-refinement.md)
  and [NL_CrvTool.cpp](../../third_party/usd-brep/source/SMLib/src/NL_CrvTool.cpp).
  [MakeNonPeriodic](../smlib-nonperiodic.md) shares the direct endpoint path.
