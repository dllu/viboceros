# Previous selection

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

These fixtures do not establish SelLast's transaction-boundary behavior.
Rhino's enclosing RunPythonScript transaction can include fixture setup objects
in its last-changed set, so that comparison needs independently isolated command
boundaries. Sub-object recall and modifier-key selection remain outside this probe.
