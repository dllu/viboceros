# Adjoining line segments

`Lines` creates a separate line object for each adjoining segment. Enter the
command, pick or type successive endpoints, then press Enter. For a complete
typed command, use `Lines 0,0,0 4,0,0 4,3,1`; add `Close` to join the last
endpoint to the first.

During point entry, `Undo` removes the last draft point, including the initial
anchor. `Close` becomes available after three points. Returning exactly to the
first point completes the command automatically after at least three segments;
a two-segment double-back waits for Enter. Enter with fewer than two points or
Escape cancels the draft. Starting another modeling command also discards it.
Completed segments share one model Undo/Redo entry and are unselected objects
on the current layer. `SelLast` recalls the entire completed set.

Unfinished segments stay in the viewport preview. They do not change document
geometry, selection, history or pending Redo. Typed and picked points share the
existing coordinate, snapping and distance/angle-constraint route. Transparent
CPlane editing preserves the draft; local coordinates entered afterward use
the new construction plane. `ReadCommandFile` feeds the same getter and retains
its whole-file Undo grouping. Enter canceling an empty or one-point draft also
stops the rest of that file.

The getter rejects endpoint distances below the public OpenNURBS
`ON_SQRT_EPSILON` value, `1.490116119385e-8`; equality is accepted. This admission
threshold is independent of document tolerance. Each line keeps its own
physical-length parameter domain. At most 100,000 segments are supported.

The [qualification record](../lines-validation.json) retains 50 native
observations: 47 complete input sequences and three early-completion driver
diagnostics. Corrected fresh captures cover those three workflows. Of the
complete sequences, 45 have valid native geometry; two tiny segments near
coordinates of 10¹² are created by Rhino but marked invalid by its SDK. Their
coefficients and history remain compared as unqualified diagnostics, and their
3DM round trips are excluded. Portable and native builds each check 33 valid
getter/file constructions and nine complete-command constructions through 3DM.

Selection-recall comparisons explicitly use
`DeselectOthersBeforeSelect=No` for direct Lines and `Yes` for command files to
match the captured Rhino policies; default preference parity is unqualified.
Comparisons cover control points, weights,
knots and 33 finite samples evaluated from the captured definitions. Continuous
distance, comparative speed and arbitrary mouse trajectories remain unqualified.
Run the windowless checks with `python3 -m tools.rhino_oracle.check_lines`.
