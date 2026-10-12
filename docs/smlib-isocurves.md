# Native isocurve extraction

Linux `native-smlib` builds route ExtractIsocurve, its density-based ExtractAll
mode, trim-aware B-rep portions, Contour boundary/isocurve preparation and
CreateUVCrv rectangle sizing through usd-brep's `N_SrfExtractIsoCrv` producer.
The native adapter exposes `surface_isocurve(surface, direction, parameter)`;
U means a U-varying curve at fixed V, and V means the reverse.

The geometry policy validates the fixed parameter before backend entry and
checks the complete returned varying-axis degree, count and full knot vector.
Interior extraction uses conditioned homogeneous controls and exact binary
scaling of the fixed-axis chart. The adapter checks native span and multiplicity
against the exact source chart before extraction, then exports an immutable
curve snapshot under the existing process-wide kernel lock.
Fixed-axis scaling includes the query parameter and backs off a downward scale
when it would lose subnormal near-zero parameters or knots. The existing picked
and density hole regressions exercise that repair.

The native producer copies first/last control rows at fixed-axis endpoints,
which is incorrect for unclamped padding. The adapter reuses checked one-sided
native endpoint clamping before selecting the resulting row. Full-order knot
groups already separate control blocks, so the adapter copies the corresponding
row or column exactly. Interior/start queries take the right limit; active ends
take the left limit. This also avoids the native producer's full-order rejection
and preserves exact clamped spherical pole payloads without projection roundoff.
No additional basis evaluator, refinement or fitting solver is introduced.

The [direct probe](../tools/usd_brep/isocurve_probe.cpp) reproduces both native
limitations: an unclamped Y endpoint of `3` instead of the independent limit
`28/11`, and rejection at an interior full-order knot. The
[retained result](smlib-isocurves-probe.json) and
[limitations ledger](usd-brep-limitations.md) keep source and reproduction details.

Shared callback APIs retain density station rules, extent/resource filtering,
trim intersections and lossless local-domain restoration. The native command
uses both isocurve extraction and the existing checked curve trimmer for clipped
face portions. Display wire construction and portable builds retain their
existing default producers. Document selection, attributes and history remain
with the command layer; native handles never enter document state.

The [native regressions](../crates/viboceros-smlib/tests/isocurves.rs) compare
complete bases, Euclidean controls, weight ratios and 65 parameter samples
against the portable reference and original surface. They cover both directions,
periodic/rational sources, endpoint and near-knot queries, binary chart scales
`2^-360`, `1`, `2^360`, common gauges `1e-300`, `1`, `1e300`, `-1e300`, shifted
domains, full-order fixed-axis knots, inactive endpoint controls, exact clamped
poles, invalid requests and source purity. Shared-policy tests check hole
intervals, both extraction callbacks and `1e12` UV translations.

The complete native geometry/command/bridge suite passes 3,591 tests (11 ignored),
including the picked and density hole regressions after conditioning repair.
The Python oracle suite passes 1,097 tests. Workspace lint passes in both feature
configurations with warnings denied, and formatting passes. Replaying the 365
retained reference records still gives 364 complete matches and the previously
documented whole-domain surface outer-knot padding discrepancy.

The new `surface_isocurve_geometry` protocol accepts a complete surface, one
varying direction and its fixed parameter. All 15 new native cases succeed;
[retained native definitions](smlib-isocurves-native.json) and
[qualification status/hashes](smlib-isocurves-comparison.json) keep that evidence.
Fresh SDK comparison is pending: the licensed Rhino Wine session currently
reports a missing `AcquireCredentialsHandleW` export in `sspicli.dll` and opens
its sign-in screen before running the worker. Capture availability remains
limited by this startup error; fresh SDK parity for the new fixture is pending.

Native degree/resource/gauge restrictions remain those of the shared adapter.
Exact span guards reject a native classification that disagrees with the source
chart. These finite checks do not certify arbitrary weight/chart ratios,
continuous parity or kernel speedups.

```sh
CMAKE_BUILD_PARALLEL_LEVEL=10 cargo test --release -p viboceros-smlib \
  --features native --test isocurves
cmake --build /tmp/viboceros-usd-brep-build \
  --target viboceros_usd_brep_isocurve_probe -j 10
/tmp/viboceros-usd-brep-build/viboceros_usd_brep_isocurve_probe
```
