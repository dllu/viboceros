# RemoveKnot

Start `RemoveKnot` with one curve or single-face surface selected, or pick the
source after starting it. Click knot markers or surface knot lines repeatedly.
`Undo` reverses the last successful removal while keeping the getter active.
Enter accepts the staged geometry as one document edit; Esc discards it. Removal
can change the shape. The viewport and picking curve show the current staged
result while the document and its existing Undo/Redo history remain unchanged.

Surface `Direction=U|V` names the displayed isocurves: U removes V knots and V
removes U knots. `Toggle` switches direction. These preferences belong to
RemoveKnot independently of InsertKnot and InsertKink. Both, Symmetrical,
Midpoints and Automatic are not removal options. Bare numbers are ignored by
the point getter, as in Rhino. The separate legacy numeric API remains available.

Curve picks choose the nearest knot marker in model space; equal-distance
markers choose the later station. Endpoints cannot be removed. Surface picks
choose a removable interior knot line in the active direction in the active
view. Projection preserves perspective and the integer device-coordinate pick
location; the closest line can differ between Perspective and Top views.
Trimmed faces expose the underlying knot lines across trim holes too. A pick
without a removable knot creates no local Undo checkpoint. Polycurves retain NURBS leaves; removing a connecting
joint merges its adjacent leaves into one NURBS segment. Rhino retains the
left leaf's outer interval and the merged NURBS inner chart; subsequent markers
are clipped to the retained outer interval. Local Undo restores both charts.
Periodic directions retain their cyclic basis and repeated controls rather than being clamped.

Trimmed faces retain the UV region, including holes. The existing surface-edit
module reconstructs exact rectangular boundaries or fits other boundary images
and requires continuous parameterized bounds before accepting them. It does not
enlarge the document tolerance to admit an inconsistent B-rep. This can be slower
than editing an untrimmed surface. Internal vertex/edge table order can change.

`SubCrv` at the source prompt temporarily shortens a curve. It shares endpoint,
length, unit, direction-lock and return-to-parent input with the insertion
commands. Acceptance creates a new object on the current layer with default
attributes; the original remains. Polysurfaces and block members are excluded.

A completed or canceled command clears the source selection. Acceptance retains
object identity, attributes and groups. Curve and untrimmed surface acceptance
clears geometry user text even if every removal was undone locally or no point
was picked; trimmed B-reps retain that text. Cancellation preserves geometry text.

For scripting, `Points` distinguishes the repeated getter from the legacy
parameter form:

```text
RemoveKnot Points w2,1,0 w4,0,0 Undo
RemoveKnot Points Direction=U w3,4,0 Toggle w5,2,0
RemoveKnot Points SubCrv=object-id,start,end w2,1,0
```

`RemoveKnot parameter|u,v Direction=U|V` retains its existing parameter-axis
meaning and its existing periodic rejection policy. See [NURBS scripting](nurbs.md).
Headless calls without a view use model-space distances. Supply
`picking::Projection` to `Draft::point_in_view` or
`CommandRegistry::execute_with_pick_projection` to reproduce a viewport getter.
Opening a 3dm retains its viewport pixel context until the first layout. The
interactive path uses the selected backend's homogeneous fitting operations;
`native-smlib` uses usd-brep for open fits and cyclic collocation. Cyclic removal
reuses the geometry module's periodic basis and Greville interpolation machinery.

The [validation receipt](../remove-knot-validation.json) records the native
command matrix, retained source 3dm files, comparisons and limitations. The
157-command matrix includes genuinely deformed controls, repeated picks,
local/document Undo, cancellation, analytic and periodic curves, trimmed faces and holes,
temporary portions, and periodic surface directions. Sharp deformations of the
linear extrusion direction become polysurfaces in Rhino; those two failure
rows are retained, alongside smooth periodic tensor deformations that succeed.
It does not establish full Rhino coverage or a general performance claim. Numeric temporary shortening
inherits shared input tests and has no new RemoveKnot-specific native capture.
Native metadata-only Undo/Redo records are not captured. Rust command results do
not expose Rhino's complete Cancel/Failure enum. Boundary curves are checked
against their surface image at document tolerance, while native snapshots retain
edge endpoints rather than full boundary definitions. Boundary tables are
compared modulo permutation and edge orientation. The two unused outer knot
slots omitted by RhinoCommon are normalized; every active knot, control, weight,
domain and sample remains checked at the recorded epsilon. Display guides and
closest-point searches have the existing bounded numerical policies. Projected
guide picking currently requires its control hull in front of the camera.
Repeated composite-joint captures have two leaves; larger composite workflows,
high-DPI pixel parity and general performance remain unqualified.

```sh
cargo test -p viboceros-command --test remove_knot_command
cargo test -p viboceros-command --features native-smlib --test remove_knot_command
cargo test -p viboceros app::tests::remove_knot
python3 -m unittest tools.rhino_oracle.test_remove_knot_command
tools/rhino_oracle/run_headless.sh exec python3 -m tools.rhino_oracle.generate_remove_knot_command
tools/rhino_oracle/run_headless.sh exec python3 -m tools.rhino_oracle.generate_remove_knot_followup
tools/rhino_oracle/run_headless.sh exec python3 -m tools.rhino_oracle.generate_remove_knot_smooth_periodic
```

Public references: [RemoveKnot](https://docs.mcneel.com/rhino/8/help/en-us/commands/removeknot.htm),
[BrepFace.RebuildEdges](https://developer.rhino3d.com/api/rhinocommon/rhino.geometry.brepface/rebuildedges).
