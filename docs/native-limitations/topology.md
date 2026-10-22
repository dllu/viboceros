# Native topology, transfer and runtime

[Limitations ledger](../usd-brep-limitations.md) · [Native modeling](../native-modeling.md)

- **Fixed in the Join adapter: closed remote charts failed normal orientation.**
  Ray-based orientation returned status 1006 after stitching a valid six-sheet
  box. The adapter now uses native topological normal unification for all
  assemblies; the existing command-level exact spatial witness chooses outward
  sense for each provable newly closed component. Unknown closed assemblies
  retain the previous native ray path through one bounded retry. Eight bridge
  cases and 64 complete
  Join/JoinCopy workflows retain the source charts and geometry. The upstream
  ray query path itself remains unqualified. See [scope and control runs](../native-closed-join.md).

- **Planarity includes topology-zone inflation.** Exact planar NURBS can fail
  `SmSurface::IsPlanar` below their declared zone tolerance because its bounding
  box expands by that zone. The [generated repair](../../third_party/patches/usd-brep-planarity-zone.patch)
  measures the unexpanded control support and retains the requested tolerance.
  The direct rotated-plane/warped-surface probe and
  [coplanar merge regressions](../coplanar-faces-qualification.json) cover this.
  A [second repair](../../third_party/patches/usd-brep-protected-topology.patch)
  checks protected edges before `IsTopological` can expand their surfaces, and
  protected vertices before isolated-loop cleanup. Coplanar merging protects
  surviving boundaries while allowing removed interface vertices to disappear.

- **Offset intersection cleanup can retain deleted topology.**
  Shell failure tests exposed three temporary-topology lifetime errors:
  combining edges records the wrong edge at changed endpoints; a closed edge
  destructor removes only one of its two incidences at a shared endpoint; and
  common-edge processing reuses a vertex consumed by a prior squeeze. The
  [generated-source repair](../../third_party/patches/usd-brep-proto-edge-endpoint-links.patch)
  preserves those endpoint relationships and uses surviving vertices. A fourth
  error promotes isolated-loop vertexuses onto a loop that is then deleted.
  The [loop-owner repair](../../third_party/patches/usd-brep-isolated-loop-owner.patch)
  assigns them to the selected surviving loop and its mate. The pinned
  third-party checkout is unchanged. See the [Shell qualification](../shell-qualification.json),
  [cleanup baseline](../shell-asan-baseline.log),
  [squeezed-vertex baseline](../shell-asan-squeezed-vertex-baseline.log) and
  [isolated-loop baseline](../shell-asan-isolated-loop-baseline.log).

- **Intersection-curve trimming can accept an off-curve vertex projection.**
  `SmEdgeDefinition::IsInteriorPoint` checked that nearest-point projection
  succeeded but ignored its model-space distance. A boundary-crossing lamp
  cutter returned native edge/vertex gaps up to `4.7084e-3` against stored zone
  radii near `1e-9`, before copying or portable extraction. The merged
  [lamp-well regression](../native-lamp-well.md) verifies a bounded
  [source patch](../../third_party/patches/usd-brep-interior-point.patch) requiring
  finite distance within the existing tolerance before authorizing a trim.
  CMake applies it to a generated source copy; the pinned submodule is unchanged.
  Both Boolean postprocessing modes pass original native endpoint-zone checks,
  strict portable validation and 3DM roundtrip. Legal additive native zone radii
  are translated into local portable bounds and checked against every original
  incident edge; geometry and validation tolerances are not moved globally.

- **Point-set normal queries can replace a previous classification (source audit).**
  `SmPointSet3d::IsPlanar` uses its sequential classifier, while `GetNormal`
  calls `SetUpAxes`, which reruns best-fit classification and replaces the
  cached type. `SetUpAxes` also ignores the fitter's return status. The extrusion
  adapter rechecks the type after requesting the normal and requires a finite,
  unitizable vector. No fixture inducing a hidden fitter failure has been retained;
  these convenience queries are not independent geometry-validity certificates.
  Source: [SmPointSet.cpp](../../third_party/usd-brep/source/SMLib/src/SmPointSet.cpp)
  and [numeric extrusion](../smlib-extrusion-direction.md).

- **End-cap convenience helper leaves final stitching status unchecked.**
  `SmBrep::CreateEndCapsFromSurface` can return success after calling
  `StitchAndOrient` without checking the result. The apex adapter follows the
  call with checked stitching and manifold status, then validated B-rep export.
  Curve-path extrusion now shares that adapter helper.
  No fixture inducing the ignored internal failure has been retained. Its cap
  planarity checks also use the default `IsPlanar` tolerance; general nearly
  planar cap behavior remains unqualified. See [apex extrusion](../smlib-apex.md).

- **Planar-face tolerance argument is ignored by the kernel entry point.**
  `SmApiCreatePlanarFaces` passes a hard-coded `0.01`, but
  `SmBrep::CreatePlanarFacesWith3DCurves` ignores that argument and uses the
  receiving B-rep's tolerance. The bridge configures its private B-rep before
  calling the existing constructor. See [planar surfaces](../smlib-planar-surfaces.md).

- **Curve-intersection wrapper fixes tolerance at `0.001` (source audit).**
  `SmApiIntersectCurves` also reports `SM_ERR` for empty successful solves.
  Callers needing model-specific tolerance must use the existing native
  `GlobalCurveIntersect` entry point. This wrapper was inspected during planar
  integration; the final planar command needs only native plane grouping and
  region construction. Source:
  [SmApiIntersectors.cpp](../../third_party/usd-brep/source/SM_API/src/SmApiIntersectors.cpp).

- **Linear-sweep final stitching status is unchecked (source audit).**
  The analytic/simple sweep paths call `StitchAndOrient` without checking its
  return value before reporting success. This is not a validity certificate.
  The bridge rejects coplanar capped offsets, requires a native manifold solid
  for caps, and validates exported B-reps. No fixture inducing an ignored stitch
  failure has been retained. Source:
  [SmPrimitiveCreation.cpp](../../third_party/usd-brep/source/SMLib/src/SmPrimitiveCreation.cpp).

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
  Source: [SmStitch.cpp](../../third_party/usd-brep/source/SMLib/src/SmStitch.cpp).

- **Extreme parameter intervals break stitching despite spatial normalization.**
  An exploded box at scale `2^-360`, spatially normalized to unit extent while
  retaining its original knots, returned `SM_ERR_NULL_POINTER` (1006). Mapping
  edge and surface domains to `[0,1]`, with matching UV-trim mappings, allows
  that fixture to stitch. The
  [regression](../../crates/viboceros-smlib/tests/stitching.rs) covers `2^-360`, unit
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
  [export](../../crates/viboceros-smlib/native/brep_export.cpp) and the rational-sheet
  stitching regression.

- **Gap refresh can conceal earlier failures (source audit).**
  `SmBrep::RefreshGaps` checks only the last return value in each of its edgeuse
  and vertexuse loops; `RefreshTols` ignores the return value from `RefreshGaps`.
  A successful last item can therefore conceal a previous failure. No fixture
  inducing that failure sequence has been retained. These methods must not be
  treated as validity certificates. Source:
  [SmBrep.cpp](../../third_party/usd-brep/source/SMLib/src/SmBrep.cpp).

- **Mass-property accuracy is clamped by SM_API.** The public B-rep property
  wrapper clamps requested relative accuracy to at least `1e-8`. Branch volume
  checks use the existing native face integrator with explicit accuracy instead.
  See [BooleanSplit integration](../smlib-boolean-split.md) and
  [bridge](../../crates/viboceros-smlib/native/bridge.cpp).

- **Shared context requires serialization.** Default-context calls share
  mutable state. Our C boundary holds a process-wide mutex, including handle
  destruction. This prevents parallel kernel work in one process; isolated
  worker processes remain an option. See
  [upstream concurrency guide](../../third_party/usd-brep/.agents/operations/concurrency.md).

- **Standard ARM64 bootstrap dependency unavailable in the trial.** Upstream
  Packman's pinned `3.12.15+nv1-manylinux_2_35_aarch64` download failed. The
  kernel-only CMake build avoids that dependency. This observation does not
  establish current availability of upstream's package server. See
  [native trial](../usd-brep.md#reproduce-the-native-trial).
