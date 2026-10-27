# Native Sweep1 and Pipe interpolation

Linux `native-smlib` builds now delegate rail clamping, section-station knot
insertion, adaptive curve fitting and homogeneous section solving to usd-brep
for Sweep1 and Pipe. The native surface producers are `sweep_surface` and
`fit_sweep_model_surface`. Existing native common-section matching remains in
place. Command output and the direct sweep oracle use the same producers.

`SweepSurfaceOperations` keeps numerical producers separate from the shared
frame/blending model. Retained-rail construction preserves the source rational
basis, Local Euclidean profile blending and end-weight policy. Refit uses the
shared arc-length fitter, transported sections and homogeneous blending.
Non-Greville interior sections still force the established refit policy.
Continuous-model fitting keeps its adaptive span seeds, two validation lattices,
positive individual weight checks and sampled error criterion. Every mode
retains section and closed-seam audits, resource checks and source purity.

Station metadata is prepared without constructing the portable basis rows.
The existing native fixed-row interpolation adapter factors one banded matrix
and solves all profile control channels in one batch. Fixed endpoints use
identity rows and preserve exact section point payloads. Consecutive equal
parameters are accepted only when both rows are fixed, supporting independent
one-sided full-order constraints without admitting an unconstrained duplicate.
`FixedInterpolationSystem` generalizes the existing `CurveFitInterpolation`
name, which remains a compatibility alias. No new basis evaluator or linear
solver is added. Frame transport, profile placement, rail-weight queries and
independent source/surface audits remain in the geometry layer.

The [63-record comparison](smlib-sweep-interpolation-comparison.json) matches
the saved pre-change producer at absolute `1e-8`, relative `1e-12`, including
command/API cases, section and weight diagnostics, and closed-circle definitions
and refits. Maximum numeric difference is `8.41e-9`, in closest-point query
samples on a varying rational profile. These queries retain their original
model tolerance; the comparison does not certify continuous matched-parameter
surface parity.

Native regressions exercise Local/Global/Roadlike models, high-degree input
rails, interior stations, retained/refit modes, closed seams and continuous
model fitting. Complete bases, control nets and 17×17 parameter grids are
compared with the portable producer. Common profile gauges `1e-300` through
`1e300` and relative coordinate detail down to `1e-280` remain intact.
Shared-policy tests inspect callback use, direct source constraints and typed
rejection of inconsistent solver output.

Verification passes 3,632 distinct geometry/command/bridge tests (11 ignored):
3,630 unit/integration checks in the full optimized run, the additional fixed-row
guard regression, and the compile-fail documentation check. Documentation tests
pass when rerun alone after a dependency-loading error from overlapping builds.
Workspace lint passes in both feature configurations with warnings denied;
formatting passes.

The existing 4,096 rail-control, 512 profile-control and 262,144 surface-control
budgets apply before solving. Native clamping/refinement retain their degree-17,
parameter-chart limits; [full-order elevation](smlib-piecewise-elevation.md)
now uses separated native pieces. Banded LU and per-channel
residual guards keep kernel tolerances unchanged. Existing Rhino basis/domain
and world-query discrepancies remain documented in [Sweep1 validation](sweep1/validation.md).
Continuous model error in the rail direction, arbitrary conditioning and speed
relative to Rhino remain unqualified. Fresh captures remain pending the Wine
licensing startup described in [isocurve qualification](smlib-isocurves.md).
See the [limitations ledger](usd-brep-limitations.md).

```sh
CMAKE_BUILD_PARALLEL_LEVEL=10 cargo test --release -p viboceros-smlib \
  --features native --test sweep_interpolation --test section_sweep --test curve_fit
```
