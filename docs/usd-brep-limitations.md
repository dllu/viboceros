# USD BRep limitations and bugs

This ledger tracks findings relevant to Viboceros's pinned usd-brep 1.0.0
(`c9c5979f04cea8c38213004d4f04df0dcfb40d05`). It distinguishes upstream
behavior from restrictions in our adapter. See [integration](usd-brep.md) for
setup and [upstream known issues](../third_party/usd-brep/KNOWN_ISSUES.md) for
issues reported by NVIDIA that have not all been reproduced here.

## Native API behavior verified during integration

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
