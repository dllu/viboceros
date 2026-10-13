# Native point-grid surfaces

Linux `native-smlib` builds route `SrfPtGrid`, the `surface_grid` oracle API,
and point-grid 3DM construction through usd-brep basis evaluation and matrix
solving. `SrfControlPtGrid` directly stores its supplied control net.

The shared geometry layer retains mean-chord/Greville stations, averaged knots,
degree clamping, U-fast tensor storage, coordinate normalization, constant
channels and exact repeated controls. Command input remains V-fast; retained
point clouds preserve entry order. Crease partitioning, solid orientation and
atomic document history use the existing command topology path.

`PointGridInterpolationSystem` supplies each axis's complete knots, construction
stations, folded-control flag and batched homogeneous targets. Native open
systems share banded LU; closed systems share partial-pivot LU with the existing
scalar `N_RealMatrixRightForBack`, avoiding both the pivot convenience solver’s
small-RHS skip and dense per-coefficient point-blending calls. All four
homogeneous channels remain in the batch layout. The factorization
is reused across every row of the other axis. Native preparation does not build
the portable dense collocation matrices. Portable builds retain faer full-pivot
LU and the independent Rust basis implementation.

Closed directions of degree greater than one constrain independent Greville
sites, including sites outside the active domain. `N_BasisDerivs` already
continues the nearest boundary-span polynomial. The [standalone probe](../tools/usd_brep/grid_continuation_probe.cpp)
returns exactly `[9/8,-1/4,1/8]` at `-0.5` for a quadratic with active domain
`[0,5]`; [its output](smlib-point-grid-probe.json) is retained. The adapter sums
absolute unfurled coefficients before folding columns and returns that
amplification to the independent tensor evaluator. Both native linear-system
residuals and every construction-site evaluation must pass before returning
geometry. Incorrect backend layouts, nonfinite amplification and failed
geometry audits return errors.

The bridge exposes `interpolate_point_grid` and `point_grid_brep`;
`interpolate_open_point_grid` remains compatible and delegates to the same
producer. TweenSurfaces sampling shares this open-grid path.

[Complete pre-change comparisons](smlib-point-grid-comparison.json) pass all
83 ordinary API, command, higher-degree and same-file 3DM records. The 75
ordinary records use absolute `2e-12` plus relative `1e-14` (maximum difference
`3.23e-14`); eight higher-degree records use `1e-8` plus `1e-12` (maximum
`1.01e-10`). These compare a saved portable producer, not fresh Rhino output.
Existing Rhino qualification of the portable producer is documented on the
[point-grid page](point-grid-surfaces.md). Fresh Rhino captures remain pending
the Wine licensing startup described in [isocurve qualification](smlib-isocurves.md).

All four high-degree diagnostic operations return geometry. Degree eleven's
API/command records match the portable net at the higher-degree epsilon;
degree nine's two full records differ by up to `1.18e-7` in control coordinates
and about `3.04e-8` in active samples. They remain failed diagnostic comparisons.
[Independent 80-digit evaluation](smlib-point-grid-residuals.json) at recorded
construction sites gives degree-nine residuals `9.92e-9` for the native net and
`6.41e-10` for the portable net; degree-eleven residuals are `1.67e-9` and
`1.84e-9`. The [reference script](../tools/usd_brep/references/point_grid_residuals.py)
uses exact binary64 inputs with a separate local Cox/de Boor recursion. These
numerical diagnostics do not certify arbitrary conditioning or a Rhino match.

[Local timing](smlib-point-grid-performance.json), from three alternating
five-iteration runs after warmup, includes the shared station and evaluator
checks. The open 256×256 grid takes 36.5 ms native versus 35.6 ms portable;
the closed 64×64 grid takes 4.08 ms versus 1.87 ms. Closed dense native solves
remain slower than the portable batch factorization. These are local throughput
diagnostics, not Rhino measurements or CI timing assertions.

Expanding the native-oracle test run also exposes 20 existing Join/audit
assertions written for portable behavior. [Saved-binary replay](smlib-point-grid-oracle-baseline.json)
compares all 2,092 records in 27 referenced Join fixtures against the pre-change
native oracle;
complete successful values and failure outcomes remain unchanged. These tests
are still failing under the native feature and are not counted as passing checks
for this change. The point-grid oracle test and full geometry/command/bridge
suite are qualified separately.

The optimized geometry/command/bridge suite passes 3,647 tests (11 ignored),
including native grid/tween regressions, topology, source retention and history.
The native point-grid tests cover 47 API fixture nets, all requested open
degrees through 11, the 256×256 grid, minimum closed counts, repeated controls,
`2^-360`/`2^360` coordinate scales and nonconstant channels down to `1e-280`.
Shared tests reject invalid callback layouts, invalid amplification and incorrect
returned geometry without weakening construction-site checks.

The scoped native point-grid oracle test passes. Workspace lint passes in both
feature configurations with warnings denied, and formatting passes.

Counts remain limited to 256 per axis and degrees to 11. Boundary-span
continuation can amplify floating-point cancellation substantially. The
[high-degree diagnostics](point-grid-surfaces.md#verification), narrow-span
native guards and solver restrictions remain explicit in the
[usd-brep ledger](usd-brep-limitations.md). A successful solve does not imply
that all closed-grid source points lie inside the active patch, or that a
folded shell is embedded. Arbitrary-conditioning and Rhino-speed parity remain
unqualified.

```sh
CMAKE_BUILD_PARALLEL_LEVEL=10 cargo test --release -p viboceros-smlib \
  --features native --test point_grid --test surface_tween
```
