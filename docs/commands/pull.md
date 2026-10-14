# Pull

`Pull` creates curves and points at the target's closest surface locations.
Curved surface paths use the [usd-brep normal-drop producer](../smlib-surface-pull.md)
when `native-smlib` is enabled. Planar curves use exact normal projection;
points use the existing bounded surface/face and indexed mesh closest queries.
All results stage before one Undo/Redo edit.

Select curves and points before starting, or start Pull and pick sources, then
press Enter to select targets. Mixed source/target preselection executes
immediately. Enter at target selection creates and selects the results.
`DeleteInput=Yes|No` and `OutputLayer=Current|Input|TargetObject` are remembered
independently of Project for the application session. Outputs use fresh
attributes on the chosen layer. A complete miss preserves the original inputs.

Headless examples:

```text
Pull Sources=source-id Targets=surface-id DeleteInput=Yes
Pull Sources=point-id Targets=mesh-id OutputLayer=TargetObject
Pull Targets=surface-id SubCrv=curve-id,2,6 DeleteInput=Yes
Pull Sources=source-id InfinitePlane=WorldYZ
```

DeleteInput removes whole sources. An original referenced only by temporary
SubCrv or SubCrvLength ranges remains in the model. In the picker, type `SubCrv`, choose a curve and pick the range
with the shared drafting input. `IP` opens the [shared plane getter](project-infinite-plane.md):
World/CPlane axes, planar object/face references, drawn planes, AroundCurve and
typed NamedCPlane definitions work without creating a model surface. Each
infinite plane uses its own normal. Esc restores the initial selection; Undo
while picking a plane or subcurve revises that transient input.

Curved B-rep results retain paired native spatial/UV witnesses. The existing
trim-region clipper restricts them to original face loops, including holes,
and recertifies retained spans on the original surface at model tolerance.
Duplicate seam witnesses are removed only after the shared curve intersector
reports complete overlap on both spatial curves. Distinct spans remain separate
outputs. Points pulled to bounded surfaces or faces may land on their outer or
hole boundaries. Mesh nearest queries reuse the projection BVH, existing
triangle closest-point solver and exact distance ordering, visiting nearer boxes
first and retaining deterministic ties.

`Loose=No` is accepted. Loose=Yes, curves pulled to meshes, grips, edit points,
edge subobjects and SubD targets remain pending. Curved paths inherit native
preparation and certificate limits. Global closest-branch choice, native trim
and seam parity, native getter behavior and comparative speed remain unqualified;
this is not a claim of complete Rhino compatibility. The [existing Wine issue](../oracle-sspi.md)
still prevents fresh live Rhino captures.

The Python `pull_curve_surface_command` operation accepts full `curve` and
`surface` definitions, runs the Viboceros command and returns sampled curve
geometry. Its Rhino counterpart uses public
[Curve.PullToBrepFace](https://developer.rhino3d.com/api/rhinocommon/rhino.geometry.curve/pulltobrepface),
so it compares geometry without qualifying the native command's input or
metadata behavior. The earlier `pull_curve_surface` operation retains raw paired
kernel output; that historical record intentionally keeps both cylinder-seam
witnesses. The command operation tests their spatial admission separately.

```sh
cargo test -p viboceros-command --features native-smlib pull::tests
cargo test -p viboceros app::tests::project::pull
python3 -m tools.rhino_oracle viboceros \
  tools/rhino_oracle/fixtures/pull_curve_surface_command.json --native-smlib
```

See [qualification](../pull-command-qualification.json) and the
[Rhino command reference](https://docs.mcneel.com/rhino/8/help/en-us/commands/pull.htm).
