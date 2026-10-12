# Native closed-curve seams

Linux builds with `native-smlib` use usd-brep for the refinement or segment
extraction needed by CrvSeam, closed numeric/midpoint SubCrv preparation, and
cutting Split/Trim. Analytic leaves retain their types; partial NURBS leaves in
polycurves use the shared native extractor.

`NurbsCurve::try_change_closed_seam_with_operations` shares the existing seam
policy with the portable backend. Simple periodic seams use native InsertKnot
before cyclic control rotation and knot translation. Multiple periodic knots,
clamped curves and wrapped parameters use two checked native segment extractions
before the existing homogeneous append. Endpoint no-ops and period translations
remain exact. The adapter adds no refinement, evaluation or interpolation solver.
The supplied split callback receives the periodic topology policy.
[Surface seams](smlib-surface-seams.md) now share the rotation and append helpers
after validating closure and periodic topology for the complete control net.

The native entrypoint is `viboceros_smlib::change_curve_seam`. Command preparation
propagates native errors through the same typed conversion used by knot edits and
segment extraction. Sources are immutable and native handles never enter document
history. The shared callbacks also keep previews and accepted SubCrv output on
the same backend.

The [native seam regressions](../crates/viboceros-smlib/tests/curve_seams.rs)
compare complete knot vectors, degrees, periodicity, Euclidean controls, weights
and 65 parameter samples against the portable reference. They cover the 11 SDK
fixture definitions, binary chart scales `2^-360`, `1`, `2^360`, common weight
scales `1e-300`, `1`, `1e300`, `-1e300`, and parameters one period on either side
of the source domain. Invalid/open requests and exact start-seam no-ops are
retained. A geometry policy regression checks that closed length preparation
calls the supplied seam operation before final extraction without changing its
source.

Fresh [Rhino 8.32 SDK captures](../tools/rhino_oracle/observations/curve_change_seam.json)
contain all 11 seam fixtures, collected on isolated Xvfb under private scheme
`VibocerosOracleSmlibBridgeFinal20261010`. Every complete native definition matches
at absolute `1e-10` and relative `1e-12`; the
[raw comparison](smlib-curve-seams-comparison.json) retains each result and the
fixture/observation SHA-256 hashes. The additional 327 retained editing,
decomposition, splitting and intersection records also match at their existing
tolerances, including the established `1e-6` ceiling for numeric-length pieces.
Captured timings include the harness and do not establish kernel speedups.

The geometry/command suites and complete native bridge suite pass 3,568 tests
(11 ignored). Four native SubCrv preview tests pass. Workspace lint passes in
both feature configurations with warnings denied, and formatting passes.
Geometry comparisons allow native projection roundoff; requested anchors,
full seam knot vectors and history snapshots retain exact comparisons.

Native refinement and extraction retain their existing
[limits and workarounds](usd-brep-limitations.md), including endpoint conditioning
and the full-order split defect. These finite cases do not certify arbitrary
weight ratios, chart ratios or continuous parity. Tensor seam qualification is
documented separately.

```sh
CMAKE_BUILD_PARALLEL_LEVEL=10 cargo test --release -p viboceros-smlib \
  --features native --test curve_seams
```
