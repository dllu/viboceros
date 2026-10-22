# Control-polygon extraction

[Editing commands](editing.md) · [Native comparisons](../oracle/control-polygon.md)

`ExtractControlPolygon` creates polylines through curve controls and polygon
meshes through surface or SubD controls. Select objects first or start the
command, pick objects, and press Enter. Single-face B-reps, including trimmed
surfaces, use their underlying surface net. Whole polysurfaces, points and
meshes are filtered out; unsupported preselected peers stay selected when an
eligible source succeeds.

```text
ExtractControlPolygon OutputLayer=Input
ExtractControlPolygon SubDPriority=TextureCoordinates
```

`OutputLayer=Current|Input` chooses the result layer. Names, object colors and
existing group memberships come from each source. Inputs stay unchanged;
outputs are selected. Undo and Redo release the command's selections.

`SubDPriority=Geometry` preserves SubD control topology and crease seams.
`TextureCoordinates` transfers packed texture corners, with separate vertices
at seams. Surface meshes carry Greville texture coordinates and evaluated
normals. Choices can be changed while picking and reset for the next invocation.
Escape cancels picking without model edits or clearing Redo.

The documented `TargetObject` layer value remains accepted as an input-layer
alias for compatibility. Rhino 8.32 rejected that spelling in the retained
capture; those trials are excluded from native qualification. Complete typed
invocations accept both options together; Rhino's preselected macro getter can
finish after its first option.

[Qualification](../control-polygon-qualification.json) covers 42 whole-object
workflows through the command and interactive getters. SubCrv input, face
component picking, texture display and comparative performance remain separate
work. The [public Rhino help](https://docs.mcneel.com/rhino/8/help/en-us/commands/extractcontrolpolygon.htm)
describes the broader native command.
