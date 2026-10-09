# BlockManager

`BlockManager` opens the Block definitions window. It can stay open while a
modeling command is pending. Opening, closing, searching and inspecting the
window do not add model history entries.

The table uses natural name order and shows direct definition-object counts,
top-level instances, nested instances and their total. Nested counts include
each placed occurrence: two parent instances containing two leaf references
produce four nested leaves. An unused parent still references its child in the
catalog, even when both model-use counts are zero.

Select a definition row to rename it, select its selectable top-level instances,
or delete the definition and its model instances. Selection targets those roots
directly, without selecting ordinary group peers. Rename keeps the definition
ID and existing geometry storage. Delete removes all model roots, including
hidden/locked ones, in one Undo step. A definition referenced inside another
active definition cannot be deleted; the button explains that restriction.
Edits end a pending modeling command before changing the document. External
renames preserve a typed name draft and block conflicting application.

```text
Point 1,2,3
SelAll
Block 0,0,0 Part
Insert Part 10,0,0
BlockManager
```

The document API exposes `block_definition_info`, `rename_block_definition`
and `delete_block_definition_and_instances`. Use counts are computed from the
definition graph without expanding or evaluating geometry. The pass runs in
linear time in model objects and catalog references, with checked count sums.

The [native management fixture](../../tools/rhino_oracle/fixtures/block_management.json)
and [capture](../../tools/rhino_oracle/observations/block_management.json) compare
four workflows on Rhino 8.32.26160.13001 under private Xvfb. They cover repeated
and deep nesting, rename, allowed deletion and unused-container restrictions.
The reference adapter implements the published BlockManager deletion guard by
inspecting the active catalog before calling SDK deletion. This is an explicit
manager policy: SDK Delete with reference removal can otherwise delete nested
references. SDK GetReferences(2) can also retain objects from deleted parents;
active catalog references are counted separately from model occurrences.
See [provenance](../block-manager-provenance.json).

This initial pane manages embedded definitions. Linked-file updates, duplication,
export, hierarchical expansion, subobject editing, wildcard search, persistent
window layout and general native panel interaction parity remain unfinished.
The UI tests exercise actions and egui table rendering; they are not GPU captures.
Protected deletion is verified locally against the explicit-ID deletion API,
not by a fresh protected-root native capture.

References: [BlockManager](https://docs.mcneel.com/rhino/8/help/en-us/commands/block.htm#BlockManager),
[UseCount](https://developer.rhino3d.com/api/rhinocommon/rhino.docobjects.instancedefinition/usecount).
