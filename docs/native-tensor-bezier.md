# Native tensor Bezier decomposition

Both-direction `ConvertToBeziers` and `ConvertToSingleSpans` now use usd-brep's
`N_SrfDecomposeToBez` directly. The producer clamps a private surface through
the repaired native splitters, handles independent full-order control blocks,
and completes degree-zero spans. The adapter no longer composes U and V curve
decompositions for this operation. One-axis strips retain the curve producer
to preserve the entire unselected basis and each control line's weight scale.

The original tensor copy loops started at `degree - multiplicity`, or `-1`
at a full-order interior break. Unclamped sources also violated the routine's
clamped-end assumption. The repair starts independent blocks at zero and
prepares a private net on a temporary stack. Its loop endpoints include the
last degree-zero span, which previously remained uninitialized. All blending
stays homogeneous: an intermediate zero weight can still yield finite final
patch controls. Source data remains unchanged; final patches belong to the
caller's output stack.

The C ABI checks degrees, counts, full knot vectors, exact span endpoints and
U-outer/V-inner order before publishing snapshots. Exact binary parameter
conditioning, common weight scaling, coordinate origins, aggregate resource
limits and atomic failure behavior retain their existing policies.

The [direct probe](../tools/usd_brep/tensor_bezier_probe.cpp) checks 1,008
configurations: 28 paired basis families, four parameter charts, native
polynomial sentinels, homogeneous uniform/varying weights, and signed or binary
weight scales. Degrees include 0, 1, 2, 3, 5 and 17, including 17×17. Fixtures
include full-order breaks, uniform and irregular unclamped bases, inactive
controls, a one-ULP span and intermediate zero weights. Expected homogeneous
control nets use independent explicit polynomial identities and tensor products.
Degrees, control counts and full knots compare exactly; control error is bounded
by `5e-13 * max(1, abs(expected/gauge))`. The observed maximum absolute scaled
control error is `4.44e-16`. Isolated block copies additionally compare bitwise.

Every output scalar has storage separate from the source and other outputs.
All patches are read again after destroying the source stack and after the
producer's private stack has been released. The 396 safe parent configurations
produce identical complete raw rows. Known unsafe or uninitialized parent
paths are excluded; this record does not claim a parent crash reproduction.
All 1,008 candidate configurations also pass AddressSanitizer with leak
checking disabled. Complete SMLib, SM_API and C ABI archives are instrumented;
this sanitizer run executes the direct producer, while Rust bridge tests run
normally.

All 122 saved Rhino command records replay successfully without output
normalization. Fresh public SDK checks evaluate complete source and native
patch definitions at 144 paired stations across six continuous families,
including degree 17 and unclamped knots. Points determine sampled
agreement at `1e-10`, with maximum observed point difference `6.66e-16`;
derivatives and normals remain diagnostic. These checks do not establish a
continuous error bound or fresh Rhino command-definition parity. The initial
SDK batch stopped when the worker rejected a source with inactive endpoint
padding; its exact request and error log are retained. The full bridge and
command suites, workspace lint and formatting results are retained in the [validation record](native-tensor-bezier-validation.json).

The native bridge still caps degrees at 17 and aggregate controls at one
million. Degree-zero tensor qualification concerns the internal NLib producer;
the public Rust surface representation retains its existing positive-degree
policy. A final zero weight or unrepresentable Euclidean control is an error.
Arbitrary span ratios, unrelated object-layer decomposition, extreme independent
Both-direction weight ratios, leak freedom and kernel speedups remain
unqualified. Upstream `repo.sh`/`prog_test`, USD, Python binding, OCCT and GUI
harnesses were not run; this ARM bridge builds the actual SMLib/SM_API sources.

```sh
cargo test --release -p viboceros-smlib --features native \
  --test tensor_bezier --test bezier
```
