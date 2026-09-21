# Join and JoinCopy

[Command reference](README.md) · [Curve joining details](../curve-editing.md)

```text
Join
Join JoinDisjointMeshes=Yes
Join JoinDisjointMeshes=No
JoinCopy
JoinCopy JoinDisjointMeshes=No
```

`Join` and `JoinCopy` accept selected curves or selected polygon meshes. The command-first
workflow filters eligible objects and exposes the remembered mesh option.
Mixed object families, surfaces, B-reps, and SubD joining remain unimplemented.

`JoinCopy` retains all original objects, including their exact geometry, IDs,
attributes, and groups. Outputs inherit their seed's attributes and memberships.
Preselection leaves originals and outputs selected. Command-first mesh copying
and early closed-curve copying retain participating originals selected, with
outputs unselected; ordinary open-curve copying clears selection.
Neither command expands selection to unselected group peers.
Disconnected curves and single selected objects are not duplicated. With
multiple meshes and `JoinDisjointMeshes=No`, even isolated mesh components
produce fresh copies. Both commands use the same staged geometry/document path.

## Curves

Preselection scans document table order and batch-joins all compatible chains,
using majority direction and chord-length parameters for wholly linear batches.
Individual command-first picks extend only the first open curve, in pick order;
skipped curves are not revisited and a second unrelated chain is not started.
The seed's direction and parameter interval are retained in this mode.
An individually picked chain completes immediately on closure, without Enter.
See [cycle seams and command-boundary evidence](../join-cycles.md).
See [curve joining details](../curve-editing.md) for endpoint matching.

A single object fails without geometry edits. Disconnected preselected open
curves succeed unchanged; individually picked curves that cannot extend the seed
fail and leave only the seed selected. Neither case changes undo/redo history.
Existing closed curves are ignored when open curves are present; an all-closed
selection fails and is released without changing geometry or undo/redo history.

## Meshes

`JoinDisjointMeshes=Yes` (the native default) creates one mesh, preserving raw
vertex order, coincident-but-unwelded vertices, unused vertices, polygon types,
face order, and winding. This is not a Boolean union. Mesh alignment can move
naked vertices before concatenation, as described below.

`No` emits connected source components. It compacts raw vertices into first
face-use order and orients adjoining source meshes. A shared vertex can connect
objects even without a shared edge; a vertex-only connection reverses the later
mesh's winding in the recorded cases. Already-disconnected source meshes remain
separate, even when another source bridges their pieces. Ordinary disconnected
inputs are also replaced with fresh IDs. A single selected mesh fails unchanged,
retaining its selection.

The earliest source supplies each output's layer, name, color, and group
memberships. Preselection uses document table order; command-first selection
uses pick order. Preselected results stay selected; command-first results are
deselected. Unselected group peers are not reselected. All results are staged
before mutation and form one undo step; failures preserve the previous redo
entry. Options live in the command registry, outside model history.

## Alignment policy

The [Rhino command documentation](https://docs.mcneel.com/rhino/8/help/en-us/commands/join.htm)
describes alignment of naked mesh vertices. The recorded Rhino 8.32 commands
use a narrower matching distance: `absolute document tolerance * 1e-4`, tested
with document tolerances from `1e-6` to `1`. Matching uses finite binary32
positions, while outputs retain the original binary64 anchor coordinates.
This explains both threshold-boundary rounding and movement larger than the
distance when translated positions coincide in binary32. Native kernel callers
choose the matching precision explicitly; the command selects this measured
Rhino policy.

Interior vertices can anchor naked neighbors without moving themselves.
Alignment is not transitive clustering. Interior anchors are processed first,
then naked anchors in source order.
An unassigned later anchor can reclaim a neighbor assigned to an earlier anchor.
Each moved vertex matches its final anchor directly in the chosen precision.
This avoids moving an entire chain to a remote endpoint. Distinct raw vertex
indices are not welded together.

## Architecture and validation

The command's `join` module separates curve/mesh geometry staging from shared
copying, deletion, direct selection, and preferences. Kernel `mesh/append` performs checked linear
concatenation; `mesh/join` handles matching, connectivity, and orientation.
Candidate matching uses a widest-axis sweep, with 100,000 inputs, one million
matching pairs, and 16 million candidate scans as resource ceilings. Exhausting
a budget or producing a degenerate face fails atomically. Dense spatial queries
can still reach the work ceiling; this is not an end-to-end speed comparison.

The [181-case fixture](../../tools/rhino_oracle/fixtures/mesh_join.json) and
[raw Rhino observations](../../tools/rhino_oracle/observations/mesh_join.json)
cover adjacent/disjoint meshes, triangle/quad mixtures, winding, non-manifold
contacts, unused vertices, disconnected sources, chains, translated coordinates,
interior anchors, tolerance/precision thresholds, reversed selection order, and
pre/postselection. Both engines receive identical double-precision vertices and
face indices, checked before and after Rhino document insertion. Replay compares
every recorded field exactly, without topology, coordinate, or order normalization.
All 181 cases match exactly (maximum numeric error zero). See the
[comparison report](../mesh-join-comparison.json), which records input,
observation, and release-binary hashes.

Independent tests check concatenation arithmetic, 10,000 appended meshes,
2,000 disconnected outputs, 256 randomized alignment cases against a literal
all-pairs reference, bounded direct movement, input preservation, groups,
two undo/redo cycles, error rollback, and the app's object-selection workflow.

The additional [140-case workflow fixture](../../tools/rhino_oracle/fixtures/join_workflow.json)
and [raw observations](../../tools/rhino_oracle/observations/join_workflow.json)
check both commands, both selection modes, source identity/retention, creation
order, layers/colors/groups, no-ops, closed inputs, independent chains, NURBS/line
joins, unrelated nonlinear curves, and raw local/parent parameter domains.
They compare every recorded field, with absolute epsilon `1e-10` and relative
epsilon `1e-12` for numbers; mesh indices and document metadata agree exactly.
See the [workflow comparison](../join-workflow-comparison.json).
Unit tests also verify exact source preservation, group reverse indices, atomic
undo/redo over two cycles, and app command-first picking for both commands.

```sh
tools/rhino_oracle/run_headless.sh compare tools/rhino_oracle/fixtures/mesh_join.json --timeout 420
cargo test --release -p viboceros-oracle join_command::tests
tools/rhino_oracle/run_headless.sh compare tools/rhino_oracle/fixtures/join_workflow.json --timeout 360 --relative-epsilon 1e-12
```

Normals, texture coordinates, per-vertex colors, ngons, restricted-source
deletion behavior, and arbitrary conflicting orientation cycles need further
coverage or implementation. Inputs outside finite binary32 coordinate range
retain binary64 matching natively; Rhino parity for that range is unproven.
These observations establish the recorded cases, not full mesh compatibility.

The four earlier closed-chain discrepancies are resolved in the additional
[284-case cycle suite](../join-cycles.md). Measurements now capture the named
command's EndCommand result and state, not subsequent macro commands.
Batch picks during a command, Undo-within-prompt, edge
subobjects, cross-command sharing of remembered options, and Rhino construction
history associations for JoinCopy still need independent coverage/implementation.
