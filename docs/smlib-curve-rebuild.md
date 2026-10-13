# Native curve Rebuild

Linux `native-smlib` builds use usd-brep for open and periodic curve Rebuild,
including the readonly command preview and the `curve_rebuild_geometry` oracle.
The bridge exports `rebuild_curve`; the shared geometry callback
`try_rebuild_curve_with_interpolation` retains count adjustment, arc-length
sampling, uniform knots, even-degree half-span periodic stations, coordinate
conditioning, exact open endpoints, polynomial weights, tangent projection and
periodic control repetition. Sources remain immutable and native buffers never
enter document history.
Closed degree-one outputs retain the existing non-periodic flag convention.

The existing interpolation adapter now accepts degrees and constraint layouts
for Rebuild as well as InterpCrv and Loft. It reuses `N_BasisDerivs`, native
banded LU and homogeneous back solving, with exact span checks and normalized
per-channel residual checks. No extra basis evaluator or linear solver is added.
Portable builds retain their previous band elimination and Fourier solve.

Periodic uniform Greville collocation is a cyclic band around control
`floor(degree / 2)`. The adapter aligns those control columns, then orders the
stations as `0, n-1, 1, n-2, ...`. Neighboring cyclic stations stay nearby in this
ordering, so both rows and columns fit a native band no wider than
`2 * degree + 1`. It restores the original control order after solving.
Matrix storage and residual checks are linear in point count at fixed degree;
factoring uses the native band routines. Nonuniform periodic InterpCrv and Loft
retain their existing full pivoted matrix path.

The fitting convenience producer cannot directly replace periodic collocation:
`N_FitCrvInterpGivenParams` uses identity endpoint rows even on unclamped bases.
The [direct probe](../tools/usd_brep/rebuild_interpolation_probe.cpp) and
[retained result](smlib-curve-rebuild-probe.json) demonstrate incorrect endpoint
limits for the exact target `X(u)=u`. Dedicated periodic fitting exports are
also absent from the pinned default kernel build. See the
[limitations ledger](usd-brep-limitations.md).

Regressions compare complete definitions and 65 parameter samples with the
portable implementation for all degrees 1–11, count adjustment, odd/even periodic
counts, tangent preservation and the 1,000-point limit. Independent analytic
circle checks verify every periodic interpolation station, including small
systems and the public point limit. Tiny-channel checks retain relative detail
at `1e-18`, `1e-180` and `1e-280`. Shared-policy tests inspect normalized targets
and reject invalid requests or inconsistent callback output before construction.

All [94 full-record comparisons](smlib-curve-rebuild-comparison.json) match the
saved pre-change portable oracle at absolute `1e-10`, relative `1e-12`;
maximum numeric difference is `9.04e-12`. The same native binary preserves all
43 existing point/curve-through comparisons and 364 of 365 retained Rhino
refinement/extraction/seam records. The existing unused whole-surface knot
padding discrepancy remains in the raw report.

[Diagnostic timings](smlib-curve-rebuild-performance.json) for 1,000-point open
and closed Rebuild at degrees 3 and 11 show median native times 1.30–1.80 times
faster than the pre-change portable implementation on this Linux ARM64 machine.
Each backend ran five requests with five timed rebuilds and an untimed warmup
per operation; serialization is excluded, and other compilation was active.
These measure the complete Rebuild path, including source arc-length sampling.

The complete optimized geometry/command/bridge suite passes 3,612 tests (11
ignored), including the independent circle witnesses and existing command
preview/history checks. Workspace lint passes in both feature configurations
with warnings denied; formatting passes.

Fresh Rhino capture remains unavailable pending the Wine licensing startup
described in [isocurve qualification](smlib-isocurves.md). Native/portable
agreement does not certify fresh SDK parity, a continuous approximation bound,
arbitrary input conditioning or speed relative to Rhino. Native LU tolerances
and the existing degree/point limits still apply.

```sh
CMAKE_BUILD_PARALLEL_LEVEL=10 cargo test --release -p viboceros-smlib \
  --features native --test curve_rebuild --test curve_interpolation --test loft_basis
cmake --build /tmp/viboceros-usd-brep-build \
  --target viboceros_usd_brep_rebuild_interpolation_probe -j 10
/tmp/viboceros-usd-brep-build/viboceros_usd_brep_rebuild_interpolation_probe
```
