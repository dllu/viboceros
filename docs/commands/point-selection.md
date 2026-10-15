# SelPt and InvertPt

`SelPt` adds selectable point objects and every displayed control or curve edit
grip to the selection. Existing object picks remain selected. Grip owners and
group peers are selected only if they were already selected; point cloud samples
remain part of their point-cloud object.

`InvertPt` exchanges selected and unselected visible grips. It preserves ordinary
object picks and the geometry. Running it twice restores the original grip picks.
Hidden, locked, disabled or stale grip displays are excluded. Coincident mesh
grips retain their separate `(object ID, index)` identities.

Both commands preserve Undo and Redo. They accept no arguments, support the usual
case-insensitive command names and underscore/hyphen prefixes, and retain the
enabled grip display. `SelNone` clears selections while retaining that display;
`PointsOff` also hides the grips.

You can use both commands during the source-selection phase of Move, Copy,
affine grip transforms, SetPt, Smooth and Circle FitPoints. The active source
getter remains open. SelPt respects its object filter: for example, Smooth
accepts displayed grips while excluding ordinary point objects. Circle FitPoints
can select both. A malformed selection command leaves the prompt and selection
unchanged. Enter then accepts the selected sources and continues the operation.

For example, create and select a curve, run `EditPtOn`, then enter:

```text
SetPt XSet=No YSet=No ZSet=Yes
SelPt
<Enter>
0,0,5
```

The command selects the displayed edit stations through the SetPt source getter
and aligns them to world Z=5. To edit the complement of a mouse-picked set, run
`InvertPt` before starting the transform.

McNeel's [command reference](https://docs.mcneel.com/rhino/8/help/en-us/commandlist/command_list.htm)
includes grips in SelPt, while the detailed
[selection page](https://docs.mcneel.com/rhino/8/help/en-us/commands/selection_commands.htm)
describes SelPt as point-object selection and describes InvertPt's visible-grip
behavior. This implementation follows the broader SelPt reference. Independent
regressions cover both grip modes, grouped point objects, hidden/locked displays,
coincident mesh controls, history preservation, filtered getters, Circle fitting
and keyboard SetPt input. Fresh native selection/getter parity and performance
remain unqualified; the recorded [Wine SSPI issue](../oracle-sspi.md) deferred
live Rhino retesting. Solid grips, SubD grips, per-grip hiding and the remaining
point-selection commands need further implementation.

The [six pending Rhino recipes](../../tools/rhino_oracle/fixtures/point_selection.json)
use the public command/SDK probe and have no captured native results. The
[qualification record](../point-selection-qualification.json) retains source
hashes, test results and these limits.

```sh
cargo test -p viboceros-command point_selection
cargo test -p viboceros point_selection
```
