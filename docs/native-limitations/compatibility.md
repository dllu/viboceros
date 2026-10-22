# Observed Rhino compatibility differences

[Limitations ledger](../usd-brep-limitations.md) · [Native modeling](../native-modeling.md)

- **Fixed: strict native solves rejected in-tolerance skew curve contacts.**
  Fitted curves can have a nonzero gap below the requested contact tolerance.
  Both strict Newton attempts could stop without arithmetic coincidence, and
  their bounded witnesses were discarded. A final independent spatial check
  now accepts the better witness inside both requested intervals and the
  existing distance budget. The preceding producer misses 60 of 252 direct
  cases; the repair passes all 252 normally and under AddressSanitizer. Five
  fresh Rhino `Intersect` recipes cover accepted and rejected gaps. See
  [the qualification](../native-curve-contact-tolerance.md).

- **Curve intersection equation convergence and parameter merging can miss
  nearby crossings.** The original global solver reports no contacts for
  `C(t)=(2t,4t(1-t),0)` against `Y=1-1e-10` at tolerance `1e-12`. Small normal
  equations can stop before physical coincidence; generic duplicate comparison
  also uses `1e-4` of a parameter domain. Shallow off-center tangencies can exhaust
  endpoint/midpoint guesses. Generated scoped repairs add physical convergence,
  separate tangent retries, physical point comparison and quarter guesses.
  Shared source-space polishing removes rounded multiple-knot tangent families.
  [Qualification](../smlib-curve-intersections.md) retains original and patched
  diagnostics, ASan, independent analytic cases and saved SDK replays. The pinned
  kernel stays unchanged; general event completeness remains unqualified.
  Source: [SmCurve.cpp](../../third_party/usd-brep/source/SMLib/src/SmCurve.cpp).

- **Whole-domain curve trims preserve source outer entries in Viboceros.**
  The raw `trim-rational-whole` record has source outer values `-2` and `4` in
  Viboceros and padded neighboring values `-1` and `3` in Rhino. OpenNURBS
  omits those two full-vector slots; all stored knots and controls agree.
  The native adapter preserves the existing exact no-op policy. The primary
  raw comparison retains this mismatch; independent reference-definition
  sampling checks the same active locus. No comparison-time knot normalization
  hides it. See [qualification](../smlib-curve-segments.md).

- **Dedicated knot removal differs from Rhino's interpolation rule.**
  `N_CrvRemoveKnot` uses a tolerance-bound removal algorithm. On the retained
  cubic-middle fixture, tolerance `1e10` removes one knot but leaves a maximum
  control-coordinate difference of `3.473170830538584` from Rhino. The adapter
  instead shares the existing native homogeneous interpolation producer with
  ChangeDegree. This is a command compatibility difference, not an upstream
  geometry-invalidity claim. See [qualification](../smlib-knot-removal.md) and
  [direct native probe](../../tools/usd_brep/knot_removal_probe.cpp).

- **Native edge-surface exact matching retains a near-knot distinction.**
  In the two-multispan ruled fixture, domain mapping produces knots `0.3` and
  `0.30000000000000004`. The native adapter retains both, giving five profile
  controls versus Rhino's four. All captured 13×13 surface coordinates agree
  within absolute `2e-12`, relative `1e-14`. The complete raw difference remains
  in [edge-surface qualification](../smlib-edge-surfaces-comparison.json); no
  comparison-time knot merging or coefficient filtering is applied.
  This is an adapter representation policy, not an established upstream geometry bug.

- **Fixed: numeric extrusion direction was an application policy difference.**
  The live short-span XZ profile is extruded along positive Y by Rhino, while
  the application's numeric ExtrudeCrv path used global Z. The command now uses
  native classification of parallel profile directions, positive dominant-axis
  orientation and active CPlane fallback. This was an app command-default gap,
  not a demonstrated usd-brep geometry bug. See
  [direction qualification](../smlib-extrusion-direction.md).

- **Short-span revolution has a different V domain.** The new live command
  uses V `[0, 10.837476096278881]` in Rhino and `[0, 1]` in Viboceros. The tensor
  controls, weights, U basis and full-degree V knot-group multiplicities match,
  describing the same rational Bezier patch images. Raw V differences remain
  in the primary comparison; parameter/derivative parity is not claimed. See
  [retained comparison](../smlib-modeling-basis-comparison.json).

- **Portable planar overlap policy differs from current Rhino.** The live
  fixture with an outer rectangle, two competing inner rectangles and a separate
  inner hole produces areas 83 and 16 in both Rhino and usd-brep. The previous
  custom command policy detached both competing holes and produced areas 99, 16
  and 16. Native PlanarSrf now uses the kernel region constructor directly;
  portable builds retain the older policy pending correction. See
  [planar qualification](../smlib-planar-surfaces.md).

- **Native Join is not raw Rhino parity.** The
  [92-case replay](../smlib-join-comparison.json) has no native execution errors but
  only 14 complete raw matches. Stored uncertainty, UV domains, face senses,
  geometry and ordering still differ. Even disconnected fresh outputs can gain
  model-sized native tolerances and edge-aligned UV parameter domains while
  retaining their spatial definitions. Broader geometry parity remains open.

- **Closed-box wall offsets differ from Rhino's SDK.** SMLib constructs wall
  volumes for the tested closed-box Solid requests, while Rhino returned no
  bodies. Positive box offsets also have 12 edges versus Rhino's 36. Geometric
  measurements and partition differences are tracked separately in
  [offset qualification](../smlib-offset-surfaces.md) and its
  [partition ledger](../smlib-offset-partitions.json). Command-level semantics and
  broader self-intersecting/collapsing offset behavior remain unqualified.
