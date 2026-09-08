# Selection recall

[Document commands](commands/document.md) · [Groups](groups.md) · [Oracle](oracle.md)

```text
SelPrev
SelPrev DeselectOthersBeforeSelect=No
```

`SelPrev` recalls recorded object IDs, not their current groups. Adding or
reordering memberships after recording a selection does not enlarge the recalled
set. Deleted objects, hidden/locked objects, and objects on hidden/locked layers
are excluded. This differs from [group picking](groups.md), which can include
hidden/locked peers: recalling that group selection does not reselect those peers.

For ordinary selectable sets A and B, recall updates memory as follows:

| Previous set | Current set | Option | New selection | Next previous set |
| --- | --- | --- | --- | --- |
| A | empty | Yes | A | A |
| A | B | Yes | A | B |
| A | B | No | A ∪ B | A |

Thus repeated replacement recalls can toggle between nonempty sets, but recalling
from empty does not replace useful memory with an empty set. Repeated additive
recalls retain the remembered set. If recalled objects were deleted, replacement
can clear the current selection; the next recall can restore that current set.

The command remembers valid `DeselectOthersBeforeSelect` choices per command
registry, including across documents. A new native registry starts with Yes;
invalid arguments do not change the preference. Selection and preferences do not
create geometry undo entries. Native selection iteration retains action order;
the Rhino fixtures below compare membership, not selection-action ordering.

## Verification

`selection_recall.json` checks 84 actual command sequences with empty, replacement,
and additive recalls, remembered options, overlapping/nested groups, membership
edits, and deletion. `selection_recall_picking.json` checks 52 recalls after actual
idle-viewport clicks, including object/layer modes and Move. All 136 comparisons
match exactly, including geometry and group records where present. Recorded
selection traces for the first fixture and complete responses for the second
are checked by native tests under `tools/rhino_oracle/observations/`.

```sh
tools/rhino_oracle/run_headless.sh compare tools/rhino_oracle/fixtures/selection_recall.json --timeout 300
tools/rhino_oracle/run_headless.sh compare tools/rhino_oracle/fixtures/selection_recall_picking.json --timeout 300
```

## Last changed objects

`SelLast [DeselectOthersBeforeSelect=Yes|No]` recalls changed selectable objects
without expanding their groups. Hidden/locked objects and objects on hidden/locked
layers are excluded, even when the preceding group-picked Move affected them.
It remembers its option independently of SelPrev; adding an empty layer leaves
the recalled objects intact.

`last_selection.json` checks 32 sequences after real idle-viewport clicks and
Move, including overlapping/reordered groups, object/layer modes, explicit and
remembered options, and empty-layer creation. All fields match Rhino exactly;
the full recorded response is a native regression test. Setup returns from
RunPythonScript before Move: otherwise Rhino's enclosing script transaction can
incorrectly include fixture setup objects in the comparison's last-changed set.

```sh
tools/rhino_oracle/run_headless.sh compare tools/rhino_oracle/fixtures/last_selection.json --timeout 300
```

### History

`last_selection_history.json` preserves 16 additional sequences and their full
Rhino observations.
SelLast now retains the changed set through pure deletions and their undo/redo:
surviving members remain recallable, and restored members become recallable
again. Deleting unrelated objects does not replace the remembered set.
`deletion_recall.json` adds 54 sequences covering deletion of moved
members, unrelated objects, both end objects, and all selectable objects.
Complete traces now match Rhino across all 70 sequences, including immediate
Undo/Redo selection, and are checked by native regression tests. These fixtures
were formerly named with a `_diagnostics` suffix while that behavior differed.

Undo can reselect a restored deleted object or hidden/locked peers from a
previously group-picked Move. Selection travels with recorded object states,
not a snapshot of the entire document selection. See [history selection](history-selection.md)
for the independently checked Explode and mesh-face edit behavior.

Import/new-object transaction boundaries, broader history behavior, sub-object
recall, and modifier-key selection remain outside the verified SelLast coverage.
