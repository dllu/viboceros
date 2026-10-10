# Optional Rust SMLib bridge

The `viboceros-smlib` crate adds a narrow C ABI adapter for the pinned NVIDIA
[USD BRep kernel](usd-brep.md). Linux builds can opt in with the `native`
feature. Default workspace builds do not compile SMLib or require its submodule
or TBB. Application curved Boolean commands can opt in with `native-smlib`;
see [difference/import](smlib-import.md), [union/intersection](smlib-set-booleans.md)
and [split](smlib-boolean-split.md).
The same application feature enables [curved STEP material regions](step-regions.md).

## Build and exercise

Install a C++17 compiler, CMake 3.20+ and system oneTBB development libraries
(`libtbb-dev` on Ubuntu), then run:

```sh
GIT_LFS_SKIP_SMUDGE=1 git submodule update --init third_party/usd-brep
CMAKE_BUILD_PARALLEL_LEVEL=10 cargo test -p viboceros-smlib --features native --release
CMAKE_BUILD_PARALLEL_LEVEL=10 cargo run -p viboceros-smlib --features native \
  --example perforated_plate --release -- /tmp/viboceros-smlib-plate.obj
```

Cargo builds static SMLib, SM_API and bridge libraries directly from the
unmodified upstream sources. No Packman, OpenUSD or Python runtime is involved.
The example subtracts sixteen cylindrical cutters from a plate, checks its
analytic volume and closed mesh topology, and writes double-precision OBJ
vertices. This is a diagnostic example rather than an application file format.
The [captured plate mesh](smlib-bridge-diagnostics/perforated-plate.obj) can be
opened in an OBJ viewer. The `reference` example emits JSON measurements for the
fixed Rhino recipes:

```sh
cargo run -p viboceros-smlib --features native --example reference --release
```

## API and ownership

`Solid::box_solid`, `Solid::sphere` and `Solid::cylinder` construct independently
owned native solids. `Solid::boolean` performs union, intersection or difference
on copies and returns a new solid. Both inputs survive success and failure.
`properties` returns volume, tight bounds and manifold status with explicit
relative integration accuracy. `tessellate` produces a validated Viboceros
`TriangleMesh` from a complete triangular mesh, with explicit quality controls.

`KernelCurve::from_nurbs` and `to_nurbs` transfer degree, Euclidean control
points, weights and full knot vectors without fitting. Positive-weight clamped
curves up to degree 64 are supported; negative weights, unclamped curves and
discontinuous interior knots fail explicitly. Native evaluation can be compared
against the original Rust curve over the unchanged parameter domain.
Polynomial exports supply unit weights when SMLib omits an explicit weight array.

Handles use Rust RAII and are neither Send nor Sync. A process-wide native mutex
serializes construction, operations, property queries and destruction; context
initialization occurs once under that mutex. Independently owned handles can be
created by separate threads, but kernel calls do not execute concurrently.
Code linking other direct SMLib callers must also coordinate those calls; the
adapter's mutex cannot guard unrelated consumers. Parallel throughput will need
isolated worker processes rather than shared-context calls.

The bridge checks statuses against `SM_SUCCESS`, catches C++ exceptions at the
C boundary and returns bounded error text. Failed constructors clear output
handles. Boolean temporaries follow the API's operand-consumption contract;
copied inputs remain owned until the native call succeeds. Mesh/curve transfers
check buffer sizes, finite geometry and resource limits before Rust admission.

## Qualification and limits

Native Rust tests check copied-operand booleans, curved cuts, solid volumes,
closed and consistently oriented Rust meshes, rational curve definition and
evaluation round trips, unsupported inputs, bounded error buffers and serialized
worker calls. A compile-fail test verifies that a solid cannot be sent across
threads. The private Xvfb capture of Rhino 8.32.26160.13001 contains eleven
fixed SDK recipes. Seven solid cases match volume within 2e-6 absolute plus
1e-12 relative, and bounds within 1e-4 absolute. Three rational curves retain
identical definitions and match 129 samples each within 1e-12 coordinate
tolerance; the observed maximum coordinate difference is 4.5e-16.

The enclosed sphere cavity is an explicit diagnostic: `CreateBooleanDifference`
returned zero Rhino results, while SMLib produced a manifold solid with the
analytic volume. This is a comparison of those SDK calls, not an assertion
about Rhino's interactive command behavior. The capture retains every component
and its orientation; no output partition is assumed. Scalar volume/bounds and
curve sample checks do not establish full B-rep equivalence. Detailed evidence is in
[smlib-bridge-provenance.json](smlib-bridge-provenance.json).

[`Solid::to_brep`](smlib-brep-transfer.md) now exports NURBS geometry and shared
topology into the validated Rust model, including pole trims and exact planar
pullbacks. Qualified outputs can be saved to 3DM; supported outputs, including
[certified pole trims](step-poles.md), also round-trip through STEP.
[Rust-to-SMLib import and optional curved
BooleanDifference](smlib-import.md) now provide initial document integration.
A tessellation is suitable for display but does not replace exact
topology. Fillets, offsets, healing, general imported geometry,
empty Boolean results and model-scale extremes also need qualification.

The native feature currently supports Linux only; a normal workspace build
continues to use Viboceros's Rust geometry kernel on its supported platforms.
Upstream licenses/notices remain in the submodule; the adapter is MIT licensed.
