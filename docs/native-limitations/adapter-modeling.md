# Adapter contact, interpolation and fitting policies

[Limitations ledger](../usd-brep-limitations.md) · [Native modeling](../native-modeling.md)

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
  See [surface qualification](../smlib-surface-surface-intersections.md) and
  [trim qualification](../smlib-surface-trim-intersections.md) and
  [tangency qualification](../smlib-surface-trim-tangencies.md).

- **Native curve/surface contacts retain bounded preparation and acceptance.**
  Curve/B-rep queries share the underlying-surface producer and original-face
  trim policy. Unsupported degree, full-order interior breaks, weight ratios
  and collapsed frames retain the portable query. Original-geometry point
  residuals and per-span overlap samples reject invalid native proposals, but
  do not certify continuous coincidence, arbitrary conditioning or event
  completeness. See [qualification](../smlib-curve-surface-intersections.md).

- **Curve self-contact search has bounded qualification.** IntersectSelf and
  SelSelfIntersectingCrv use the native global producer for eligible curves,
  preserving source-space acceptance and portable preparation fallback. A
  generated repair removes mirrored span pairs before solving; without it, a
  backtracking range can span both visits across its turnaround. Analytic
  contacts and basic command behavior are checked separately from the retained
  nested-collinear Rhino command/API differences. No completeness, continuous
  overlap or kernel-speed certificate is claimed. See
  [self-intersection qualification](../smlib-curve-self-intersections.md).

- **Curve Match retains measured continuity policy around native preparation.**
  Trim-to-target, exact elevation, end-control refinement and six-control affine
  Greville interpolation now reuse native adapters. Shared G0/G1/G2 and rational
  curvature rules, far-end preservation and continuity audits remain intact.
  All 160 pre-change records match; 159 match retained Rhino captures completely.
  The existing average-position boundary tail discrepancy remains in the raw
  comparison. Native degree/chart limits and the 1,000-control affine budget
  apply, with broader arbitrary matching and fresh captures unqualified. See
  [qualification](../smlib-curve-match.md).

- **TweenCurves retains direct blending/refit policy around native sampling.**
  Nonlinear sampled sources now use the shared native open cubic adapter,
  including 10,000 stations and 10,002 output controls at 9,999 divisions.
  The extended native allowance applies only to open handle systems with four
  channels; other interpolation layouts retain their existing limits. Public
  InterpCrv/curve-through point ceilings stay at 256. ControlPoint and adaptive
  Hermite Refit remain direct constructions, and linear sampled sources stay
  degree one. Arbitrary conditioning, continuous agreement and performance
  relative to Rhino remain unqualified. See [sampling qualification](../smlib-curve-tween.md).

- **Sweep1 and Pipe retain model policy around native interpolation.**
  Rail clamping, section-site refinement, adaptive fitting and homogeneous
  section solving now use the existing native adapters. Equal parameter rows
  require two fixed identity constraints; arbitrary duplicate interpolation
  stations remain invalid. Frame transport, rational blending, rail-weight
  queries and independent profile/seam audits stay shared. The 4,096/512-axis
  and 262,144-surface control budgets remain in force. All 63 pre-change sweep
  records match, including existing diagnostic and closed-circle cases;
  broader Rhino basis/domain and off-surface query differences remain open.
  See [native sweep interpolation](../smlib-sweep-interpolation.md).

- **FitCrv retains adaptive and fixed-handle policy around native solving.**
  Its endpoint and internal kink handles are control constraints, so the adapter
  supplies identity rows alongside native Greville basis rows. Cubic systems
  keep five-wide storage; other degrees use native banded LU instead of the
  application's reduced dense solve. The fixed-row adapter allows 4,096 stations,
  while other native interpolation jobs retain their previous bounds. FitCrv
  keeps the existing 4,096/512 control budgets and sampled acceptance policy;
  no continuous error bound or arbitrary-conditioning qualification is claimed.
  Internal Sweep1/Pipe fitting now shares the native producer. Fresh Rhino fitting captures remain
  pending Wine licensing startup. See [native FitCrv](../smlib-curve-fit.md).

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
  model geometry. See [native preview preparation](../smlib-morph-preview.md).

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
  See [native offsets](../smlib-curve-offset.md) and
  [native curve intersections](../smlib-curve-intersections.md).

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
  [native morph interpolation](../smlib-morph-interpolation.md).

- **Point-grid interpolation retains continuation and conditioning limits.**
  Open directions use the existing native banded solver; closed directions fold
  repeated control columns, reuse alternate-end cyclic band ordering for
  larger grids, and retain full partial-pivot LU for small/failed band jobs.
  Closed point-grid channels share `N_RealMatrixRightForBack` scalar solves (no
  small-RHS skip), retaining every channel solve and residual check before
  the shared constant-channel correction between axes. The native
  zeroth-derivative basis supports boundary-span polynomial continuation; the
  [direct probe](../../tools/usd_brep/grid_continuation_probe.cpp) returns the exact
  independent quadratic coefficients `[9/8,-1/4,1/8]` at `-0.5` outside active
  domain `[0,5]`. [Retained output](../smlib-point-grid-probe.json) records this
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
  [native point grids](../smlib-point-grid.md).
  A constant-channel solve bypass changed the 64×7 degree-11 diagnostic's
  extrapolated construction samples by `1.17e-8` versus the existing producer,
  exceeding its `1e-8` allowance even with the open-axis solver unchanged.
  That bypass is not used: every channel retains its native solve/residual
  check before shared constant correction. This is an adapter qualification
  limit, not proof that an upstream solver is incorrect.
  Arbitrary conditioning and Rhino performance remain unqualified.
  See [native point grids](../smlib-point-grid.md) and the existing narrow-span and
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
  [native TweenSurfaces](../smlib-surface-tween.md).

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
  are not native qualifications. See [surface Rebuild](../smlib-surface-rebuild.md).

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
  [native Rebuild](../smlib-curve-rebuild.md).

- **Curve Blend shares the native exact endpoint interpolator with BlendSrf.**
  Native builds use derivative basis rows and homogeneous back solving for
  independent G0/G1/G2 constraints. Picking, tangent/curvature queries, default
  handles, bulges and domain policy remain shared; G0/G0 retains its direct line.
  Portable builds use the same common endpoint interface. The Python oracle can
  select `native_smlib=True`, and all 282 retained Rhino SDK definitions preserve
  degree, weights and control coordinates within `1e-12`, with the existing
  arc-length domain ceiling of `5e-8`. Native command history and readonly source
  behavior are checked. Interactive handles, G3/G4 and general conditioning
  remain unqualified. See [native curve Blend](../smlib-curve-blend.md).

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
  numerical bug. See [native BlendSrf preparation](../smlib-surface-blend.md).

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
  All 42 [pre-batch native definitions](../smlib-make-periodic-batch-comparison.json)
  match with zero numeric difference. The tested rational tensors run about
  twice as fast in the [diagnostic benchmark](../smlib-make-periodic-surface-performance.json);
  this is not a Rhino timing claim.
  Independent rational constraints cover both surface axes, extreme signed
  weight gauges and tiny nonzero channels. Fresh Rhino capture, arbitrary
  conditioning and performance relative to Rhino remain unqualified. See
  [MakePeriodic integration](../smlib-make-periodic.md).

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
  [qualification and comparison](../smlib-curve-interpolation.md).

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
  [NL_SrfAdv.cpp](../../third_party/usd-brep/source/SMLib/src/NL_SrfAdv.cpp) and
  [section matching](../smlib-section-bases.md).
