# Remove interior surface holes

[Command reference](README.md)

Run `UntrimHoles` to pick accessible surface/B-rep edges in a viewport.
Ctrl+Shift (Command+Shift on macOS) click or drag selects components before
starting the command. Edge clicks take priority over faces and toggle selection;
rectangles add components. Hidden or locked sources cannot be selected.
`All=Yes` switches to face picking, including polysurface faces. Overlapping
edges offer numbered choices with hover highlighting. The command line and
small option buttons accept `All`, `MaximumEdgeLength`, and `KeepTrimObjects`;
bare option names open value subprompts. Enter keeps a subprompt's current value.
Typed invocation accepts one component:

```text
UntrimHoles object-id component-index All=No MaximumEdgeLength=0 KeepTrimObjects=Yes
```

`All=No` interprets the zero-based component index as an edge and removes the
inner loop containing it. `All=Yes` interprets it as a face and removes its
eligible inner loops. Outer-edge picks and faces without eligible holes succeed
without geometry edits or new history. Whole-object selection is insufficient.
The three options are remembered by each command registry, outside Undo.

Zero disables `MaximumEdgeLength`. A positive limit applies to the complete
hole perimeter, including every boundary edge, with equality accepted. The
42 native split-edge cases test square perimeters of eight units and rectangle
perimeters of ten units, selecting either short/long edges or their face.

Removal follows joined hole walls and closes their other openings. Exact
surviving surfaces, UV trims, edge curves, domains, tolerances, and orientation
are preserved. `KeepTrimObjects=Yes` retains naked-hole curves or detached
B-rep walls with current-layer defaults, no name or groups, and no selection.
Retained objects precede the renewed source; its identity and attributes remain.
Retained multi-edge curves follow trim-loop order and keep native edge parameter
intervals; they are not assigned chord-length intervals by the hole command.
Each typed component edit is atomic and restores geometry, retained objects,
metadata, and ordering in one Undo/Redo step.

`UntrimHolesSelection::prepare` provides a read-only, fully validated edit for
component tools. `commit` applies it immediately. Geometry, tolerance, visibility,
and locking changes invalidate prepared picks before any edit. The source is
unchanged when preparation or commit fails.

Each viewport pick applies immediately. `Undo` during the prompt discards the
last accepted pick and its retained objects. Enter, Esc, and starting another
geometry command finish the prompt and keep completed edits. One subsequent
external Undo/Redo restores the entire command, including ordering and metadata.
View and CPlane commands preserve the picking session. Other document edits
invalidate its continuation token, so local Undo cannot reverse a foreign edit.
`commit_in_group` exposes this behavior through `Document::begin_history_group`.
No edit transaction remains open while waiting for the next pick.

Preselection uses the remembered options before accepting new option tokens.
Only edges qualify when All=No; only faces qualify when All=Yes. Other component
kinds are ignored. Multiple distinct eligible components make Rhino return
Failure without edits; duplicate references to one component are processed once.
Invoking the command clears component selection, including rejected and ignored
selections. External Undo/Redo does not restore that transient selection.

During the prompt, drag left-to-right to enclose a component or right-to-left to
cross it. Rectangle selection accepts exactly one eligible component; several
eligible components leave geometry unchanged and keep the prompt active. Four
edges of one split hole still count as four components. Partial windows do not
select an entire face; a crossing rectangle can select its filled area in Shaded
or Ghosted mode. Inverse window and crossing modes share the object-selection
predicates. Ctrl+Shift and plain rectangles have the same measured command result.

The 136 saved native cases include 16 multiple-pick/history captures and 26
component-selection captures. New rectangles start on empty space to avoid an
immediate face pick. They compare geometry, attributes, and transient component
selection before/after the command and external Undo/Redo. The offline rectangle
probe covers Top-view planar polygons with bilinear surfaces; unsupported sources
are rejected. Production viewport selection uses projected real edges, face
meshes, and depth clipping. Mouse sequences match surviving components after
topology compaction by exact curves/surfaces, rejecting ambiguous or missing
matches. Rust tests also exercise real egui click/drag events, options, ambiguity,
cancellation, CPlane interaction, stale sources, and local/external Undo. Native
option changes between accepted picks remain unmeasured. Generic mesh/SubD
component preselection remains pending. [UnjoinEdge](unjoin-edge.md) also
consumes surface/B-rep component preselection.

See [kernel and native evidence](../geometry/remove-holes.md). All live captures
use private Xvfb displays. Replaying saved observations requires no Rhino GUI:

```sh
cargo test -p viboceros-command untrim_holes
cargo test -p viboceros-oracle untrim_holes
cargo test -p viboceros app::tests::untrim_holes
python3 -m tools.rhino_oracle.untrim_replay tools/rhino_oracle/fixtures/untrim_holes_components.json tools/rhino_oracle/observations/untrim_holes_components.json --timeout 300
python3 -m tools.rhino_oracle.untrim_replay tools/rhino_oracle/fixtures/untrim_holes_undo.json tools/rhino_oracle/observations/untrim_holes_undo.json --timeout 300
python3 -m tools.rhino_oracle.untrim_replay tools/rhino_oracle/fixtures/untrim_holes_limits.json tools/rhino_oracle/observations/untrim_holes_limits.json --timeout 300
python3 -m tools.rhino_oracle.untrim_replay tools/rhino_oracle/fixtures/untrim_holes_history.json tools/rhino_oracle/observations/untrim_holes_history.json --timeout 300
python3 -m tools.rhino_oracle.untrim_replay tools/rhino_oracle/fixtures/untrim_holes_selection.json tools/rhino_oracle/observations/untrim_holes_selection.json --timeout 300
```
