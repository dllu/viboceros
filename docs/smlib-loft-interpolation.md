# Native Loft interpolation

Linux `native-smlib` builds now use usd-brep for Loft's homogeneous interpolation
as well as its common section basis. Normal, Tight and Uniform share native basis
evaluation and one native factorization across all control rows. Open systems
retain their two automatic control-handle constraints; closed systems fold the
periodic control columns into the existing unique-control system. Loose and
Straight retain direct control assembly and never enter the solver.

`LoftInterpolation` carries the shared full knots, section parameters and
row-major homogeneous targets. The geometry layer retains section spacing,
endpoint-weight normalization, local origins, common weight scale, constant
homogeneous channels, exact open endpoints, periodic repetition, estimated final
domains and command topology. Portable builds use the same targets with faer's
solver. The caller callback's output length is checked before projection.

The native adapter uses `N_BasisDerivs` at derivative order zero: unlike
`N_BasisEval`, it computes the actual unclamped periodic endpoint basis rather
than substituting a clamped identity row. Exact span checks surround the native
evaluation, and binary chart scaling retains the original parameters and knots.
The matrix is assembled from those native coefficients and the shared control
constraints. Open systems order the two handle rows beside their endpoints and
use five-wide banded storage with `N_RealMatrixLuDecompose`. Closed systems use
the full cyclic matrix and `N_RealMatrixLuDecomposePivot`.

Each closed-system homogeneous target column receives the factorization's row permutation
before `N_RealMatrixForBack`. This avoids the real-vector pivot solve's tolerance
skip for small nonzero right-hand-side values. The
[direct probe](../tools/usd_brep/loft_solver_probe.cpp) and
[retained output](smlib-loft-interpolation-probe.json) reproduce a two-by-two
example independently. Kernel tolerances are unchanged. Per-channel normalized
residual checks retain the input constraints without overflowing large scales or
discarding small channel units. All calls run under the shared kernel lock;
stack cleanup and Rust-owned output buffers also cover failures.

The existing native skin and periodic fitting convenience producers have
different control-count/end-condition requirements. The matrix routines let
Loft retain its established Rhino policy without adding another basis evaluator
or linear solver. See the [limitations ledger](usd-brep-limitations.md).

The [native Loft regression](../crates/viboceros-smlib/tests/loft_basis.rs)
compares the 72 permanent inputs with the portable model, including all five
styles, open/closed modes, mixed sections and endpoint-weight diagnostics. It
checks full definitions and 17×17 grids. Additional tests retain common weight
gauges, binary profile charts, source purity and relative channel detail at
`1e-18`, `1e-180` and `1e-280` in both open and closed systems. Shared-policy tests
verify target layouts, solver bypasses and rejection of bad callback outputs.

The complete native geometry/command/bridge suite passes 3,600 tests (11 ignored).
Workspace lint passes in both feature configurations with warnings denied, and
formatting passes. All 72 full API/command records match the saved pre-change
portable oracle at absolute `1e-9`, relative `1e-12`; maximum numeric difference
is `7.11e-15`. The [comparison](smlib-loft-interpolation-comparison.json) retains
each outcome, input fixture hashes and both audit hashes. The three existing
near-equal-weight diagnostics are portable-reference checks here, not new Rhino
parity claims. The original 365 retained Rhino records also retain their prior
364 full matches and one unused surface outer-knot padding discrepancy.

The native LU tolerance can reject a sufficiently ill-conditioned system. This
integration does not certify arbitrary section-spacing/weight ratios, continuous
surface parity or kernel speedups. Fresh Rhino capture availability remains
limited by the Wine licensing startup recorded in
[isocurve qualification](smlib-isocurves.md).

```sh
CMAKE_BUILD_PARALLEL_LEVEL=10 cargo test --release -p viboceros-smlib \
  --features native --test loft_basis
cmake --build /tmp/viboceros-usd-brep-build \
  --target viboceros_usd_brep_loft_solver_probe -j 10
/tmp/viboceros-usd-brep-build/viboceros_usd_brep_loft_solver_probe
```
