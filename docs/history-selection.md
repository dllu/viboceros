# Selection through Undo and Redo

[Document commands](commands/document.md) · [Selection recall](selection-recall.md) · [Oracle](oracle.md)

Undo/Redo exchanges selection with the stored state of each inserted, removed,
or changed object. Selection of unrelated objects remains untouched. Changes
made to selection before Undo become the selection restored by Redo; replay does
not force the command's original output selection back onto those objects.

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
16 operation types, starting with overlapping groups, a locked group peer,
selection memory, and a redo branch. After rejected or deliberately aborted
transactions it compares the complete document/history state with its original
snapshot. This checks rollback invariants, independently of Rhino command parity.
