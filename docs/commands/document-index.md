# Documents, selection and viewports

[All commands](README.md) · [Command input](../command-line.md)

- [Block definition management](block-manager.md)
- [CreateUniqueBlock and definition duplication](create-unique-block.md)
- [ReplaceBlock](replace-block.md)
- [AddObjectsToBlock](add-objects-to-block.md)
- [BlockResetScale](block-reset-scale.md)
- [In-place BlockEdit](block-edit.md)
- [Detach or copy group members](remove-from-group.md)
- [Create shared blocks and insert instances](blocks.md)
- [Explode instances one level or recursively](explode-blocks.md)
- [Selection, attributes, layers, and groups](document.md)
- [Select extrusions, block instances and mirrored placements](typed-selection.md)
- [Volume selection](volume-selection.md)
- [Model units](units.md)
- [Model tolerances](../tolerances.md)
- [Native planar STEP export](export-step.md)
- [Display modes and drafting controls](interface.md)
- [Zoom, extents, and border settings](zoom.md)
- [World and construction-plane views](set-view.md)
- [Named views](named-view.md)
- [Construction-plane editing](../cplane.md)

## Examples

These are independent snippets. Prepare the required sources and selection first;
replace object IDs, names and paths with values from your document. See each
command page for supported options and limitations.

```text
Layer New Construction
Layer Hide Construction
Layer Show Construction
Layer Current Default
ChangeLayer Construction
CopyToLayer Default
SelAll
ZoomEnds All
ShowEnds
ZoomEnds Current
ZoomEnds Next
ZoomEnds Previous
ZoomEnds Mark
ShowEndsOff
SnapSize 0.25 ApplyTo=AllViewports
Grid MinorLineSpacing=0.5 MajorLineInterval=5 GridLineCount=50 ShowGrid=Yes ApplyTo=AllViewports
SelWindow
SelCrossing
SelRectangular SelectionMode=InvertCrossing
SelCircular SelectionMode=Crossing
SelBoundary SelectionMode=Crossing
SelBox 0,0,0 5,4,0 3 SelectionMode=Crossing
SelVolumePipe 0.5 SelectionMode=Crossing
SelVolumeObject SelectionMode=Crossing
SelVolumeSphere 0,0,0 5 SelectionMode=Crossing
SelFence
SelFence Curve
SelLast
SelPrev
SelLast DeselectOthersBeforeSelect=No
SelCrv
SelSelfIntersectingCrv
SelOpenCrv
SelClosedCrv
SelPlanarCrv
SelLine
SelPolyline
SelShortCrv 1.0
SelSmall 5.0
SelPt
SelPtCloud
SelSrf
SelOpenSrf
SelClosedSrf
SelPlanarSrf
SelTrimmedSrf
SelUntrimmedSrf
SelPolysrf
SelOpenPolysrf
SelClosedPolysrf
SelMesh
SelOpenMesh
SelClosedMesh
SelSubD
SelOpenSubD
SelClosedSubD
SelNonManifold
SelColor 12,34,56
SelName "Fastener *"
SelID 550e8400-e29b-41d4-a716-446655440000
SelLayer "Construction *"
SelLayerNumber 1
SelGroup Assembly
SelKey "Part Number"
SelValue "A*"
SelKeyValue "Part Number" "A*"
SelDup
SelDupAll
Invert
Group Assembly
Group All Everything
AddToGroup Assembly
RemoveFromGroup Copy=No
SetObjectName "Fastener Part" AppendCounter=Yes
SetUserText "Part Number" "A 12"
SetUserText "Part Number" "On geometry" AttachTo=Object
SetUserText "Part Number" ""
GetUserText "Part Number"
GetUserText "Part Number" AttachTo=Object
SetObjectColor 12,34,56
SetObjectColor ByLayer
Ungroup
UngroupAll
Ungroup Assembly
Hide
Show
Lock
Unlock
HideSwap
LockSwap
Isolate
Unisolate
IsolateLock
UnisolateLock
Delete
Clear
Undo
Redo
ImportStl path/to/model.stl
ExportStl Binary path/to/model.stl
ExportStl Ascii path/to/model.stl
ImportStep path/to/model.step
ExportStep path/to/model.step
ExportStep Native=Yes path/to/editable.step
Import3dm path/to/model.3dm
Export3dm path/to/model.3dm
Help
```
