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

`BlockEdit AddObject <object-id> ...` copies external objects into the edit while
retaining the originals in the model. The window's model-object chooser provides
the same operation. `BlockEdit RemoveObject [object-id ...]` releases members to
the model on save; bare input uses preselection or the ordinary member picker.
Released objects stay protected until the edit closes. Discard removes these
temporary copies and releases along with all other workspace changes.
Copied nested references retain attribute text while clearing geometry text;
the external source keeps both text stores, matching the native dialog.

`BlockEdit SetBasePoint x,y,z` sets a world-space insertion reference. The window
also accepts coordinates. The scene stays in its current world frame while
editing; save shifts the shared definition so the chosen base point corresponds
to each root's existing insertion location. Root placements stay unchanged and
released objects retain their original world positions. These controls have
workspace Undo/Redo and fold into the single saved model history entry.

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

Saved member order follows temporary object chronology, including unchanged
saves. Geometry replacement renews that chronology and history restores it.
The workflow oracle distinguishes individual public-SDK transforms from a
batched native Move command.

The document APIs are `open_block_edit`, `block_edit_objects`,
`save_block_edit` and `discard_block_edit`. The oracle's `edit_roundtrip` action
drives Rhino's public BlockEdit lifecycle and translates exposed objects through
the public SDK. Eight private-Xvfb native workflows match 41 recorded model
states for save, discard, translated and reflected placements, nonuniform
rejection, nested references, text and group handling. Captures describe the
closed model; native temporary-object, selection, Undo and panel parity remain
unverified. See [provenance](../block-edit-provenance.json) and the
[validation checkpoint](../validation-checkpoint.md).

See [control validation](../block-edit-controls-provenance.json) for the newer
dialog-button captures, source copying, member release and base-point evidence.
Sixteen workflows capture 82 closed-model states, including discard, unchanged
saves, SDK and command Move, grouped sources, nested references and a reflected
circle. The compared member arrays retain native order and attached metadata.
Native controls are exercised from Rhino's idle event through public WPF/Win32
interfaces, with a getter-prompt handshake and prescribed command-line input.

Nested-definition navigation, viewport picking of external Add Object sources,
interactive base-point picking, double-click opening and linked definitions
remain unimplemented. Saving an
empty definition with existing placements is rejected by the current instance
admission rules. This milestone adds no GPU or performance measurements and does
not establish full Rhino BlockEdit compatibility.

Reference: [BlockEdit](https://docs.mcneel.com/rhino/8/help/en-us/commands/block.htm#BlockEdit).
