# Optional curved union and intersection

Linux builds with `--features native-smlib` now stage closed curved solids for
BooleanUnion and BooleanIntersection through the [SMLib bridge](smlib-import.md).
Existing convex/polyhedral command paths run first. Only their explicit
unsupported-geometry errors select the native path; arithmetic, topology,
resource and native failures remain errors. Open sheets remain unsupported.

## Union staging

Each native input is an independent copy. Pair intersections establish equal
or enclosed regions, and face-level intersections identify non-point boundary
contact. The contact query preserves inputs and propagates solver failures.
Related inputs form connected groups; a group must contain an actual boundary
interaction to produce a replacement. Equal, disjoint and strictly nested-only
groups remain unchanged. Interior inputs join a changing group and are consumed,
while unrelated singleton objects retain their original IDs and selection.

Contributors with exposed material determine ownership: the first supplies
attributes/groups and the last supplies geometry user text for a single result.
Material-body extraction keeps cavities with their body and separates disconnected
material outputs. MergeCoplanarFaces uses the existing Rust planar face merge
where supported. Broader coincident-face ownership and preservation of every
original native face partition remain under qualification.

## Intersection staging

Common intersection reduces inputs in order. An enclosing or equal input leaves
the current owner unchanged; a fully replacing smaller input takes ownership;
a cutting input supplies the latest geometry user text. Empty common material
reports no changes. Two-set intersection computes pair regions and unions them
before material-body extraction. Maximal first-set pair contributors determine
attributes and geometry text; multiple outputs for an owner clear that text.

Work is bounded to 128 inputs and 128 retained pair regions. Independent Rust
volume integration supplies dimensional no-change and coverage comparisons;
native status failures are never converted to no-change answers. Tiny features,+near-equality, broad compound configurations, tangencies and output ordering
still need qualification. These checks do not establish every Rhino policy.

Both paths prepare geometry and metadata before any model edits, then use the
existing selection, DeleteInput and one-step history logic. Default builds retain
the Rust implementation. BooleanSplit, fillets and offsets are not yet connected
to the native kernel.

## Reproduce the workflow

```sh
GIT_LFS_SKIP_SMUDGE=1 git submodule update --init third_party/usd-brep
CMAKE_BUILD_PARALLEL_LEVEL=10 cargo run --release --features native-smlib
cargo test -p viboceros-command --release --features native-smlib
cargo run -p viboceros-command --release --features native-smlib \
  --example curved_sets -- /tmp/viboceros-curved-sets
```

The example constructs a Rust box and a sphere crossing its upper face, runs
union, common intersection and two-set intersection, checks analytic volumes,
and exports editable 3DM results. The existing Rhino interchange probe can read
the generated request on private Xvfb. Rhino 8.32 accepts the three captured
artifacts as valid solids. Their 594 edge and 891 surface samples match the
analytic boundaries/surfaces within 1e-8, with observed maximum errors of
5.33e-15 and 1.78e-15 respectively. These samples do not prove whole-boundary
equivalence or interactive command parity. Native display-mesh diagnostics are
retained separately. [Provenance](smlib-sets-provenance.json)
records actual checks, geometry captures and remaining differences.
