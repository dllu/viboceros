# Project

Project curves and points in both directions along the finishing viewport's
construction-plane Z axis onto finite NURBS surfaces or B-rep faces. Face
boundaries and holes clip the results. The default build supports planar curve
targets and the existing analytic point contact queries; `native-smlib` adds
curved curve targets and general point contacts.

Start `Project`, select curves and points, press Enter, select targets, and press
Enter again. Curves/points can be preselected; mixed source and target
preselection completes immediately. Picking and typed IDs use the current
stage's filter. Esc restores the selection from before the prompt.

```text
Project [Sources=id[,id...]] [Targets=id[,id...]]
        [Direction=CPlaneZ|View|x,y,z] [DeleteInput=Yes|No]
        [OutputLayer=Current|Input|TargetObject]
```

`Direction=View` uses the active camera at completion. An explicit vector
supplies a custom direction; its length and sign do not affect the bidirectional
projection. Outside the app, View requires `ViewDirection=x,y,z`.
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

Planar projection is an exact affine transform followed by the shared curve/face
clipping query. Curved projection constructs the existing exact ruled surface
`C(u)+v*d` and intersects it with each original target using usd-brep. The native
adapter retains both UV witnesses, checks the complete output curve against
both source relations, clips original face trims, and certifies the retained
spans again. Finite sweep bounds require nonzero common-sign rational weights.
No additional projection solver or vendor-source patch is introduced.

Mesh/SubD targets, grips, edge subobjects, Loose projection, and the interactive
two-point Custom option are pending. Curved source kinks and distinct target
faces can produce separate objects. Coincident, collapsed or singular source
relations can be declined by existing contact/preparation checks. Continuous
acceptance certifies admitted curves; it does not establish complete discovery,
branch topology, broad Rhino parity or comparative performance.

The local regressions (12 native and ten portable command tests, five app
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
