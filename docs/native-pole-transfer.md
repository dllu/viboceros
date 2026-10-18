# Native pole transfer and exact copies

Reparameterizing the same sphere or hemisphere could drop its singular trims
or make native B-rep export fail. The 20-case baseline uses radius two, model
tolerance `0.001`, independent U/V extents from `0.0001` to `10000`, and both
axis orders. Four cases retained the correct poles; five silently lost pole
trims and eleven failed export. These are parameterization changes of the same
spatial geometry.

The adapter fixes three independent causes:

- Pole reconstruction checks natural boundaries using a floating-point
  allowance scaled by each axis's extent. A certified collapsed boundary is
  restored before ordinary endpoint uncertainty can hide its UV span. The
  complete boundary must collapse within the existing model/vertex distance
  bound, using the sign-coherent control hull or continuous surface-image
  certificate. Short gaps on regular boundaries do not become singular trims.
- Reversed open sheets reuse the shared exact face U/V permutation before
  import. The native face-swap operation mirrors UV curves through a normalized
  diagonal frame; rounding can mix a long parameter axis into a short one.
  Surface coefficients, loop traversal and face sense now compensate exactly
  at this import boundary.
- Native B-spline curve/surface copy constructors no longer run knot fixup.
  A copy retains the full source basis and homogeneous control data. The two
  bounded patches compile generated source files; the pinned usd-brep checkout
  remains unchanged. B-rep import reuses the existing checked basis adapter
  after canonical raw-data construction. That adapter now retains the native
  curve dimension and uses the correct X/Y/weight layout for 2D rational trims.

The direct C++ probe creates valid native sources through `SetExpert`, then
calls the actual native copy operations. Its baseline executable overrides only
the two repaired translation units with pinned upstream sources, keeping other
kernel/adapter repairs equal. Cases cover 2D and 3D rational curves, both tensor
axes, parameter scales `2^-40`, `1`, `2^40`, and an interior knot one ULP below
the active end. Eight of 24 unmodified copies retain exact knots and controls;
all 24 repaired copies do. Source data remain unchanged. Twelve additional
checks reparameterize unowned curve copies through the shared basis adapter and
retain homogeneous data and dimension, including 2D weight channels.

The native sphere/hemisphere regression now retains the expected one or two
poles in all 20 cases. It independently checks radius, area, outward normals,
closed-sphere volume, manifold/closed topology and source preservation. Four
rescaled outputs also round trip through 3DM with their poles and geometry.
Unit tests exercise UV extents `2^-360` and `2^360`, translated charts, both face
senses, noncollapsed short gaps and inherited model-space vertex bounds.

The [336-case Join comparison](native-pole-join-comparison.json) keeps the raw
before/after responses. All fields outside UV trim definitions agree; eight
cases have a different cyclic starting trim and retain that raw difference.
A separate cyclic comparison verifies the same complete trim definitions at
the ordinary `1e-10` absolute / `1e-12` relative epsilon. Document state,
topology and spatial geometry retain their recorded values. Existing raw Rhino
differences remain; this is native transfer regression evidence.

The full native release suite passes 400 tests, with one ignored. The command
release suite passes 1577 tests, with three ignored and two existing large-scale
planar Boolean failures. Both failures also reproduce in a retained pre-change
executable and without SMLib; their [baseline logs](native-pole-existing-planar-failures.log)
are retained. The coplanar group regression now compares the exact ordered
membership before and after the operation, removing an assumption that random
object IDs sort in creation order.

[Validation and retained outcomes](native-pole-transfer-validation.json) identify
the baseline, exact source/patch hashes, direct probe results and regression
commands. This qualifies these transfer and copy paths. Other native operations
can still change parameterization or re-enter canonical queries; arbitrary
modeling topology, tiny-chart native solvers and Rhino performance parity are
not established by these tests.

```sh
cargo test -p viboceros-smlib --features native --test pole_transfer -- --nocapture
cargo test -p viboceros-smlib --features native --lib brep::poles::tests
# In the CMake build directory printed by Cargo's native build:
cmake --build BUILD_DIR --target viboceros_smlib_copy_basis_probe viboceros_smlib_copy_basis_baseline -j 10
BUILD_DIR/viboceros_smlib_copy_basis_baseline
BUILD_DIR/viboceros_smlib_copy_basis_probe
```
