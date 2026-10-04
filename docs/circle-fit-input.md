# Circle FitPoints input

[Command reference](commands/README.md) · [Fitting kernel](circle-fit.md) · [Plane frames](plane-fit-basis.md)

`Circle FitPoints` fits an analytic circle to selected point objects and
[displayed control points or mesh vertices](control-points.md). Start
it directly, or enter `FitPoints` at Circle's center prompt. Three or more
preselected points complete immediately, counting object and grip picks
together. With fewer than three, the command clears prior selection and opens
a point picker. Click or window-select points, then press Enter. `SelAll` and
`SelNone` include displayed grips and point objects. Escape cancels without
editing geometry or consuming Redo. Enable source grips with `PointsOn` first.

The command fits full spatial center distances on a least-squares plane, gives
each selected point or grip equal weight, preserves source geometry/attributes,
and adds one unselected Circle on the current layer. It does not use the construction
plane to reorient the fit. Equal coordinates in separate point objects retain
their separate weights.

Completed fits retain preselected points/grips and irrelevant preselected
object peers; Undo/Redo preserve that accepted object selection. Points picked
during the command are deselected when its getter finishes. Control-point
displays stay enabled through Undo/Redo unless turned off with `PointsOff`.
Collinear/coincident selections finish without an output, model edit or new
Undo entry. Fewer than
three accepted points retain the picker for further selection.

## Native evidence

Eight owned Rhino **8.32.26160.13001** workflows were captured using private
Xvfb and a separate settings scheme. They cover immediate preselection,
insufficient preselection, cancellation, an irrelevant selected line, later
point selection and a collinear no-output fit. The
[provenance record](circle-fit-input-provenance.json) hashes the helper,
recipes and unmodified observations. Both the EndCommand snapshot and final
script state are retained: postselection cleanup happens between them. Completed
fits receive no trailing Escape tokens, keeping the subsequent history probe
independent of idle selection/display cleanup.

Application tests replay all eight workflows through both the direct command
and the Circle center option, including selection, output roles and Undo/Redo.
Command tests additionally replay all 22 earlier Circle fitting captures,
checking native center/radius bounds and independent preselection lifecycle.
The geometry kernel tests compare
64 native boundary witnesses for accepted regular and conditioning cases.
Sixteen further [grip workflows](control-points.md) verify mixed inputs,
control-point display and replay lifecycle.

## Compatibility work

This implements point-object, curve/surface control-point and mesh-vertex input
for Rhino's
[Circle FitPoints option](https://docs.mcneel.com/rhino/8/help/en-us/commands/circle.htm).
Complete grip editing, overlap menus and periodic or
composite grip combinations need more work. Native oriented normals and seams are unresolved,
so the generated Circle's parameterization can differ despite a matching locus.
Maelstrom's signed FitPoints deformation still needs that oriented frame work.

The retained `1e-6` near-collinear input differs from native by about `2.35e-6`
in both center and radius. It remains an explicit compatibility gap. Arbitrary
thin/noisy inputs, tied plane singular values, extreme ranges, original native
performance and deformable output also need verification or implementation.
The current witnesses do not establish full Rhino parity.

```sh
cargo test --release -p viboceros-command circle_fit_points
cargo test --release -p viboceros app::tests::circle_fit_points
python3 -m unittest tools.rhino_oracle.test_circle_fit_selection
tools/rhino_oracle/run_headless.sh exec python3 -m tools.rhino_oracle rhino \
  tools/rhino_oracle/fixtures/circle_fit_selection.json \
  --scheme VibocerosOracleCircleFitSelection \
  --output /tmp/circle-fit-selection.json
```
