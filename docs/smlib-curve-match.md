# Native curve Match preparation

Linux `native-smlib` builds use usd-brep structural preparation for Match and
AverageCurves, including the `curve_match_geometry` oracle. The bridge exposes
`match_curve_end`, `average_match_curve_ends` and `interpolate_affine_greville`.
`CurveMatchOperations` keeps target selection, G0/G1/G2 endpoint controls,
rational weight formulas, far-end preservation and independent continuity
checks in the shared geometry layer.

Position matching uses the checked native curve extractor for trimming toward
the target, then retains the original output domain. Single-span elevation uses
the existing native degree adapter. Short multi-span sources use native knot
refinement to separate matched and preserved end-control sets. The measured
five-control cubic and short quadratic G2/G2 policies reuse native uniform
Greville interpolation at affine source parameters, rather than arc-length
Rebuild. Source weighting, polynomial preparation, curvature coefficients,
handle lengths and conditional output domains retain their established rules.

Affine preparation shares the existing normalized Rebuild targets and native
banded producer; endpoints retain their exact sampled payloads. Both Rebuild
and Match now use one interpolation adapter. Structural callbacks validate
requested degree/domain, refinement progress and the six-control polynomial
layout before control edits. Source curves remain immutable; command output
replacement and AverageCurves remain one atomic history step.

All [160 full-record comparisons](smlib-curve-match-comparison.json) match the
saved pre-change producer at absolute `1e-8`, relative `1e-12`. Maximum numeric
difference is `6.89e-15`. The corpus includes single/multi-span G0/G1/G2,
averaged ends, preserved far curvature, short polynomial/rational preparation
and translated homogeneous-distance diagnostics. Native regressions compare
complete bases, controls, weights and 65 parameter samples for 59 representative
cases, while shared-policy checks inspect elevation and reject invalid layouts.

The same records have 159 complete raw matches against retained Rhino captures
at absolute `1e-6`, relative `1e-12`. The established average-position boundary
case, where Rhino retains a short parameter tail, remains in the primary
comparison. No comparison-time normalization removes that discrepancy.

The complete optimized geometry/command/bridge suite passes 3,641 tests (11
ignored), including Match output, averaged replacement, preservation and history
checks. The additional public affine-budget guard test also passes, bringing
verification to 3,642 distinct tests. Workspace lint passes in both feature
configurations with warnings denied; formatting passes.

Native basis editing inherits the degree-17, chart, representability and
resource restrictions documented in the [ledger](usd-brep-limitations.md).
Affine interpolation keeps the existing 1,000-control preparation ceiling.
Other builds retain their portable producers. Broader arbitrary-input matching
and fresh SDK parity remain unqualified; new captures are still unavailable
pending the Wine licensing startup in [isocurve qualification](smlib-isocurves.md).

```sh
CMAKE_BUILD_PARALLEL_LEVEL=10 cargo test --release -p viboceros-smlib \
  --features native --test curve_match --test curve_rebuild
```
