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

Bare `ReplaceBlock` accepts preselection or a block selection prompt, then asks
for a target definition name. Type All or None during that name prompt to set
scope. Invalid or missing names keep the prompt active; Cancel preserves the
model and redo branch. The current UI uses name entry; picking a replacement
instance or opening the native definition-list chooser remains unfinished.
Quote target names containing spaces, or quote the whole name option token.

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
nested references. See [source/capture provenance](../block-replace-provenance.json).

Layer-level protection, native option memory, picking/history parity, linked
definitions, replacement-list UI and performance remain unverified. Mixed
definitions in one selected source set reject in this implementation.

Reference: [ReplaceBlock](https://docs.mcneel.com/rhino/8/help/en-us/commands/block.htm#ReplaceBlock).
