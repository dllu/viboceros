# ReplaceBlock

`ReplaceBlock [All|None] [BlockDefinitionName=]target-name` changes the definition
used by selected block roots while keeping their IDs and affine placements.
All selected sources must use the same definition. None is the default and
changes only those roots; All also changes every model root of the original
definition, including unselected, hidden and locked instances. References stored
inside other definitions remain unchanged.

Root names, layers, display attributes, attribute user text and model groups are
retained. Geometry-attached root text is cleared by replacement, following the
native command. Hidden and locked states are retained for roots included through
All. Explicit source roots must be editable. Definition tables and prototype
geometry are not changed.

Bare `ReplaceBlock` accepts preselection or a block selection prompt, then lets
you pick a visible, unlocked replacement instance. The target supplies only its
definition; its placement, selection and group peers do not change. Empty clicks,
ordinary geometry and selection windows keep the source set intact.

Type or click All or None to set scope. `SelectFromBlockDefinitionList` opens a
searchable chooser in natural name order, including unused definitions. Selecting
a row prepares the choice; Replace or a double-click accepts it. Cancel, Esc or
closing the chooser preserves the model and redo branch. Removed definitions
are reconciled before acceptance, and source permissions are checked again.

`BlockDefinitionName` switches to a name getter, where names such as All, None,
Delete and Part=A are literal names. Quote names containing spaces. The default
target-picking prompt also accepts the scripted syntax above. Invalid names keep
the prompt active and retain the chosen scope. Command-line option buttons work
in either input phase.

```text
Point 1,0,0
SelAll
Block 0,0,0 Original
Point 7,0,0
SelLast
Block 0,0,0 Target
Insert Original 10,0,0
ReplaceBlock None Target
Undo
ReplaceBlock All BlockDefinitionName=Target
```

Geometry and permissions are staged before any edit. An invalid target,
inconsistent source definitions, empty target placement or geometry failure
rejects without partial replacement. Each operation makes one Undo entry; Undo
restores source geometry snapshots, metadata and states.

The document API is `replace_block_instances`. The typed
[workflow oracle](../block-workflow-oracle.md) adds `replace_block`,
`object_state` and optional `record_states`. Six Rhino 8.32.26160.13001 workflows
ran under private Xvfb and constrain single/multiple roots, None/All, hidden and
locked peers, reflected nonuniform placements, metadata/groups and unchanged
nested references. See [replacement provenance](../block-replace-provenance.json).
Three further native captures select the replacement instance through Rhino's
object getter and match all 23 states for None, All with protected peers, and
target visibility/locking transitions back to a normal selectable object.
Local egui pointer-event tests exercise chooser acceptance, cancellation, removed
rows and changed source permissions. See
[input provenance](../block-replace-input-provenance.json).

Layer-level protection, native option memory, native chooser interaction/history
parity, linked definitions, GPU rendering and performance remain unverified. Mixed
definitions in one selected source set reject in this implementation.

Reference: [ReplaceBlock](https://docs.mcneel.com/rhino/8/help/en-us/commands/block.htm#ReplaceBlock).
