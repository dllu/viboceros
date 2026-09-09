# RemoveFromGroup

[Command reference](README.md) · [Group model](../groups.md)

```text
RemoveFromGroup
RemoveFromGroup Copy=Yes
```

`Copy=No` (the native default) removes every group membership from selected
grouped objects, not just the last membership. Their identities, geometry,
attributes, and selection are retained; unselected peers are unchanged. Empty
group definitions remain. This differs from top-level `Ungroup`.

`Copy=Yes` leaves grouped originals intact and makes ungrouped, selected copies
in place. Copies retain source geometry and attributes. Originals are deselected;
no extra group definitions are created. Both modes use one model transaction,
with native Undo/Redo coverage. Invalid options or no eligible objects fail
without edits or loss of redo.

Membership removal resolves selected objects in one batch, without a repeated
object-table scan per source or geometry copies. The document batch API validates
all requested IDs and membership transitions before editing, also inside an
existing transaction. Tests cover duplicate IDs, ungrouped no-ops, retained group
definitions, unchanged peers, failure atomicity, and ordered Undo/Redo.

Without eligible preselection, the UI collects grouped objects individually,
without expanding picks to their group peers. Ungrouped objects are filtered
from click/window selection and `SelAll`. Enter finishes; `Copy=Yes`/`Copy=No`
changes the mode during selection, and Esc cancels before membership edits.

Four private Rhino probes use command-first selection with an explicit Copy
option. They cover a single object in overlapping groups and an entire group in
both modes. Complete records of geometry, names, source identity, memberships,
group tables, and selection match native replay. Independent Python tests check
the retained originals and selected ungrouped outputs.

```sh
tools/rhino_oracle/run_headless.sh compare tools/rhino_oracle/fixtures/remove_from_group.json --timeout 180
```

The [Rhino documentation](https://docs.mcneel.com/rhino/8mac/help/en-us/commands/group.htm)
also describes `RememberCopyOptions`; that cross-command preference is not
implemented here. Native invocations currently default to `Copy=No`. The probes
do not establish every preselection/modifier gesture, hidden-member case, or
Rhino outer Undo/Redo behavior.
