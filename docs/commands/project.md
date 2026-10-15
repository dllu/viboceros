# Project

Project curves and points in both directions along the finishing viewport's
construction-plane Z axis onto finite NURBS surfaces, B-rep faces or meshes. Face
boundaries and holes clip results with `Loose=No`. The default build supports planar curve
targets and the existing analytic point contact queries; `native-smlib` adds
curved curve targets and general point contacts.

See [mesh projection](project-mesh.md) for triangle conventions and qualification.

Start `Project`, select curves and points, press Enter, select targets, and press
Enter again. Curves/points can be preselected; mixed source and target
preselection completes immediately. Picking and typed IDs use the current
stage's filter. With Custom direction, preselection waits for two direction
points before projecting. Esc restores the selection from before the prompt.

```text
Project [Sources=id[,id...]] [Targets=id[,id...]]
        [Direction=CPlaneZ|View|Custom|x,y,z] [DeleteInput=Yes|No]
        [OutputLayer=Current|Input|TargetObject]
        [Loose=Yes|No]
        [SubCrv=id,start,end] [SubCrvLength=id,start,signed-length]
```

`Direction=View` uses the active camera at completion. `Direction=Custom` picks
a start and an end point through the shared drafting input: object snaps,
coordinate filters, distance/angle constraints, units, and world/relative
coordinates are available. Coincident points keep the end prompt active. Undo
during this getter revises the start without changing document history. The
accepted direction persists for the session; cancelling an incomplete
replacement preserves the last accepted direction.

An explicit vector also supplies a custom direction; its length and sign do
not affect the bidirectional projection. Outside a point prompt, Custom requires
`DirectionStart=x,y,z DirectionEnd=x,y,z` in world coordinates. Outside the app,
View requires `ViewDirection=x,y,z`.
`DeleteInput=No` and `OutputLayer=Current` are the initial defaults. Prompt
options persist during the application session. Output objects have default
attributes on the chosen layer and become selected. Inputs are deleted only
when the complete query succeeds and produces output. A miss creates no undo
entry; a failure preserves inputs, selection and redo history. Successful edits
are one Undo/Redo operation.

For example, select a point or line above a finite plane and its target, then
run `Project`. Use `Project Direction=1,0,2 OutputLayer=Input` for oblique
projection; use `Project Sources=1,2 Targets=3 DeleteInput=Yes` for explicit IDs.
IDs here are examples, not stable document identifiers.

During source selection, type `SubCrv`, choose a curve, and pick its start and
end. This uses the same temporary range picker as ApplyCrv/CreateUVCrv, with
typed points, a curve preview, numeric lengths and cursor direction locking.
Repeat it to collect multiple ranges, or combine ranges with whole curves and
points. Enter returns from an incomplete range to source selection; SelNone
clears the collected ranges. Source preselection skips this stage, so start
without curves or points selected when choosing a temporary range.

Headless `SubCrv=id,start,end` uses the source's native parameter domain, which
can differ from 0–1. `SubCrvLength=id,start,signed-length` uses a model-unit arc
length; its sign specifies the direction. Both arguments can repeat. For
example, a line from `(1,5,3)` to `(9,5,3)` has domain 0–8: a range from 2 to 6
projects the part from `(3,5,3)` to `(7,5,3)`. These arguments add ranges to the
whole inputs supplied by Sources or selection. To project only a temporary
range, leave its whole source unselected.

Temporary ranges create no model objects. DeleteInput=Yes deletes whole source
inputs only; a curve used solely through temporary ranges is preserved.
OutputLayer=Input uses the original range source's layer. Failed range resolution
preserves geometry, selection and redo history. Projection still stages all
outputs before a single history edit.

Planar projection is an exact affine transform followed by the shared curve/face
clipping query. Curved projection constructs the existing exact ruled surface
`C(u)+v*d` and intersects it with each original target using usd-brep. The native
adapter retains both UV witnesses, checks the complete output curve against
both source relations, clips original face trims, and certifies the retained
spans again. Finite sweep bounds require nonzero common-sign rational weights.
No additional projection solver or vendor-source patch is introduced.

InfinitePlane targets use the [IP picker and plane definitions](project-infinite-plane.md).

[Loose projection](projection-loose.md) moves edit points and reconstructs the
original NURBS basis. Its point constraints do not clip the curve between them.
SubD targets, grips and edge subobjects remain pending. Curved
source kinks and distinct target faces can produce separate objects. Coincident,
collapsed or singular source relations can be declined by existing
contact/preparation checks. Continuous
acceptance certifies admitted curves; it does not establish complete discovery,
branch topology, broad Rhino parity or comparative performance.

The initial local regressions (12 native and ten portable command tests, five app
tests, two native protocol tests and 112 Python worker tests) cover finite planes,
oblique and nearly parallel directions, bidirectional point projection, solid
multiple hits, holes, polylines, collapsed inputs, curved graph targets, partial
curved faces, output layers, cancellation and atomic history. The
Python fixture `tools/rhino_oracle/fixtures/project_curve_surface.json` supplies
six independent open-curve recipes and an arc-length-sampled comparison to
RhinoCommon's directional `Curve.ProjectToBrep`. A fresh isolated Rhino run
on 2026-10-14 timed out before the worker responded; these recipes have no
fresh Rhino observations yet. [Qualification record](../project-qualification.json)
retains the local response and source hashes.

The custom-direction increment passed 21 native and 19 portable command tests
and 13 Project app tests, including actual viewport clicks with endpoint snaps,
mixed preselection, typed coordinates, transient Undo/Cancel, saved direction,
and atomic validation of headless custom points. These are local checks;
interactive Rhino parity has not been measured. See the
[direction qualification record](../project-direction-qualification.json).

The subcurve increment passes 27 native and 24 portable command tests, 17
Project app tests in both builds, four shared range-command tests in both
builds, and six existing UV range-picker app tests. The shared range resolver,
trimming and projection producers are reused. Project-specific native getter
behavior, broad Rhino parity and comparative performance remain unqualified;
the current Wine dependency failure prevents fresh captures. See the
[subcurve qualification record](../project-subcurve-qualification.json).

```sh
cargo test -p viboceros-command --features native-smlib project::tests
cargo test -p viboceros app::tests::project
python3 -m tools.rhino_oracle viboceros \
  tools/rhino_oracle/fixtures/project_curve_surface.json --native-smlib
# Run Rhino in the private display and settings scheme:
tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/project_curve_surface.json --native-smlib \
  --scheme VibocerosOracleProject --absolute-epsilon 1e-6 --relative-epsilon 1e-10
```

Public references: [Rhino Project](https://docs.mcneel.com/rhino/8/help/en-us/commands/project.htm)
and [RhinoCommon ProjectToBrep](https://mcneel.github.io/rhinocommon-api-docs/api/RhinoCommon/html/Methods_T_Rhino_Geometry_Curve.htm).
