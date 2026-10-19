# SoftEditCrv

Select one curve and run `SoftEditCrv`. Pick a base point on the curve, then
an offset point. The base snaps to an edit station; the offset defines control
displacements with a Gaussian falloff measured along the current source curve.
Repeat base/offset pairs; Enter
finishes. Escape discards an unfinished offset and retains accepted edits.

`Distance` opens a numeric or curve-point getter. Numeric values support model
units and calculator expressions. A picked distance is the straight-line
distance from the snapped base to the projected curve point. `FixEnds=Yes`
pins nonperiodic endpoints. `Copy=Yes` creates a separate result for each
offset, retaining the current source for the next pick. Options update the
preview in every viewport; one Undo restores the complete command.

The command API accepts complete input, for example:

```text
SoftEditCrv 1.3,1.1,0 1.3,1.1,2 Distance=4 FixEnds=No Copy=No
```

Degree, knots and rational weights remain unchanged. Accepted results use
NURBS curves, including line, polyline, conic and composite sources. Replacements
retain source identity, attributes and groups and clear geometry user text.
Copies use current-layer defaults without groups or geometry user text.
Selection is released. Accepted edits replace redo history; previews and
unaccepted cancellation preserve it.

Falloff has standard deviation `Distance / 3`, with no hard cutoff. Shared seam
controls sum contributions from stored Greville sites inside the active chart,
then synchronize their aliases. Periodic endpoints remain coupled; FixEnds
does not pin periodic controls. Prepared falloffs are reused for destination
previews. The kernel retains its precise arc-length integration.

Validation replays 187 single-edit and 32 repeated/getter workflows from Rhino
8.32, including copies, cancellation, complete definitions, metadata and
Undo/Redo. Another 187 public SDK records compare default and `1e-12` fractional
length tolerances. Mathematical controls are checked at `1e-9`; command and
application results use `5e-8` because native default length integration can
differ from the precise result. No geometric coefficients are fitted to outputs.
Arbitrary trimmed-edge input, SubCrv selection, large control nets, extreme
coordinate ranges and cross-application performance remain unqualified.

```sh
cargo test -p viboceros-geometry --lib soft_edit
cargo test -p viboceros-command --test soft_edit_curve_command
cargo test -p viboceros --bin viboceros tests::soft_edit_curve
python3 -m unittest tools.rhino_oracle.test_soft_edit_curve
```

[Rhino command reference](https://docs.mcneel.com/rhino/8/help/en-us/commands/softeditcrv.htm)
· [Public scripting API](https://developer.rhino3d.com/api/rhinoscript/curve_methods/softeditcurve.htm)
· [Validation and captures](../soft-edit-curve-validation.json)
