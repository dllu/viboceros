# Native degree changes

Linux builds with `--features native-smlib` now use usd-brep for all
`ChangeDegree` modes on curves and untrimmed tensor surfaces. Parsing, degree
limits, mixed selections, identities, attributes, groups, selection and atomic
Undo/Redo retain the existing command policy.

Non-deformable elevation uses `N_CrvElevateDegree`, preserving the rational
geometry and parameterization. Reduction and `Deformable=Yes` use native
Greville stations, homogeneous evaluation and `N_FitCrvInterpGivenParams`.
The latter delegates the interpolation matrix and solve to NLib. Surfaces
apply the existing curve operations to each control row or column, in U then V.
The shared Rust helper chooses target knot multiplicities; it does no geometry
construction or fitting. Native command and oracle paths no longer use the
portable homogeneous interpolation solver for degree changes.

Only directions whose degree changes are clamped at both active endpoints,
using the shared [native refinement adapter](smlib-knot-refinement.md).
Unchanged degrees return the original definition even with `Deformable=Yes`.
Full definitions enter that adapter, including periodic padding, unclamped
outer knots and the stored weight gauge. Its split/reversal workarounds and
common binary weight scaling also apply here. Native result degree, control
count and every target knot are checked before copying the result into Rust.
The native Greville helper can overflow its sum on finite charts, and native
evaluation can reject those unscaled charts. Evaluation and fitting use one
privately binary-scaled source/target chart with exact round-trip checks.
Every original target knot is restored around the native-fitted controls;
see the [limitations ledger](usd-brep-limitations.md).

Fresh Rhino 8 captures cover five curves and five tensors: polynomial and
rational elevation, reduction, deformable changes, single-axis surface edits
and periodic-seam clamping. All ten complete raw definitions match at absolute
`1e-10`, relative `1e-12`. Degrees, domains, control counts, controls, weights,
knots and applicable closed/periodic flags remain in the comparison.
The [retained report](smlib-degree-change-comparison.json) includes every
outcome and fixture/capture hashes, without comparison-time normalization.

Independent tests check source purity, unchanged-degree identity, agreement
with the portable implementation at 65 curve parameters and 17×17 tensor
grids, and exact rational elevation across a ten-unit bulge within a `1e-14`
knot span. The latter covers
chart scales `2^-360`, `1`, `2^360` and common weight gauges `1e-300`, `1`,
`1e300`, `-1e300`. Command regressions check mixed curve/surface replacement,
Undo and atomic invalid-input rejection.
Rational reduction/deformable regressions additionally cover `2^1022`, which
reproduced the native Greville overflow before the query scaling fix.

The native adapter's degree-17 and one-million-control limits apply; the
command accepts degrees 1 through 11. Output controls must remain representable
as finite Euclidean points with nonzero weights. Reduction and deformable
changes are approximations without a certified deviation bound. Broader
ill-conditioned interpolation, mixed-sign rational weights and chart extremes
remain unqualified. The tested full-order interior discontinuity returns a
native elevation error without changing its source; general discontinuous
exact elevation remains unsupported. These tests establish the recorded cases,
not arbitrary Rhino parity or kernel speedups. Other builds retain the portable backend.

```sh
CMAKE_BUILD_PARALLEL_LEVEL=10 cargo test --release -p viboceros-smlib \
  --features native --test degree_change
CMAKE_BUILD_PARALLEL_LEVEL=10 cargo test --release -p viboceros-command \
  --features native-smlib change_degree
```

Verification passed 2,129 geometry tests, 1,313 native command tests (two
ignored), 85 final native bridge tests (one ignored), focused native and
portable degree-change checks, all 1,095 Python oracle tests, and native/portable
workspace clippy with warnings denied. Formatting also passed.
