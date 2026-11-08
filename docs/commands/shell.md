# Shell

`Shell` removes chosen faces from a closed manifold polysurface and hollows the
remaining faces inward. Positive thickness requires `--features native-smlib`;
zero thickness also works in the portable build.

Start `Shell`, pick opening faces, and press Enter. At least one face per source
must remain. `Thickness` opens a numeric getter; numbers and unit suffixes such
as `2mm` are accepted. Enter keeps its current value, and Esc returns to face
selection. Esc at face selection cancels. Enter without faces also cancels.
Thickness and `DeleteInput` edits are remembered for the session, including
canceled commands. Invalid edits preserve the previous options.

Scripts can select source objects and use `Face=0` or `Faces=0,2`.
`FaceRefs=id:index,...` addresses original faces across multiple sources:

```text
Shell Thickness=1cm DeleteInput=No FaceRefs=1:0,1:2
```

Thickness is finite and nonnegative. Initial application defaults are one model
unit and `DeleteInput=Yes`. Yes replaces the source geometry in place for the
first material region; No retains it and adds a copy. Outputs inherit source
attributes and groups and are unselected. Disconnected material regions become
separate objects; enclosed cavity boundaries stay with their surrounding
material. The whole batch is one Undo/Redo step. Geometry failures preserve the
document and history, and the UI retains the picks for correction.

Zero thickness produces coincident, oppositely oriented retained skins sharing
the opening rim. This is closed topology with zero material volume. Positive
shells use localized native offsets and Boolean wall construction; roundoff UV
joint adjustments require continuous edge-deviation certificates.

[Current validation](../shell-native-validation.json) accounts for 40 licensed
Rhino observations, 24 complete command workflows, native SDK box and cylinder
geometry, actual history events, and 3DM round trips. Cylinder partitions differ
between the kernels; their boundary geometry and mass are compared. These finite
checks do not establish full Rhino parity or comparative performance. General
self-intersections, collapsed positive walls, History `EditThickness`, live
preview, factory preferences, and every selection workflow need further work.
[Earlier sanitizer work](../shell-qualification.json) is retained separately.

```sh
python3 -m tools.rhino_oracle.check_shell_native
# Fresh licensed capture, confined to a private display:
tools/rhino_oracle/run_headless.sh exec python3 -m tools.rhino_oracle.generate_shell_sdk_geometry
```

Reference: [Rhino 8 Shell](https://docs.mcneel.com/rhino/8/help/en-us/commands/offsetsrf.htm#Shell)
and [Brep.CreateShell](https://developer.rhino3d.com/api/rhinocommon/rhino.geometry.brep/createshell).
