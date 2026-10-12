# Native knot removal

Linux `native-smlib` builds now use usd-brep for `RemoveKnot` on curves and
untrimmed tensor surfaces. The command retains selection, parsing, object
identity, attributes, groups, Undo/Redo and atomic rejection of invalid edits.
Periodic curves and periodic surface directions remain ineligible.

The shared geometry policy chooses a curve knot by model-space distance,
retaining the first equal-distance match. Surfaces choose by parameter distance
and select the higher knot on midpoint ties. Endpoint choices fail, including
repeated endpoints on unclamped inputs. Selection occurs before preparation;
the caller's requested parameter remains in geometry errors.

Only the edited tensor direction is clamped, using the shared native endpoint
adapter. Removing one knot chooses the target basis without constructing any
controls in Rust. The native editor then uses `N_CrvGetGrevilleAbscissa`,
`N_CrvEvalPt` and `N_FitCrvInterpGivenParams` to interpolate homogeneous samples.
It shares the existing [degree-change fitting adapter](smlib-degree-change.md),
including binary chart/weight scaling, full basis checks and owned snapshots.
Clamped endpoint parameters and homogeneous controls are supplied exactly;
native averaging can round a repeated endpoint outside its interval. Interior
stations and samples still use the existing native routines.
No new interpolation matrix, solver or control blending is added. Native
command and oracle removal paths exclude the portable homogeneous solver.

The dedicated `N_CrvRemoveKnot` uses a tolerance-bounded removal rule rather
than Rhino's Greville interpolation. With tolerance `1e10` on the retained
cubic fixture it removes the requested knot but differs from Rhino by up to
`3.473170830538584` in a control coordinate. This is an algorithm/command
compatibility difference, not evidence of invalid upstream geometry. The
[direct native probe](../tools/usd_brep/knot_removal_probe.cpp) retains that input.

Integration also reproduced a shared endpoint bug: `N_BasisFindSpan` treats
positive boundary spans narrower than absolute `1e-12` as degenerate.
Clamping the unclamped rational curve on a `2^-360` chart dropped both
interior breaks and returned success. The shared refinement adapter now runs
in an exactly reversible binary-scaled chart, restores original parameter
values and checks every interior knot and the active domain. This protects
InsertKnot, MakeNonPeriodic, ChangeDegree and RemoveKnot preparation without
editing the pinned upstream sources. See the [limitations ledger](usd-brep-limitations.md).

Five fresh Rhino 8.32 SDK observations cover polynomial and rational curves,
repeated knots, model-space selection and both tensor directions. The
[complete raw comparison](smlib-knot-removal-comparison.json) retains every
degree, domain, knot, control, weight, applicable closed/periodic flag and input
hash, at absolute `1e-10`, relative `1e-12`. Independent tests compare controls
and 65 curve samples over chart scales `2^-360`, `1`, `2^360` and common gauges
`1e-300`, `1`, `1e300`, `-1e300`. They also check clamping's original shape,
short first spans, source purity, endpoint errors, whole-net periodicity and a
17×17 tensor grid with the other basis preserved.
Decimal endpoint regressions cover `[0, 0.1]` and `[-0.1, 0]` domains.

Degree-17 and one-million-control native limits apply. Chart scaling must
round-trip exactly; refinement rejects changed interior bases. Arbitrary
extreme span ratios remain unqualified. Removal is an approximation without
a certified deviation bound. These observations qualify the recorded SDK
operations and command regressions, not all Rhino inputs or kernel speedups.
Other builds retain the portable backend.

```sh
CMAKE_BUILD_PARALLEL_LEVEL=10 cargo test --release -p viboceros-smlib \
  --features native --test knot_removal
CMAKE_BUILD_PARALLEL_LEVEL=10 cargo test --release -p viboceros-command \
  --features native-smlib remove_knot
cmake --build /tmp/viboceros-usd-brep-build --target viboceros_usd_brep_knot_removal_probe
/tmp/viboceros-usd-brep-build/viboceros_usd_brep_knot_removal_probe
```

The first isolated Rhino capture timed out; its terminal retry succeeded using
private profile `VibocerosOracleSmlibBridgeFinal20261010`.

The final native run passed 2,129 geometry tests, 1,313 command tests (two
ignored) and 91 bridge tests (one ignored). All 32 retained raw Rhino cases
for insertion, degree changes, nonperiodic conversion and removal match.
Both decimal-endpoint operations also succeed. All 1,095 Python tests and
native/portable workspace clippy passed; formatting passed.
Focused portable command and oracle removal checks also passed.
