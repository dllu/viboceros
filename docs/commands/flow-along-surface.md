# FlowAlongSrf

Select objects or grips, enter `FlowAlongSrf`, and pick the base and target surface near corresponding corners and edges. Inspect the shared viewport preview, then press Enter to apply one undoable edit. Escape cancels the preview. `FlowAlongSurface` is an alias.

`Copy`, `Rigid`, `RigidGroups`, `AutoAdjust`, `ConstrainNormal`, and `PreserveStructure` accept Yes/No. Entering an option name toggles it. Copy defaults to Yes and follows `RememberCopyOptions`. Rigid placement uses the bounding-box center by default. With Rigid=Yes and RigidGroups=Yes, `GroupFrameOrigin=ObjectFrameAverage` uses the unweighted mean of selected members’ individual tight bounding-box centers. `GroupFrameOrigin=BoundingBoxCenter` uses their combined box center. Entering `GroupFrameOrigin` toggles these choices. Selected grips deform their owner even when Rigid is Yes. PreserveStructure moves NURBS controls; otherwise curves and surfaces use the shared tolerance-controlled refitting path.

With AutoAdjust=Yes, the picked corner and incident edge determine the parameter axes and normal. Trimmed faces use their shrunk underlying surface. AutoAdjust=No uses the original underlying UV domains and ignores pick orientation. B-rep face orientation does not change the right-hand UV normal.

`Plane` draws a temporary rectangle using two corners in the active CPlane; `3Point` draws its X edge and width point. No plane object is added. ConstrainNormal=Yes requests two direction points; Enter uses the Z axis of the viewport where the target was selected. `Base` or `Target` restarts that reference getter. Hidden, locked, deleted, replaced, or tolerance-invalidated references require another pick.

Scripted input uses explicit references:

```text
FlowAlongSrf Base=12 Target=13 Copy=No AutoAdjust=No
FlowAlongSrf BaseFace=12,0 TargetFace=13,2 BaseCorner=1 BaseSwap=Yes
FlowAlongSrf BasePlane=0,0,0;10,0,0;0,10,0 Target=13 ConstrainNormal=Yes Normal=0,0,1
```

Corners 0–3 denote the native UV corners (min/min, max/min, max/max, min/max); Swap exchanges U and V after choosing the corner. Explicit Corner/Swap options override a prior mouse pick. Drawn planes use length-based parameter domains. Outside the bounded base surface, the point map continues using the native parameter derivatives at the closest boundary point.

The independent implementation is checked against public Rhino 8 SDK calls and 174 completed native command workflows, including real mouse picks, trimmed/reversed faces, copy, groups, rigid placement, control/edit/mesh grips, and Undo/Redo. Point-map and preserved-structure comparisons use `1e-6`. Four nonlinear refit witnesses have different control nets: sampled same-parameter differences reach `1.01e-5` for curves and `4.23e-6` for surfaces. The captures retain both complete definitions; refit comparisons use `1.1e-5` and `5e-6`, respectively.

Current scope is regular NURBS reference surfaces and explicit B-rep faces. Singular reference derivatives can fail before editing the document. Plane supports corner and three-point input; other Rectangle modes are not implemented. Live dependency history and Rhino performance parity are not claimed. See the [mapping validation](../surface-flow-validation.json) and [group origin validation](../surface-flow-group-origins-validation.json).

Public references: [FlowAlongSrf](https://docs.mcneel.com/rhino/8/help/en-us/commands/flowalongsrf.htm), [SporphSpaceMorph](https://developer.rhino3d.com/api/RhinoCommon/html/T_Rhino_Geometry_Morphs_SporphSpaceMorph.htm).
