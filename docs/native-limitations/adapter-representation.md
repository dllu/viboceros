# Adapter charts, representation and resource limits

[Limitations ledger](../usd-brep-limitations.md) · [Native modeling](../native-modeling.md)

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
  mixed-range or conditioning guarantee. See [preview preparation](../smlib-morph-preview.md).

- **Fixed: isocurve chart conditioning rejected subnormal near-boundary queries.**
  Planar trimmed-face closest-point and density paths can supply nonzero
  subnormal parameters or knots next to zero. Scaling a moderate fixed-axis
  chart downward by its maximum knot then lost those exact values and rejected
  two ExtractIsocurve hole regressions. The isocurve adapter now chooses a
  smaller reversible binary scale that includes both knots and the requested
  parameter. This changes no source coordinates, spans or tolerances. Native
  subnormal tests and both picked/density hole regressions pass with the repair;
  arbitrary extreme span/weight ratios remain unqualified. See
  [native isocurve tests](../../crates/viboceros-smlib/tests/isocurves.rs).

- **Fixed: isocurve Split edges kept local charts in public output.**
  Seven of eight fresh Split command captures differed only in their 3D edge
  domains: our rectangular-face builder intentionally used lossless local
  edge charts, while Rhino retained the original surface-axis intervals.
  Isocurve Split now uses a constructor variant that translates every edge knot
  back exactly, including reversed sides, while still evaluating in the local
  frame. Geometry, UV trims and tolerances are unchanged. The general builder
  retains its existing policy. This was command output metadata, not a usd-brep
  geometry defect. See [surface extraction](../smlib-surface-segments.md).

- **Whole-domain surface trimming retains unused full outer knots.**
  The deliberate whole-domain trim preserves the original full U/V vectors
  `[-2,-1,0,0.8,2,3,4]`. Rhino reconstructs their artificial outer slots as
  `[-1,-1,0,0.8,2,3,3]`. Every stored knot and control matches, and independent
  9×9 evaluation grids match within `1e-10`. The complete raw definition
  discrepancy is retained in the surface extraction comparison, extending the
  already documented curve no-op padding difference. No comparison-time
  normalization hides it. See [surface extraction](../smlib-surface-segments.md).

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
  [surface extraction tests](../../crates/viboceros-smlib/tests/surface_segments.rs)
  and [Pipe tests](../../crates/viboceros-command/src/pipe.rs).

- **Surface seam closure retains its existing homogeneous tolerance policy.**
  Both backends reject the tested periodic cubic surface after a common weight
  scale of `1e-300`: its homogeneous interior samples are indistinguishable
  under the inherited absolute coincidence tolerance. This happens before
  native refinement or extraction. The new adapter retains that validation,
  including rejecting nets that close only projectively. This is a shared
  closure-policy restriction, not evidence of a usd-brep arithmetic defect.
  The native regressions qualify common scales `1`, `1e100` and `-1e100`; wider
  gauges are not certified by surface seam integration. See
  [surface seam regressions](../../crates/viboceros-smlib/tests/surface_seams.rs).

- **Numeric-length extraction retains the existing sampler's accuracy policy.**
  Native SubCrv length/midpoint extraction uses usd-brep trimming, while length
  inversion and closed-seam preparation retain the shared policy. All 20 signed
  SDK records match at the prior `1e-6` comparison ceiling; five differ at a
  stricter `1e-8` ceiling, with residuals up to `7.99e-8`. Native versus portable
  piece tests retain the same endpoints and geometry. The raw diagnostic is
  preserved separately, rather than attributed to an upstream trimming bug.
  General length inversion is not continuously certified. See
  [qualification](../smlib-length-segments.md).

- **Native Bezier decomposition uses bounded gauges and resources.**
  ConvertToBeziers and ConvertToSingleSpans accept degrees through 17, with
  one million input/aggregate output controls. Directional strips normalize
  each independent control line, preserving gauges such as `1e-280` and
  `1e280` on different rows. Both keeps intermediate controls homogeneous
  under one common binary gauge; wider overall weight ratios remain unsupported.
  Final controls at infinity, nonfinite coordinates and underflowed restored
  weights return the existing typed unrepresentable-control error. These are
  adapter restrictions, rather than established upstream geometry defects.
  See [qualification](../smlib-bezier.md).

- **Multiple-knot removal uses ordered refits and sampled crease selection.**
  RemoveMultiKnot shares native homogeneous fitting with RemoveKnot rather than
  adding another control solver. Groups are refitted in descending parameter
  order; surfaces finish U before reevaluating V candidates. Roundoff can
  accumulate through long sequences, and the shared transverse angle samples
  do not certify a continuous maximum. Approximation deviation is also
  uncertified. These are adapter/command-policy qualifications, not reproduced
  upstream geometry bugs. See [qualification](../smlib-multiple-knot-removal.md).

- **Native raw NURBS edits have bounded input and output resources.**
  InsertKnot, MakeNonPeriodic, ChangeDegree, RemoveKnot and RemoveMultiKnot use
  NLib directly, with the
  pinned degree-17 limit and a one-million-control cap. A common binary weight
  scale conditions homogeneous inputs; every exported Euclidean coordinate
  and restored weight must be finite and every weight nonzero. Extreme weight
  ratios can still fail, and degree reduction/deformable interpolation has no
  certified deviation bound. No-op conversions preserve the original payload.
  These are adapter limits, not evidence that the upstream mathematics is
  invalid. See [refinement](../smlib-knot-refinement.md),
  [degree changes](../smlib-degree-change.md) and their native regressions.

- **Upstream exact elevation rejects full-order interior breaks; the adapter composes pieces.**
  `N_CrvElevateDegree` failed on a degree-two curve with six controls and knots
  `[0, 0, 0, 0.5, 0.5, 0.5, 1, 1, 1]` when raised to degree four. The native
  [direct probe](../../tools/usd_brep/piecewise_elevation_probe.cpp) retains status
  `1` for that whole-curve call and status `0` for both independent pieces.
  The adapter now reuses native elevation per contiguous full-order block,
  keeps intermediate controls homogeneous, and verifies the assembled target
  basis. Native-owned copying respects the constructor's borrowed arrays.
  Curve, tensor and native TweenSurfaces Refit cases now succeed; the portable
  elevation path still rejects the tested discontinuity. This is an upstream
  producer restriction with an adapter workaround, not evidence of invalid
  accepted geometry. See [qualification](../smlib-piecewise-elevation.md) and
  [regressions](../../crates/viboceros-smlib/tests/degree_change.rs).

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
  [qualification](../smlib-translational-sweep.md) and
  [retained raw comparison](../smlib-translational-sweep-comparison.json).

- **Fixed: tolerance snapshots preceded native UV creation.**
  `GetOrCreateUVTrimCurve` can increase edge and vertex tolerances. The exporter
  previously copied those fields first, then copied the UV trims, producing
  inconsistent uncertainty and a rejected planar gap join. Export now copies
  tolerances after creating every trim; it uses the native values directly.
  The gap-stitching regression covers this without blanket tolerance inflation.

- Native features currently require Linux and system oneTBB. The USD scene
  layer and upstream Python packaging are not part of our kernel-only build.

- B-rep transfer requires continuous NURBS and manifold topology. The adapter
  now accepts [unclamped inputs](../smlib-unclamped-import.md) through native
  canonical clamping of their active domains; output control layouts can differ.
  Mixed open/closed imports, nonmanifold sheets and unsupported singular or
  periodic topology can be rejected. Native face reversal can swap UV axes.
  See [transfer](../smlib-brep-transfer.md) and [sheet import](../smlib-offset-surfaces.md).

- Exact export rejects native surface/curve classes requiring approximation.
  Offset export requests an explicit native NURBS approximation bound and checks
  the achieved bound; this is not a proof of the entire output boundary.
