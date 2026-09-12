# ImportStep

```text
ImportStep "part.step"
ImportStep Native=Yes "part.step"
```

The default imports display meshes. `Native=Yes` imports editable planar B-reps
without tessellation, with assembly placements, occurrence names, source units
converted to document units, and objects assigned to the current layer.
Quote filenames beginning with `Native=Yes` to treat that text as a filename.
The `ImportStp` alias accepts the same syntax.

Native mode supports straight-edged planar faces, including valid polygon holes.
Unsupported shell curves/surfaces, missing trims, or invalid topology fail the
import; there is no automatic mesh fallback. Assembly and representation warnings
follow the existing STEP importer and are counted in the result message.

All shells belonging to one shape occurrence are combined into one object.
Outer/cavity orientation is retained, and coincident topology is not welded.
This is topology assembly, not a Boolean union or validation of solid cavity
containment. Distinct occurrences remain separate objects, even with equal names.

Geometry is converted and combined before insertion. Import is one undoable
command; failures preserve the document and its undo/redo history. Tests cover
quoted/repeated-space paths, physical unit conversion, editable object type,
Undo/Redo, malformed files, and a hollow cube imported as one object of volume 992.
A mixed-file regression begins with two supported shells, changes only the
later shell's plane to an unsupported cylinder, and verifies loss-free STEP
record parsing before import. Both `ImportStep Native=Yes` and
`ImportStp native=yes` fail on that later shell with the complete document and
redo history unchanged; the previously undone command remains redoable.

See [file formats](../file-formats.md) and the
[native STEP conversion boundary](../step-brep-boundary.md) for limitations.
