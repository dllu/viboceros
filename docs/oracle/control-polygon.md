# Control-polygon extraction observations

[Rhino oracle](../oracle.md) · [SubD geometry](../subd.md)

Three immutable Rhino 8.32 captures retain 61 workflows. Of these, 42 have
complete command traces without unknown input: 32 produce control polygons and
ten reject unsupported sources. Nineteen malformed option trials remain
diagnostics. Every case records source/result 3DM files, command events, layers,
names, colors, groups, selection and Undo/Redo. Source geometry CRCs stay unchanged.

The captures establish support for curves, single surfaces, trimmed single-face
B-reps and SubDs. Whole boxes, capped extrusions, points and meshes are rejected;
unsupported preselected peers remain selected when an eligible source succeeds.
Results inherit source names, object colors and group memberships. `OutputLayer`
chooses the current or input layer. Undo and Redo release command selection.
The archive fixtures retain geometry and layers; tests reconstruct source groups
from the captured snapshots because the fixture exporter removes group indices.

SubD geometry priority preserves control topology and interior crease seams;
n-gons retain their boundary and member-face tables. Texture-coordinate priority
creates a different mesh with per-face vertices and texture coordinates. For the
captured cube, these modes produce eight and 24 vertices respectively. These
outputs must use the public native conversion rather than a limit-surface mesh.

Preselected input completes after the first option. Later option tokens in the
same macro become unknown commands. The priority capture therefore sets options
before command-first object selection. The older multi-option trials are excluded
from behavioral qualification. The documented `TargetObject` layer spelling is
also rejected by this captured version; its trials remain diagnostics.

The [whole-object qualification](../control-polygon-qualification.json) replays
all 42 trusted workflows through the command and interactive getters, including
the 32 successes and ten filtered inputs. Source geometry remains unchanged;
measured positions and channels agree within `1e-6`, and captured metadata,
selection and Undo/Redo agree exactly. The 19 malformed trials remain diagnostics.

Two additional captures retain fourteen component/range recipes and two box
face-click recipes. Five temporary ranges cover a line, a mixed polycurve,
a rational circle, its wrapped seam and a cubic spline. These outputs use
fresh attributes on the current layer, unlike whole-object extraction.
The [range/component qualification](../control-polygon-ranges-qualification.json)
compares exact control vertices, metadata, selection and Undo/Redo through typed
input and both interactive entry paths. Seven face-only preselection cases are
ignored; a mixed face/curve selection extracts only the curve and retains the
unrelated face until Undo/Redo. Both box face clicks reject the input.
Polycurve-segment preselection is ignored natively; its UI representation remains
unimplemented.

A separate [cancellation diagnostic](../../tools/rhino_oracle/observations/control_polygon_components_cancel_timeout.json)
records a 600-second owned-worker timeout. It provides no cancellation outcome
and is excluded from qualification. Input-layer ranges, repeated/mixed ranges,
texture display and comparative performance remain unqualified. See the
[command guide](../commands/control-polygons.md) for supported input.
