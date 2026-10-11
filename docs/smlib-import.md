# SMLib import and native BooleanDifference

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
Default builds retain the Rust Boolean kernel. With `native-smlib`, Difference
routes closed planar and curved inputs directly to SMLib; the Rust convex and
polyhedral Difference adapters are compiled only without that feature. Native
errors propagate before document edits. [Union](smlib-set-booleans.md) also enters
SMLib directly; Intersection still selects the adapter after explicit
unsupported-geometry errors.
Open sheets remain unsupported by native subtraction.

Participation uses native face intersections against original shells and
cutters. Strict containment and point-only contact do not activate subtraction;
face/edge contact can succeed without changing volume. Coincident operands fail.
Inactive detached shells and unopened cavities are omitted from replacements,
matching the saved Rhino compound workflows. Retained shells are transferred
without fitting and SMLib owns all cutting and material-body extraction. Native
input boundary validation also checks adjacent faces before any Boolean work.

Native operands and outputs are prepared before document changes. Inputs are
copied, and failures preserve geometry and history. Complete removal succeeds
with no output objects. Material-body extraction separates results while retaining
cavities with their body. Results reuse existing metadata, grouping, selection,
deletion options and one-step undo. Single results retain geometry user text;
split results clear it according to the existing policy.

Final volumes are independently integrated in Rust and checked against the
participating input, with dimensional document tolerance. The full original
volume cannot authorize that check when the command omits an unopened cavity.
Eligible operations retain the validated kernel result even when removed volume
is below that tolerance: small volume alone cannot establish unchanged geometry.
A unit-volume corner cut in a 100,000-unit cube checks that its three new faces
survive even though removed volume is below the dimensional threshold.
Tangency, coincident curved faces, small removed features, broader multi-object
policies, extreme scales and native output ordering need further qualification.
[Split](smlib-boolean-split.md) and [fillets](fillet-edge.md) also use SMLib;
offsets remain further work.

The 44 basic Difference outcomes and 65 polyhedral command workflows retain
their saved metadata, history, mass-property and bidirectional boundary checks.
SMLib merges additional coplanar seams: the basic coplanar-cutter case has
10 faces/24 edges versus Rhino's 13/33. Four further Difference partitions are
recorded in [the native seam ledger](smlib-difference-partitions.json). A rounded-UV
result passes its physical checks while the independent exact orientation
witness reports `Unknown`; no signed-volume heuristic replaces that classifier.
These representation differences remain outside topology/parameter parity.

Very thin cylinders remain a confirmed limitation: a radius-1e-5, length-12
Rust cylinder with polar caps makes the supplemental boundary solver expand
pathologically. Its opt-in reproduction is `thin_polar_cylinder_boundary_query_preserves_source`
in the SMLib boundary-validation tests. The native primitive factory's cylinder
at that scale also fails material-boundary export with default context settings.
These cases remain further work.

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
