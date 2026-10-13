# Native TweenSurfaces

Linux `native-smlib` builds use usd-brep operations in all three TweenSurfaces
modes, including command preparation and readonly previews. The geometry layer
retains source ordering, U/V correspondence edits, positive-weight admission,
output/control budgets, rational displacement, knot/domain policies and history.

- Control matching retains compatible source nets directly. For unequal degrees
  or counts it uses native isocurves and batched banded Rebuild, forcing open
  preparation even for closed sources. Both prepared nets are rebuilt before
  every successive output, preserving the measured drift.
- Refit delegates clamping, degree elevation and exact knot insertion to the
  existing native adapters. The shared maximum-multiplicity union keeps distinct
  adjacent knots, first-source weights and end-source output domains.
- SamplePoints retains normalized UV source samples and exact scalar blending,
  then delegates each open mean-chord point grid to native basis evaluation and
  batched banded solves. The independent tensor evaluator checks every input
  station before accepting an output.

`NurbsInterpolationSystem` is the shared open/uniform-periodic job;
`UniformInterpolationSystem` remains a compatibility alias. Surface Rebuild and
point-grid interpolation share the two-axis batch layout, finite/layout checks
and exact constant-channel preservation. Native open grids prepare knots and
stations without building the portable dense basis matrices.
[Closed point-grid continuation](smlib-point-grid.md) now shares the native
basis and pivoted solver; sampled TweenSurfaces continues to use open grids.
No new basis evaluator or linear solver is added.

The bridge exposes `surface_tweens`, `surface_tweens_refitted`,
`surface_tweens_sampled` and `interpolate_open_point_grid`. Native regressions
compare all requested outputs from 104 retained command captures against Rhino
and the portable model, covering weighted controls, unequal nets, repeated
preparation, knot/domain differences and all matching modes. Grid tests cover
degrees through 11, nonuniform mean chords, the 256×256 limit and independent
interpolation stations. Tiny-channel sampling retains detail down to `1e-280`.
Shared-policy tests count repeated preparations, bypass compatible nets and
reject invalid backend layouts without changing sources.

All [104 full-record comparisons](smlib-surface-tween-comparison.json) match the
same-source portable backend at absolute `1e-9`, relative `1e-12`; maximum
numeric difference is `1.60e-14`. The 26 sampled records also match the saved
pre-change producer. Requested Rhino output sets match in all 104 records at
absolute `1e-6`, relative `1e-10`; complete raw output lists match in 92 records.
All input/observation and audit hashes are retained in the comparison.

The complete optimized geometry/command/bridge suite passes 3,622 tests (11
ignored), including all matching modes, command properties, previews, source
retention and history. Workspace lint passes in both feature configurations
with warnings denied; formatting passes.

Twelve earlier option-change captures contain extra source-shaped copies in
addition to their requested outputs. The raw comparison retains this established
count/geometry discrepancy; requested output comparisons use the captured suffix
and are labeled separately. It is not a newly reproduced usd-brep defect.

The existing sampled API probe selects the native backend.
`surface_tween_matched_geometry` adds direct Viboceros control/Refit records;
Rhino comparisons for these modes use the retained bounded command captures.
The installed RhinoCommon documentation exposes only the sampling producer.
Fresh captures remain pending the Wine licensing startup recorded in
[isocurve qualification](smlib-isocurves.md).

Counts, sampled divisions, aggregate control budgets and source admission retain
their existing limits. Native basis edits and Rebuild inherit the pinned
degree-17 restriction. Native Refit now accepts the tested full-order interior
case through [piecewise elevation](smlib-piecewise-elevation.md); portable Refit
still rejects it. The upstream limitation and workaround remain in the
[ledger](usd-brep-limitations.md). Continuous
source-blend error, general adaptive Rhino fitting parity and speed relative to
Rhino remain unqualified.

```sh
CMAKE_BUILD_PARALLEL_LEVEL=10 cargo test --release -p viboceros-smlib \
  --features native --test surface_tween --test surface_rebuild
```
