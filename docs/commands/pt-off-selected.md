# PtOffSelected

Select objects with control or edit points displayed, then run `PtOffSelected`.
Only those owners' point displays close. `PointsOff` closes every display.
The command also works first: pick eligible owners, then press Enter. Selected
control or edit grips alone do not select an owner for this command.

Preselected owners remain selected. Owners picked during the command are
released on completion or Cancel; unrelated object and grip selections remain.
Grouped owners are handled individually. An owner with its points already off
cannot satisfy the picker, including when its geometry supports control points.
Unrelated displays retain their mode, point positions, hidden flags and picks.

Geometry, attributes and groups remain unchanged. As in Rhino, starting
`PtOffSelected` adds one unchanged-command Undo step and discards Redo, even if
the picker is canceled. Undo and Redo of that step leave point display unchanged.
`PointsOff` preserves Redo. Invalid arguments are rejected before any
display or history change.

## Validation

The qualified corpus contains 41 actual Rhino 8 command invocations: lines,
cubic/rational curves, a surface face, a quad mesh and a closed SubD, in each
supported control/edit mode. It covers owner/grip/peer selections, two owners,
groups, hidden peers, command-first acceptance and Cancel. Both registry and UI
replays compare every before/after grip position within `1e-7`, with exact
indices, counts, visibility, selection, getter results and Redo witnesses.

Thirty-six additional history controls compare no command, `PointsOff` and
`PtOffSelected`. SDK Undo succeeds only after `PtOffSelected`, reports its label,
and preserves the complete post-command grip state. Older anchor-object history
was unavailable in the controls as well as the command cases; those anchors do
not establish preservation of earlier native Undo entries. The first 27-case
capture is retained as a diagnostic:
its trailing idle Cancel changed selection, and its Redo setup was not qualified.
Successful observations and their content-addressed producer archives are immutable.

```sh
cargo test -p viboceros-oracle --test pt_off_selected
cargo test -p viboceros --bin viboceros pt_off_selected
python3 -m unittest tools.rhino_oracle.test_pt_off_selected
```

General overlap menus, arbitrary trimmed solids, hidden/locked owners and
large-model picking performance still need native qualification.

[McNeel reference](https://docs.mcneel.com/rhino/8/help/en-us/commands/pointson.htm)
