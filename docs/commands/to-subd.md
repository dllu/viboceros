# ToSubD

`ToSubD` converts meshes, NURBS surfaces, individual B-rep faces and typed
extrusions to SubDs. Select objects, run `ToSubD`, edit options, then press Enter.
Starting without a selection adds a selection stage first. Ctrl+Shift picks
B-rep faces before the command or during selection; mesh face picks are excluded.

| Option | Default | Meaning |
| --- | --- | --- |
| `UseMesh` | `ControlPoints` | Use mesh vertices directly, or interpolate their limit locations with `Location`. |
| `MeshCreases` | `No` | Preserve mesh crease features. |
| `MeshCorners` | `No` | Preserve mesh boundary corners. |
| `UseSurface` | `Location` | Fit the surface's sampled locations, or use its NURBS control net with `ControlPoints`. |
| `SurfaceCorners` | `No` | Preserve eligible open surface corners. |
| `DeleteInput` | `No` | Replace inputs instead of keeping them and adding copies. |

Options can be typed as `Name=Value` or chosen in the prompt. Accepted option
edits persist for the current application session, including after cancellation;
Undo changes the model without resetting these preferences.

Scripts act on the selected objects:

```text
ToSubD UseMesh=Location MeshCorners=Yes DeleteInput=No
ToSubD UseSurface=ControlPoints SurfaceCorners=Yes DeleteInput=Yes
ToSubD Faces=0,2
ToSubD Faces=All Object=selected-object-uuid
```

`Faces=` addresses zero-based B-rep face indices; `Object=` restricts that request
to one selected parent. Whole multi-face ordinary B-reps require face selection.
Whole typed extrusions, including capped ones, are eligible directly. Imported
3DM extrusions retain that representation. Viboceros's Box, Cylinder and planar
ExtrudeCrv creators still need the corresponding representation update; their
current ordinary B-rep outputs require selecting faces.

Copies preserve source layer, color and name, receive new identities and leave
groups. Replacements retain source identity and groups. Multiple selected faces
produce separate copies in reverse component order. With `DeleteInput=Yes`,
conversion visits faces forward and the last face replaces the parent, matching
the recorded Rhino behavior. The completion count includes all converted faces.

Surface conversion uses the underlying surface, including regions outside
trims. Some collapsed control nets produce no output. Cancellation, invalid
input or stale geometry leaves the model unchanged. Successful conversion is
one undo step; source selection is released during undo/redo.

[Kernel and extrusion evidence](../tosubd-kernel.md) describes the geometry
comparisons. The [document workflow record](../tosubd-command-qualification.json)
covers fixed command and application prompt replays, attributes, identity,
selection and history. It does not establish arbitrary topology, continuous
surface error, physical native pointer gestures or comparative performance.
