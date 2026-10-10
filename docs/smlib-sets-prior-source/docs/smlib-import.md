# SMLib import and curved BooleanDifference

`Solid::from_brep` imports closed Rust B-reps into the optional Linux native
kernel. It copies NURBS controls, weights and knots, preserves shared topology,
and constructs native faces and edgeuses without fitting, analytic replacement
or healing. UV curves are oriented in the spatial edge direction and mapped to
its parameter interval, as required by SMLib's native trim convention.

Connected shells receive separate paired native shells and regions. Their
signed Rust volumes establish boundary sense. Native face intersections reject
crossing/touching components; point classification establishes parent containment.
The importer nests shells, alternates material/void flags, and rejects sense
inconsistent with nesting. Errors propagate. Work is bounded to 128 components
and 65,536 face-pair checks.

Regressions cover primitives, ten previously qualified CAD artifacts, cavities,
disconnected solids, a nested material island, repeated follow-up cuts, invalid
nesting, component overlap, open shells and empty subtraction. They check
material/void census and subsequent operations as well as geometry round trips.
An early one-shell prototype passed volume checks but failed a follow-up cut;
the final explicit region graph retains that regression.

## Enable the application path

```sh
GIT_LFS_SKIP_SMUDGE=1 git submodule update --init third_party/usd-brep
CMAKE_BUILD_PARALLEL_LEVEL=10 cargo run --release --features native-smlib
```

Compiler, CMake and system TBB requirements are in the [bridge guide](smlib-bridge.md).
Default builds retain the Rust Boolean kernel. The feature extends
`BooleanDifference` only: convex/polyhedral paths run first, and explicit
unsupported-geometry errors select native closed-solid subtraction. Arithmetic,
work-limit and topology errors remain errors. Open sheets remain unsupported.

Native operands and outputs are prepared before document changes. Inputs are
copied, and failures preserve geometry and history. Complete removal succeeds
with no output objects. Material-body extraction separates results while retaining
cavities with their body. Results reuse existing metadata, grouping, selection,
deletion options and one-step undo. Single results retain geometry user text;
split results clear it according to the existing policy.

No-change detection uses independently integrated Rust volumes and dimensional
document tolerance; subtraction that increases volume is rejected. Tangency,
coincident curved faces, small removed features, broader multi-object policies,
extreme scales and native output ordering need further qualification. Union,
Intersection, Split, fillets and offsets are not yet routed through this adapter.

## Reproduce the document workflow

```sh
cargo test -p viboceros-smlib --features native --release
cargo test -p viboceros-command --features native-smlib --release
cargo run -p viboceros-command --features native-smlib --release \
  --example curved_difference -- /tmp/viboceros-curved-difference
```

The example constructs its box and cylinder with Rust, executes the document
command, checks analytic volume and saves its editable 3DM result. The native
oracle reads that artifact on a private Xvfb display.
Rhino 8.32 accepts the captured result as a valid solid with seven faces and
fifteen edges. Its 495 edge samples match the analytic box/cylinder boundaries
within 1e-8 (observed maximum 2.05e-10). This validates that artifact, not full
interactive command parity. The probe's display mesh has open boundaries;
its diagnostic is retained separately from exact B-rep validity.

Sixteen repeated import/cut/extraction cycles pass a bridge-instrumented
ASan/UBSan harness with leak checking. The kernel archives are uninstrumented,
and UBSan vptr checks are excluded after reporting upstream `SmTArray` storage
behavior; those initial diagnostics are preserved. This check is not a general
memory-safety proof for SMLib.
[Provenance](smlib-import-provenance.json) records obtained validation and limits.
