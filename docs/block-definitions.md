# Native block definition catalog

The document crate stores shared, immutable block definitions with typed IDs,
names and ordered members. A member contains either a geometry snapshot or a
reference to another definition. References retain the affine map from child
coordinates to parent coordinates. Definitions remain hierarchical; placed
geometry returned by `Document::resolve_block` is derived data.

The object table now supports [instance objects](block-instances.md), retaining
definition IDs and placements through editing, display, selection and history.
[Block/Insert commands](commands/blocks.md) create and place these definitions.
[Structural 3DM interchange](three-dm-structural-blocks.md) retains supported
embedded definitions and references. [BlockEdit](commands/block-edit.md) adds
in-place member editing with save/discard and native workflow captures;
advanced editor controls and performance qualification remain open.
The [source provenance](block-definition-provenance.json) records the local API
boundary and validation evidence.

## Editing and admission

`add_block_definition`, `replace_block_definition_members`,
`remove_block_definition` and `set_block_definitions` validate the complete
catalog before mutation. Batch replacement supports forward references. Names
are trimmed and unique under ASCII case folding, following the local layer API.
Empty/NUL names, duplicate IDs, missing definitions/layers, cycles and singular
placements are rejected. Reflections are allowed. These are local API policies,
not measured Rhino command semantics.

Catalog changes participate in document transactions and Undo/Redo as one edit.
Unchanged replacements preserve redo history. Geometry snapshots and immutable
member arrays are shared with prior states; replacing a definition changes later
resolution of every reference to its ID. Layers used by definition members cannot
be deleted. Ordinary object removal does not delete definitions.

Admission permits at most 10,000 definitions, 100,000 total stored members and
64 definitions on a path. Graph validation memoizes subtree heights, retaining
depth checks when child definitions are visited first or shared by many parents.
Placement resolution additionally permits at most 1,000,000 definition/member
visits and 100,000 placed leaves. The visit bound also limits empty branching
graphs that produce no geometry.

## Resolution and units

`BlockReference::try_new` rejects singular placements.
`resolve_block` composes child maps before parent maps and transforms each leaf
with the checked geometry kernel. Primitive validation uses numerical tolerance,
mesh validation uses mesh tolerance, and B-rep reconstruction uses document
matching tolerance. Existing short features do not acquire a model-tolerance
minimum length. Identity placements share the original geometry snapshot.

Each result includes raw leaf object attributes, geometry user text and an
ordered definition/member-index path. Repeated placements have distinct paths.
Hidden/locked members are retained. The separate instance display adapter resolves
hierarchical color, visibility and locking without altering these raw results.
Resolution failures return no
partial result and leave document state and history untouched.

Members use definition-local coordinates in the current document units.
`set_units(..., true)` scales both stored geometry and reference translations;
reference linear coefficients remain unchanged. This preserves nested placement
size without multiplying scale at every level. Both definition and object tables
finish all fallible transformations before mutation. Undo/Redo exchanges their
immutable snapshots together. Metadata-only unit changes retain numeric data.

## Verification

```sh
cargo test --release -p viboceros-document blocks::
```

Regressions cover noncommuting nested placement, shared references, raw metadata,
snapshot identity, shared-definition replacement, transaction rollback, history,
invalid graph admission, deletion constraints, graph/output resource bounds,
reflections, short features and late placement/unit failures. Unit tests cover
definition failures and object failures after definitions have been staged.
