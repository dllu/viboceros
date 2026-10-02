# Remove interior surface holes

[Command reference](README.md)

The typed command accepts one accessible surface/B-rep component:

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

The viewport prompt, option subprompts, component preselection, window selection,
and grouping multiple accepted picks into native command history remain pending.
Native Esc keeps accepted edits; the saved cases also cover internal Undo and
repicking. The oracle replays those component sequences through typed edits and
document history; these comparisons do not certify a viewport workflow.
The native replay rejects simultaneous multi-component preselection and mouse
indices on edited topology until those mappings are implemented.

See [kernel and native evidence](../geometry/remove-holes.md). All live captures
use private Xvfb displays. Replaying saved observations requires no Rhino GUI:

```sh
cargo test -p viboceros-command untrim_holes
cargo test -p viboceros-oracle untrim_holes
python3 -m tools.rhino_oracle.untrim_replay tools/rhino_oracle/fixtures/untrim_holes_components.json tools/rhino_oracle/observations/untrim_holes_components.json --timeout 300
python3 -m tools.rhino_oracle.untrim_replay tools/rhino_oracle/fixtures/untrim_holes_undo.json tools/rhino_oracle/observations/untrim_holes_undo.json --timeout 300
python3 -m tools.rhino_oracle.untrim_replay tools/rhino_oracle/fixtures/untrim_holes_limits.json tools/rhino_oracle/observations/untrim_holes_limits.json --timeout 300
```
