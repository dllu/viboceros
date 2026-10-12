# Native multiple-knot removal

Linux `native-smlib` builds now use usd-brep for `RemoveMultiKnot` on curves
and untrimmed tensor surfaces. Parsing, mixed selections, object identities,
attributes, groups, selection and atomic Undo/Redo retain the existing policy.
Periodic curves and selected periodic surface directions are rejected before
native editing. Empty candidate sets return the original definition unchanged.

The geometry crate shares knot-group selection between the native and portable
backends. The default removes repetitions strictly below the degree, leaving
one knot. `RemoveFullyMultipleKnots=Yes` also considers degree-multiple groups
whose one-sided tangent angle is strictly below `MaxKinkAngle`; degree-one
groups are removed entirely. Surface crease selection retains its transverse
span start/midpoint/end sampling rule. It now reads the existing basis directly
instead of allocating an otherwise unused control curve.

Selected groups are processed from higher to lower parameters. Each complete
group is removed in one homogeneous refit, rather than one refit per repetition.
Surfaces finish U before selecting V groups on the edited tensor. Original
periodic flags are validated before either direction; individual control rows
do not determine eligibility. The shared metadata helper constructs each target
basis without blending controls or solving a matrix.

Native control construction uses the existing
[knot-removal fitting adapter](smlib-knot-removal.md): `N_FitCrvInterpGivenParams`,
native interior Greville/evaluation calls, exact clamped endpoint samples,
binary chart/weight scaling and copied complete definitions. Only axes with
selected groups are clamped. No new native solver, matrix builder or C ABI is
added. Native command and oracle paths exclude the portable homogeneous solver
for these edits.

Eleven fresh Rhino 8.32 observations come from actual `RemoveMultiKnot`
commands: polynomial and rational curves, degree-one collapse, fully multiple
kink/crease thresholds, varying transverse creases and both tensor directions.
The [retained comparison](smlib-multiple-knot-removal-comparison.json) includes
all complete raw definitions and fixture/capture hashes at absolute `1e-10`,
relative `1e-12`, without comparison-time normalization or coefficient filtering.

Independent bridge tests compare removal counts, full controls and 65 curve
samples over chart scales `2^-360`, `1`, `2^360` and common weight gauges
`1e-300`, `1`, `1e300`, `-1e300`. Directional tests compare 17×17 tensor grids
and the unchanged other basis. Source purity, no-op identity, angle errors and
original periodic-error priority are also checked. Command regressions check
mixed replacements, attributes, groups, selection and atomic history.

Native degree-17 and one-million-control limits apply to nonempty edits.
Every group refits the preceding result, so roundoff can accumulate across
long removal sequences. Surface angle sampling is not a continuous maximum
certificate, and removal has no certified deviation bound. Arbitrary ill-
conditioned inputs and extreme span/weight ratios remain unqualified. These
tests establish the recorded cases, not arbitrary Rhino parity or kernel
speedups. Other builds retain the portable backend.

```sh
CMAKE_BUILD_PARALLEL_LEVEL=10 cargo test --release -p viboceros-smlib \
  --features native --test multiple_knot_removal
CMAKE_BUILD_PARALLEL_LEVEL=10 cargo test --release -p viboceros-command \
  --features native-smlib remove_multi_knot
```

The first isolated live capture timed out; its terminal retry succeeded using
private profile `VibocerosOracleSmlibBridgeFinal20261010`.

Verification passed 2,129 geometry tests, 1,313 native command tests (two
ignored), 96 native bridge tests (one ignored), all 1,095 Python oracle tests,
and native/portable workspace clippy with warnings denied. All 43 retained
raw Rhino definitions across shared NURBS edits match. Formatting passed.
Focused portable command and oracle checks also passed.
