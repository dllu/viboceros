# Native FitCrv

Linux `native-smlib` builds use usd-brep basis evaluation and banded solving for
FitCrv and the `curve_fit_geometry` oracle. The bridge exports `fit_curve`;
`try_fit_curve_with_solver` retains degree validation, arc-length preparation,
natural/kink breaks, endpoint and one-sided kink handles, adaptive bisection,
cached source samples, tolerance checks, resource bounds and polynomial weights.

Each solve supplies Greville stations and a fixed-control row mask through
`CurveFitInterpolation`. The existing native interpolation adapter replaces
fixed rows with identity constraints and evaluates the remaining rows with
`N_BasisDerivs`. Cubic fits use a five-wide band; other degrees use the existing
native degree-wide band and homogeneous back solve. No new basis evaluator or
linear solver is introduced. Coordinate translation and exact constant-channel
preservation retain small detail; fixed endpoint/kink controls are copied back
directly rather than round-tripped through the solver.

Degree-one fits and fully constrained nets bypass the solver. Portable builds
retain their existing cubic band and general-degree reduced dense solve.
Internal Sweep1 and Pipe rail fitting now reuse the native producer through
[sweep operation hooks](smlib-sweep-interpolation.md).

The [50-record comparison](smlib-curve-fit-comparison.json) matches the saved
pre-change producer at absolute `1e-9`, relative `1e-12`; maximum coordinate
difference is `3.01e-12`. Cases cover degrees 1–11, smooth/spatial curves,
closed polylines, retained/suppressed kinks and progressively tighter cubic
tolerances. Native regressions compare complete knots, controls, weights and
129 parameter samples. Independent tight-circle witnesses exercise more than
1,000 controls, checking 258 source/fitted stations at tolerance `2.5e-10`.
Tiny-channel regressions retain relative coordinate detail at `1e-18`,
`1e-180` and `1e-280`; translated nearby geometry and typed invalid options
also retain source purity. Shared-policy tests inspect fixed rows and weights,
verify direct-path bypasses with independent linear Bezier controls, and reject
bad callback layouts.
The same native binary replays 365 retained Rhino refinement/extraction/seam
records with 364 complete raw matches. The existing whole-surface unused-knot
padding discrepancy remains explicit in the comparison.

The complete optimized geometry/command/bridge suite passes 3,627 tests (11
ignored), including the new fit constraints and existing command output,
validation, source-retention and history checks. Workspace lint passes in both
feature configurations with warnings denied; formatting passes.

Only fixed-row fit jobs raise the native interpolation adapter's station budget
to 4,096; other producers retain their previous bounds. Application cubic fits
keep 4,096 controls and other degrees keep 512, with a 32,768-entry local source
sample cache. Native parameter charts and LU restrictions remain applicable;
the shared residual guard checks each channel without changing kernel tolerances.
Accuracy is sampled, not a continuous source-curve bound. Arbitrary span/coordinate
ratios and speed relative to Rhino are unqualified.

Fresh Rhino fitting capture remains pending the Wine licensing startup recorded
in [isocurve qualification](smlib-isocurves.md). These new records use the
pre-change portable implementation as their reference. See the
[limitations ledger](usd-brep-limitations.md).

```sh
CMAKE_BUILD_PARALLEL_LEVEL=10 cargo test --release -p viboceros-smlib \
  --features native --test curve_fit --test curve_interpolation --test curve_rebuild
```
