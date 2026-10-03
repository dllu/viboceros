# Mirror planes

[Transforms](transforms.md) · [Copy and history](transform-copy.md)

With objects selected, start Mirror and pick two points for a plane perpendicular
to the current CPlane. At the first point prompt, choose one of these options:

| Option | Mirror plane |
| --- | --- |
| 3Point | Pick an origin, a second point, and a noncollinear third point to define a spatial plane. |
| XAxis | The active CPlane's XZ plane, through its origin. |
| YAxis | The active CPlane's YZ plane, through its origin. |
| ZAxis | The active CPlane itself, through its origin. |

Axis shortcuts execute immediately. These choices follow
[Rhino's documented Mirror options](https://docs.mcneel.com/rhino/8/help/en-us/commands/mirror.htm).
They can also be supplied when starting the command, for example
`Mirror 3Point Copy=No`, then pick the plane points. Edit Copy at any of its point
prompts. Plane choices are available before the first two-point start is picked;
later choices are rejected while the current point prompt remains active.

Full registry invocations use world point arguments:

```text
Mirror 3Point 1,2,3 3,4,5 4,0,7 Copy=Yes
Mirror XAxis Copy=No
```

A repeated second point is rejected. A duplicate or collinear third point ends
the command without an edit, matching native plane-construction failure. Enter or Escape
at earlier prompts also leaves geometry unchanged. A completed Mirror finishes
after one edit; originals remain selected and new copies are unselected.
[Attributes, group definitions, object order, SelLast, and Undo/Redo](transform-copy.md)
follow the shared affine-command policy.

## Verification and limits

The [64 prescribed cases](../../tools/rhino_oracle/fixtures/mirror_planes.json)
compare three-point planes and all three axis shortcuts on WorldXY, translated
tilted, and translated Right CPlanes. Four selected, affinely independent point
witnesses distinguish the entire spatial affine map; an unselected fifth point
checks replacement order. Grouped copies, a single selected group member, Copy
edits, cancellation, degenerate points, and late option attempts are included.
The [raw Rhino 8.32 observations](../../tools/rhino_oracle/observations/mirror_planes.json)
retain histories, terminal Success/Cancel/Failure events, geometry, attributes,
selection, source identity, group topology, SelLast, and external Undo/Redo.
App prompts and complete registry invocations match document snapshots at
absolute epsilon `1e-9`, relative zero; automatic group names are raw telemetry.
The [five early-Enter recipes](../../tools/rhino_oracle/fixtures/mirror_enter.json)
and their [raw capture](../../tools/rhino_oracle/observations/mirror_enter.json)
verify command termination at each unfinished two-point or three-point phase.
These macros end at Enter, with no trailing Cancel token.

These witnesses verify affine maps and document behavior, without exhaustively
comparing every curve, surface, or mesh representation. The Object option,
history-linked planar targets, subcurve input, command-first source selection,
and native mirrored-object previews remain incomplete. Mouse-based plane picks
and near-collinear acceptance thresholds are not measured by this capture.

```sh
tools/rhino_oracle/run_headless.sh exec python3 -m tools.rhino_oracle.transform_copy_capture tools/rhino_oracle/fixtures/mirror_planes.json --scheme VibocerosOracleMirrorPlanesFresh --timeout 300
tools/rhino_oracle/run_headless.sh exec python3 -m tools.rhino_oracle.transform_copy_capture tools/rhino_oracle/fixtures/mirror_enter.json --scheme VibocerosOracleMirrorEnterFresh --timeout 300
cargo test -p viboceros mirror_plane_options --bin viboceros
python3 -m unittest tools.rhino_oracle.test_transform_copy
```

Use a fresh private scheme and the dedicated Xvfb wrapper for capture.
