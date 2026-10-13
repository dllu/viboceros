# Native elevation across full-order breaks

Non-deformable ChangeDegree now supports full-order interior knot groups in
native curves and tensor surfaces. The adapter partitions the clamped control
curve at groups of multiplicity `degree + 1`, applies `N_CrvElevateDegree` to
each contiguous piece, then assembles their controls and target knots without
merging shared endpoints. This preserves discontinuities and independent
one-sided limits. Ordinary interior knots remain inside their original piece.
The same helper serves surface rows and columns, section matching and native
TweenSurfaces Refit. No new degree-elevation algorithm is added.

Every piece uses the original homogeneous coordinate and weight gauge. The
assembly retains all controls, removes only each later piece's duplicated start
knot group, and verifies the complete prescribed target basis before transfer.
`N_CrvFromCPtsAndKnots` borrows its arrays; `N_CrvCopy` moves the assembled result
into native-stack-owned storage before temporary vectors are destroyed.
Sources remain immutable and the existing per-axis resource checks apply.

The [direct probe](../tools/usd_brep/piecewise_elevation_probe.cpp) and
[retained output](smlib-piecewise-elevation-probe.json) reproduce the pinned
upstream limitation. Elevating a quadratic with knots
`[0,0,0,0.5,0.5,0.5,1,1,1]` to degree four returns status `1` as one curve;
the two separated pieces both return status `0`. Independent Bernstein
coefficients give the elevated controls and join limits `[2,0,0]`/`[4,0,0]`.
The adapter now succeeds on that input while retaining the upstream finding in
the [limitations ledger](usd-brep-limitations.md).

The [21-record comparison](smlib-piecewise-elevation-comparison.json) uses
[exact rational Bernstein coefficients](../tools/usd_brep/piecewise_elevation_reference.py)
independent of both geometry kernels. Tensor reference arithmetic stays
homogeneous and exact until final projection. All records match at absolute
`1e-10`, relative `1e-12`, with a separate stored-weight relative check of
`1e-12` that also applies to tiny gauges. Maximum coordinate error is
`1.78e-15`; maximum relative weight error is `2.23e-16`.

Regressions cover independent polynomial controls, multiple rational blocks,
degrees 3 through 17, chart scales `2^-360` through `2^360`, positive/negative
common weight gauges `1e-300` through `1e300`, mixed ordinary/full-order knots,
one-sided limits and U/V tensor edits. Native TweenSurfaces Refit now succeeds
on the retained full-order example and matches independently derived blended
control nets; the portable elevation path still rejects that source.

The complete optimized geometry/command/bridge suite passes 3,639 tests (11
ignored). All 365 retained Rhino refinement/extraction/seam records keep their
previous 364 complete raw matches and one unused whole-surface knot-padding
discrepancy. Workspace lint passes in both feature configurations with warnings
denied; formatting passes.

The pinned degree-17 and million-control limits, representable nonzero output
weights and existing endpoint clamping restrictions remain. Reduction and
deformable fitting are unchanged. Fresh Rhino capture remains pending the Wine
licensing startup recorded in [isocurve qualification](smlib-isocurves.md).
These new records prove independent elevation behavior, not fresh SDK parity
or arbitrary signed-weight conditioning.

```sh
CMAKE_BUILD_PARALLEL_LEVEL=10 cargo test --release -p viboceros-smlib \
  --features native --test degree_change --test surface_tween
cmake --build /tmp/viboceros-usd-brep-build \
  --target viboceros_usd_brep_piecewise_elevation_probe -j 10
/tmp/viboceros-usd-brep-build/viboceros_usd_brep_piecewise_elevation_probe
python3 tools/usd_brep/piecewise_elevation_reference.py \
  tools/rhino_oracle/fixtures/piecewise_degree_elevation.json
```
