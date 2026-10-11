# Native union and curved intersection

Linux builds with `--features native-smlib` route BooleanUnion directly through
the [SMLib bridge](smlib-import.md) for closed planar and curved solids. The old
Rust Union adapter and shell participation helpers are compiled only without
that feature. BooleanIntersection still tries its Rust paths before selecting
native curved geometry after explicit unsupported-geometry errors. All native
failures propagate before document changes. Open sheets remain unsupported by
the native adapter.

## Union staging

Each native input is an independent copy. Kernel Difference emptiness establishes
equal or enclosed regions, and face-level intersections identify non-point boundary
contact. The contact query preserves inputs and propagates solver failures.
Related inputs form connected groups; a group must contain an actual boundary
interaction to produce a replacement. Equal, disjoint and strictly nested-only
groups remain unchanged. Interior inputs join a changing group and are consumed,
while unrelated singleton objects retain their original IDs and selection.

Input boundaries that contact the resulting boundary determine ownership: the
first supplies attributes/groups and the last supplies geometry user text for a
single result. This includes contained inputs that contribute coincident exterior
faces; earlier equal inputs retain priority. Volume thresholds do not decide
coverage or ownership, so small contributing regions survive.

Original connected shells are queried independently, using the same helper as
Difference. Only participating output boundaries are retained. Rhino's captured
Union behavior emits opened cavity boundaries as separate outward objects and
omits inactive detached shells and unopened cavities. The native command follows
that policy, while the library's material-body API continues to retain voids.
Multiple outputs clear geometry user text.

MergeCoplanarFaces=No disables native Boolean topology cleanup to retain imprinted
seams. Yes enables cleanup and uses the existing Rust planar merge where supported.
The kernel can retain different partitions from Rhino; [the seam ledger](smlib-union-partitions.json)
records those differences independently of metadata, mass and boundary checks.
Broader coincident-face ownership and complete topology/parameter parity remain
under qualification.

## Intersection staging

Common intersection reduces inputs in order. An enclosing or equal input leaves
the current owner unchanged; a fully replacing smaller input takes ownership;
a cutting input supplies the latest geometry user text. Empty common material
reports no changes. Two-set intersection computes pair regions and unions them
before material-body extraction. Maximal first-set pair contributors determine
attributes and geometry text; multiple outputs for an owner clear that text.

Work is bounded to 128 inputs and 128 retained pair regions. Intersection still
uses native volume queries and dimensional tolerances for ownership and coverage;
Union uses kernel set and boundary predicates directly. Independent Rust mass
and boundary comparisons qualify the saved fixtures;
native status failures are never converted to no-change answers. Tiny features,
near-equality, broad compound configurations, tangencies and output ordering
still need qualification. These checks do not establish every Rhino policy.

Both paths prepare geometry and metadata before any model edits, then use the
existing selection, DeleteInput and one-step history logic. Default builds retain
the Rust implementation. [Closed curved BooleanSplit](smlib-boolean-split.md)
also has a native path, as do [circular edge fillets](fillet-edge.md). Offsets
remain further work.

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
