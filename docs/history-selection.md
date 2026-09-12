# Selection through Undo and Redo

[Document commands](commands/document.md) · [Selection recall](selection-recall.md) · [Oracle](oracle.md)

Undo/Redo exchanges selection with the stored state of each inserted, removed,
or changed object. Selection of unrelated objects remains untouched. Changes
made to selection before Undo become the selection restored by Redo; replay does
not force the command's original output selection back onto those objects.

Replay cleanup preserves selected objects untouched by the entry, including
restricted peers whose selectable companions have become unselected. Objects
edited by the entry, and objects on layers changed by the entry, still undergo
normal eligibility pruning. A live mixed-mesh probe confirms that a locked
connected mesh remains selected through Explode undo/redo while the restored
exploded source is unselected. SplitDisjointMesh restores that source selected.
Only restricted peers enter the temporary preservation set; normally selectable
objects use ordinary cleanup. A read-only regression checks object-edit and
layer-edit exclusions, normal selections, and empty selections.
Successful transaction rollback restores its original selection snapshot without
eligibility pruning, so rejecting a command cannot discard an unchanged locked
peer or alter selection memories and the redo stack.

The selection at the actual object edit matters:

| Command | Source after Undo | Output after Redo |
| --- | --- | --- |
| Move | Original selected source | Selection captured before Undo |
| Delete | Restored selected source | Source absent |
| Explode | Restored, unselected source | Selection captured before Undo |
| DeleteFaces | Whole mesh unselected | Whole mesh unselected |
| ExtractMeshFaces | Whole mesh unselected | Selection captured before Undo |

Explode consumes source selection before replacing its inputs. Native face
arguments stand in for Rhino's sub-object picker, so those edits do not record
whole-mesh selection. The document kernel stores a selection bit with each
object edit and exchanges only that object's bit on replay; it does not clone
the whole document selection for each edit.

## Property history

The document's `object_properties` module snapshots only object identity,
attributes, and isolation mode for name, color, visibility, locking, isolation,
and layer-assignment edits. These edits never clone or replace geometry or
group memberships, including during Undo/Redo and rollback. Replay validates
the expected property state and retains the same selection-exchange behavior
as geometry edits. Missing objects, mismatched identities, and unexpected
properties are errors before mutation.
The same module owns property staging and commit. Its explicit display-mode
policy permits edits to hidden/locked objects and prunes unavailable selection;
the editable-attribute policy requires editable sources and preserves selection.
Both resolve all IDs before invoking changes, deduplicate in table order, and
leave history untouched for no-ops. Editable sources are all checked before
any change callback runs.

Native tests verify exact object restoration and unchanged backing allocation
of a 10,000-vertex polyline through name, color, visibility, lock, and layer
changes, standalone or in caller transactions. They also check invalid replay
is read-only. Geometry edits retain their separate whole-object history states;
this does not change their allocation or replay costs.

## Verification

`undo_selection.json` runs 10 actual Rhino command traces after the setup script
returns to idle, with and without clearing output selection before Undo. An
unrelated point is selected before each replay sequence. Mesh faces use actual
Rhino sub-object selection. The five post-command states check source existence,
whole-object selection, object/output counts, and source mesh face counts.
They do not claim full geometry or pre-command picker equivalence.

```sh
tools/rhino_oracle/run_headless.sh compare tools/rhino_oracle/fixtures/undo_selection.json --timeout 240
```

All 10 traces match exactly. Another 70 complete Move/Delete/group-mode traces
in `last_selection_history.json` and `deletion_recall.json` check selection,
coordinates, and object/layer modes, including hidden/locked group-picked peers.
Native tests compare complete recorded responses. Full sub-object selection
replay, selection-action ordering, and other commands' consumption of input
selection remain outside this coverage.

The idle worker rejects unsuccessful commands and selections, registers inserted
objects before subsequent setup can fail, and attempts all registered cleanup
even if deselection or output discovery fails. A failed baseline scan never
authorizes cleanup of enumerated objects. Atomic error responses discard partial
results; fault-injection tests cover these contracts and repeated finalization.

The document rollback test also runs 256 deterministic mixed-edit sequences over
17 operation types (including document-unit changes), starting with overlapping groups, a locked group peer,
selection memory, and a redo branch. After rejected or deliberately aborted
transactions it compares the complete document/history state with its original
snapshot. This checks rollback invariants, independently of Rhino command parity.
