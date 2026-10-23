# SubDSpinEdge

[SubD commands](meshes-index.md) · [Crease editing](crease.md)

`SubDSpinEdge` moves the ends of an interior SubD edge to neighboring vertices
around its adjacent faces. It retains control positions and component identities.
One preselected edge spins immediately. Multiple preselected edges are cleared
and the command opens its picker.

Start the command and pick edges to see each spin. `Direction` toggles clockwise
and counterclockwise; each command starts counterclockwise. `SpinEdgeAgain`
repeats the last spin, and `Undo` reverts one spin within the command. Enter or
Escape finishes and keeps accepted edits. Undoing every spin locally still
records one document-history entry, matching Rhino.

Scripts accept selected SubD parents with a point or `Edge=index`. Explicit
`Direction=Clockwise` and `Direction=CounterClockwise` values are also accepted.
Additional points, edge indices, direction changes, repeats and Undo actions
execute in order before a single history commit. Invalid or stale inputs leave
the document unchanged.

The [qualification record](../subd-spin-edge-qualification.json) covers saved
Rhino source files, physical command-first inputs, previews, metadata, object
order, component selection and Undo/Redo. Valid qualified control topology, tags,
sampled limit stations and density-two facets agree within `1e-6`.

Four native hard-crease/corner cases emit invalid vertex tags. Viboceros repairs
the tags and returns valid geometry; those cases remain explicit native
differences. A captured multi-object local Undo moved one source onto another;
Viboceros restores each actual source. A native grip-enabled probe stalled; grip-state parity remains
unqualified. Failed input probes remain archived separately. Arbitrary topology,
continuous surface error and comparative performance remain unqualified.
