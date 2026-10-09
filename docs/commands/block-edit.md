# BlockEdit

`BlockEdit` opens one selected block instance for in-place editing. Command-first
input uses the block picker; scripts can use `BlockEdit Open <object-id>`. The
definition's direct members appear in the selected instance's world frame and
can be changed with ordinary selection, geometry and transform commands. Newly
created geometry becomes part of the edited definition. Nested instances remain
references to their child definitions.

Use `BlockEdit SaveAndClose` to accept or `BlockEdit DiscardAndCancel` to discard.
The persistent **Block edit** window provides the same actions; closing the
window discards. Esc cancels the current modelling getter. Finish or discard the
edit before opening or exporting a file.

```text
BlockEdit
SelAll
Move 0,0,0 2,-3,4
BlockEdit SaveAndClose
Undo
Redo
```

The original model objects remain protected, even if Unlock is issued. Clear
removes the editable members while preserving the original model. Undo and Redo
operate on the temporary workspace while the editor is open. Discard restores
the complete original document, including its prior redo branch. Saving captures
the members back into definition-local coordinates, validates the catalog and
refreshes every original root before accepting one `BlockEdit` history entry.
Save failures leave the workspace open for correction or cancellation.

Saving preserves root IDs, placements, raw attributes, user text and model group
memberships. It clears the edited prototype's member group memberships, matching
the recorded native workflow. New layers are retained; temporary visibility and
locking changes needed to expose member layers are restored on close. Uniform
scales and uniform reflections are supported. Nonuniform scale and shear are
rejected before opening; numeric boundary parity is not established.

The document APIs are `open_block_edit`, `block_edit_objects`,
`save_block_edit` and `discard_block_edit`. The oracle's `edit_roundtrip` action
drives Rhino's public BlockEdit lifecycle and translates exposed objects through
the public SDK. Eight private-Xvfb native workflows match 41 recorded model
states for save, discard, translated and reflected placements, nonuniform
rejection, nested references, text and group handling. Captures describe the
closed model; native temporary-object, selection, Undo and panel parity remain
unverified. See [provenance](../block-edit-provenance.json) and the
[validation checkpoint](../validation-checkpoint.md).

Add Object, Remove Object, Set Base Point, nested-definition navigation,
double-click opening and linked definitions remain unimplemented. Saving an
empty definition with existing placements is rejected by the current instance
admission rules. This milestone adds no GPU or performance measurements and does
not establish full Rhino BlockEdit compatibility.

Reference: [BlockEdit](https://docs.mcneel.com/rhino/8/help/en-us/commands/block.htm#BlockEdit).
