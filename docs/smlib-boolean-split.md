# Native closed-solid BooleanSplit

Linux builds with `--features native-smlib` route closed planar and curved operands
directly through the [SMLib bridge](smlib-import.md), without preparing a Rust
Boolean arrangement first. Arithmetic, work-limit, topology and native failures
propagate before model changes. Finite-plane and open-sheet operands retain their
qualified Rust paths; builds without the feature use the Rust implementation.

## Partition and lineage

Native imports copy operands. For each target material body, cutters must have
nonempty inside and outside material and non-point boundary contact. Kernel set
queries make those decisions; integrated volume is a conservation check, not a
split threshold. Strictly
internal, enclosing, equal, disjoint and self cutters do not split that body.
Only crossed material bodies enter the partition, matching the existing compound
target policy; unrelated bodies do not become replacement objects.

Accepted cutters run in getter order. Each current piece produces inside and
outside regions; both must be nonempty and their independently integrated Rust
volumes must conserve the parent volume within dimensional document tolerance.
Native material-body extraction keeps cavities attached and separates disconnected
branches. A disconnected branch clears geometry user text for all its children;
later cuts carry that state. Single connected children retain their target's
geometry text. Final output volume must match the crossed target material.
Partition/output counts are bounded to 4,096.

A unit-volume corner partition of a 100,000-unit cube qualifies the small-piece
path: both solids survive even though the small branch is below the dimensional
conservation threshold. Geometry text, cutter retention and one-step undo are
checked. The Rust flux integration now chooses its reference from topological
vertices; a retained supporting surface can have a much larger control hull than
the trimmed result, and using that hull's center caused cancellation in this
small branch's volume query.

All outputs are staged before document mutation. Results use existing target
attributes, layer, groups, selection, DeleteInput preference and one-step history.
Only split targets are deleted; cutters remain. Unchanged targets retain their
identity. Preselected results are selected, command-first results are unselected,
and retained inputs are released from command selection.

## Reproduce the workflow

```sh
GIT_LFS_SKIP_SMUDGE=1 git submodule update --init third_party/usd-brep
CMAKE_BUILD_PARALLEL_LEVEL=10 cargo run --release --features native-smlib
cargo test -p viboceros-command --release --features native-smlib
cargo run -p viboceros-command --release --features native-smlib \
  --example curved_split -- /tmp/viboceros-curved-split
```

The example constructs a Rust cylinder and slab, runs the document command,
checks three solid pieces and total analytic volume, and exports separate 3DM
files plus the existing Rhino interchange request. Native checks must use
`tools/rhino_oracle/run_headless.sh` on a private Xvfb display.
Trimmed pieces retain their underlying surface definitions. Conservative surface
bounds can therefore span the original cylinder height; the example records
partition vertex ranges and oriented volume instead.
Rhino 8.32 accepts all three captured 3DM pieces as valid solids. Their vertex
ranges cover 0–4, 4–6 and 6–10, and 1,155 edge/lifted-trim samples lie on the
expected boundaries within 1e-8 (observed maximum 2.67e-15). These checks qualify
the artifacts; they do not establish full interactive command parity.
[Provenance](smlib-split-provenance.json) records obtained checks and artifacts.

## Remaining qualification

The native path currently accepts closed solids only. Curved open-target and
sheet-cutter workflows, tangent/coincident cuts, very small features, extreme
scales, general compound/region configurations and Rhino output order require
further work. Native operations consume rounded NURBS outputs between successive
cuts; this differs from the Rust polyhedral path's one exact original-face
arrangement. Every stage is validated and volume-checked, but these checks are
not a complete proof of geometric partition equivalence. STEP pole trims and
circular edge fillets are available through the adapter; offsets remain further work.
