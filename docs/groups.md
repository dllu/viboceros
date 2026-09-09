# Groups and ordered membership

[Command reference](commands/README.md) · [Architecture](architecture.md) · [Oracle](oracle.md)

```text
Group Assembly
Ungroup
UngroupAll
AddToGroup Assembly
```

`Group` appends a new membership to each selected object. An object's top group
is its **last membership**, not the newest definition in the group table.
Membership lists can therefore differ even for objects belonging to exactly the
same groups. This agrees with the open-source
[OpenNURBS attributes implementation](../third_party/opennurbs/opennurbs_3dm_attributes.cpp)
and direct Rhino 8 observations.

[`RemoveFromGroup`](commands/remove-from-group.md) detaches all memberships
from individual selected members, or creates selected ungrouped copies with
`Copy=Yes`. It leaves unselected peers and group definitions intact.

Bare `Ungroup` removes only the last membership on each selected object;
`UngroupAll` removes all memberships on those objects. Both retain empty group
definitions and leave unselected peers unchanged. These are the
[documented nested-group commands](https://docs.mcneel.com/rhino/8mac/help/en-us/commands/group.htm).
Ordinary group-aware picking can expand selection before a command starts;
these commands themselves do not expand it.

Deleting objects likewise removes their memberships, not their group definitions.
An empty group remains addressable, and undo restores memberships in their original
order. Explicit group deletion remains a separate operation.

Viboceros also retains explicit document-management extensions: `Group all Name`
groups every selectable object; `Ungroup Name` deletes a named definition and
all of its memberships; `Ungroup all` deletes every definition in the document.
The last form is distinct from the selection-scoped `UngroupAll` command. The
groups pane uses the named deletion form. These extensions are not claims about
Rhino command syntax.

`AddToGroup group-name` adds preselected objects to an existing named group.
Names can contain spaces and are case-sensitive. Only selected objects are
modified; unselected peers are not pulled into the target. New memberships
append, existing memberships keep their positions, and selection is cleared
even for an all-existing no-op. Empty group definitions are valid targets.
Missing names, missing groups, or empty selection produce an error without edits.
The UI also accepts bare `AddToGroup`: pick source objects and press Enter, then
pick a grouped object or type the target group name. Preselection skips source collection, and
`AddToGroup group-name` with no selection collects sources for that named target.
Source clicks are additive; removal modifiers, window selection, `SelAll`, and
`SelNone` can correct the source set. Target entry fixes the source selection;
invalid names remain retryable. Esc cancels without membership edits. CPlane and
interface commands remain transparent; another model command or a sidebar action
ends the prompt. Clicking a target uses that object's last ordered membership,
including unnamed imported groups. Clicking an ungrouped/nonselectable object
does not complete the command; target-window gestures do not change sources.
Four owned-window Rhino mouse probes verify ordinary, overlapping, reversed,
and already-existing target memberships in `add_to_group_picking.json`.
Native replay compares memberships, selection, object modes, and layer flags;
independent Python expectations check the chosen group and membership order.
This does not establish every ambiguous-pick menu or modifier-key behavior.
Native Undo/Redo is one transaction for
actual additions; a membership no-op creates no history entry.

Three private Rhino `AddToGroup` probes verify partial selection of a group,
repeat/all-existing additions, and an empty target. The complete ordered
memberships, reverse group index, retained geometry, names, and selection match
the saved observations in `add_to_group.json`; native and independent Python
tests replay/check them. These probes do not claim Rhino outer Undo/Redo parity.

```sh
tools/rhino_oracle/run_headless.sh rhino tools/rhino_oracle/fixtures/add_to_group_picking.json --timeout 180
```

## Model and transactions

`Object::group_ids()` exposes an ordered, duplicate-free list;
`Object::top_group()` reads its last entry. `Group::members()` is a reverse
index used for group lookup and group-aware selection. The document's `groups`
module owns membership mutation and validates both directions before updating
either. Adding an existing membership is a no-op. Reordering existing entries
is a real edit. Missing definitions, missing objects, and duplicates in an
explicit replacement are rejected without a partial membership update.

Membership history stores object IDs and before/after group-ID lists, not
geometry copies. Group creation/removal, membership changes, object deletion,
geometry changes, and clear-document operations compose in the same transaction.
Undo/redo and rollback restore exact membership order and both indices.

Group-aware picking expands only each picked object's **last membership**.
With groups `[A, B]` and `[B, C]`, picking A selects A/B; picking B or C
selects B/C. Reversing B's membership list changes B's pick to A/B, without
changing the other picks. Members' other groups do not recursively expand.
The former connected-component implementation and its performance benchmark
were removed because they modeled the wrong selection policy.

A hidden or locked object cannot be picked directly, but is included when a
selectable peer's top group contains it. This also applies to hidden or locked
layers. Selected locked members can be edited without unlocking them; Move
preserves those visibility/lock states. Direct edits of unselected locked objects remain
rejected. History pruning retains already-selected members reached through a
selectable peer, without adding new objects. Explicit visibility/lock edits
still prune the selection.

Temporary object/layer indices and a deduplicated set of picked group IDs keep
each selected group from being expanded repeatedly. Native tests exercise all
three-object group tables, independent membership reversals, seed/eligibility
combinations, a 1,024-object chain, selection action ordering, and grouped edits
through undo/redo and rollback.

3DM import first creates the complete group table, including empty definitions,
then assigns each object's file-ordered memberships. Export writes that order
back. Repeated import with name collisions, hidden/locked objects, and undo/redo
are covered by native interchange tests.

## Commands and copied groups

`Distribute` uses the top membership to form its selected rigid units. Adding
an object to an older group later changes that unit; re-adding an already
present membership does not. See [distribution](commands/distribute.md).

Multi-source `Copy`, `Array`, `ArrayLinear`, and `ArrayPolar` recreate touched groups on first use,
walking source objects in document order and each source's ordered membership
list. Changing selection-action order does not change this mapping in the
tested Rhino cases. Originals keep their memberships and selection; copies
receive the ordered remapped IDs and are unselected. Single-source Copy/arrays
leave copies ungrouped but still allocate corresponding empty definitions.
Affine/morph/layer copy APIs share ordered group reconstruction; commands that
produce pieces or extract geometry preserve their source membership order.
`Explode` selects its outputs, including point-cloud members, without selecting
untouched group peers. An older point-cloud probe used in-command selection
while its native counterpart used preselection; the probe now matches the
native workflow. Postselection equivalence is not claimed.

New copied definitions use automatic `GroupNN`-style names. Native naming finds
the first unused **live** name; it does not model Rhino's reservation of deleted
automatic names. The oracle normalizes only automatic numeric names by relative
numbering within a case, leaving explicit names and wrong naming styles intact.
Consequently these comparisons verify naming style and relative group mapping,
not absolute counters in a reused Rhino document.

## Verification and limits

The 52 `group_picking.json` cases use actual idle-viewport mouse clicks in an
owned private Rhino window, rather than SelID or preselecting through the API.
They cover overlapping/reordered/nested/empty groups, hidden/locked objects and
layers, and Move after picking. Selection, object modes, layer flags, and moved
coordinates match exactly. The recorded Rhino output is retained under
`tools/rhino_oracle/observations/` and checked by native tests. Separate public
GetObject and command-first Move mouse probes confirmed the overlapping-group
and object-mode cases. This does not establish every command's selection filter,
modifier-key behavior, or window/crossing selection semantics.

Reproduce the idle-click comparisons on a private display:

```sh
tools/rhino_oracle/run_headless.sh compare tools/rhino_oracle/fixtures/group_picking.json --timeout 300
```

The 56 `group_memberships.json` comparisons record every step's ordered memberships,
reverse member lists (including empty definitions), source identity, names,
selection, domains, and sampled geometry. Their epsilon is absolute `1e-8`,
relative `1e-12`; maximum observed coordinate error is `1.34e-15`.
Native tests additionally exercise all three-membership
permutations, first-use copy mapping, group/object deletion, invalid edits,
mixed transaction rollback, repeated undo/redo, and command-level 3DM round-trips.
Decomposition/extraction regressions exercise `ConvertToSingleSpans`,
`ConvertToBeziers`, curve/isocurve `Split`, `Explode`, `SplitDisjointMesh`,
`ExtractMeshFaces`, and `ExtractDuplicateMeshFaces`.

The former `ConvertToBeziers` mismatch now passes: fresh pieces are unnamed,
ungrouped, unselected, and unit-domain. Three additional real `Delete` probes
verify retention of empty definitions. The old diagnostic was merged into the
regular fixture; no comparison fields or epsilons were relaxed. Nameless outputs
are attributed only when a command has one unambiguous source; names remain
explicit comparison fields. See [Bézier conversion](commands/beziers.md).
The separate 78 [single-span conversion](commands/single-spans.md) comparisons
also verify fresh ungrouped outputs, retained no-op sources, and empty definitions.
The previous native expectations of inherited attributes/groups and in-place
source replacement for that command were incorrect and have been removed.

A private GUI smoke test imported differently ordered memberships and an empty
group, distributed six points, exercised undo/redo and interactive Copy, and
ran both ungroup commands. Four 3DM exports retained exact expected point
coordinates, ordered memberships, empty definitions, names, and layers.

Python tests check fixture and macro whitelists, missing sources, completed
preselection, duplicate-add no-ops, and cleanup after partial failures. The
worker restores existing objects, groups, selection, all CPlanes, and universal
CPlane mode; only probe-owned objects/groups are removed, including new groups
that reuse deleted table slots. Probes are untimed and do not establish kernel
performance parity or complete Rhino command compatibility.
