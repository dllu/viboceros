# SetPt

`SetPt target-point [XSet=Yes|No] [YSet=Yes|No] [ZSet=Yes|No]
[Alignment=World|CPlane] [Copy=Yes|No]`

Select point objects, whole objects, control grips (`PointsOn`), or curve edit
grips (`EditPtOn`). SetPt replaces the enabled coordinates with the target's
coordinates and retains the other axes. All axes are enabled by default;
at least one must remain enabled. This follows the coordinate constraint in
[McNeel's SetPt reference](https://docs.mcneel.com/rhino/8/help/en-us/commands/setpt.htm).

For example, select several displayed grips and run
`SetPt 0,0,5 XSet=No YSet=No ZSet=Yes` to align them at world Z=5. Unselected
control grips retain their positions. Edit-grip changes reconstruct the curve
through its other unchanged edit stations using the
[original-basis edit-point solver](edit-points.md). Both grip modes preserve
NURBS degree, knots, weights and parameter domain within their supported layouts.
If the parent is also selected, its selected grips determine the edit. Ordinary
selected objects can be edited in the same batch. Surface and mesh control grips
use the existing [grip transform](../grip-transforms.md) indexing and seam rules.

`Alignment=CPlane` constrains coordinates in the active construction plane.
Explicit command-line target coordinates are world coordinates. Interactive
typed targets use the viewport's point-input rules; the accepted target's
viewport supplies the constraint plane.

Run `SetPt` first to select objects or grips, then press Enter to pick the target.
Inline axis and Copy options survive source selection. With preselected inputs,
the target prompt starts immediately. Axis, alignment and Copy options can be
changed at that prompt. `Copy=Yes` duplicates the entire grip owner and edits
only the selected grips in each copy. Repeated target picks retain the original
source identities and indices, so later placements do not duplicate earlier
copies. Press Enter or Escape to finish; accepted placements remain. Each SetPt
placement keeps its existing separate Undo entry. `RememberCopyOptions` retains
the existing SetPt Copy preference policy.

Mixed object/grip edits stage all geometry before replacement. Invalid frozen
picks and rejected singular geometry preserve source geometry, picks, Undo and
Redo. A rejected target retains the point prompt for correction. Grip copies
retain their display mode; Undo/Redo use the existing grip display and selection
exchange policy.

During target input, moving the cursor previews the geometry and selected grips
in all four views. Whole objects use affine display instances; partial control
and edit-grip previews share the same prepared geometry as the existing affine
commands. Preview and commit use one SetPt coordinate-map function. CPlane
alignment uses the viewport receiving the target, including when switching views.
Copy previews retain the originals and use the frozen source identities.

SetPt projections can be singular. The preview validates whole-object and grip
edits before displaying a candidate batch. Rejected geometry retains the last
valid preview; with no earlier valid target, only the originals remain. Whole
geometry validation caches both success and failure by immutable source snapshot,
map and tolerance across the viewports. It does not build display meshes.
Changing axes or alignment clears an obsolete map; Copy changes retain its
geometry while updating display style. Leaving a viewport retains the last map.
Hover, preview updates and cancellation preserve model geometry, history, saved
Copy preferences and the last accepted point.

Implementation regressions cover rational control/edit grips, parent precedence,
ordinary point peers, World/CPlane constraints, original-basis structure,
unselected points, repeated copies, command-first options, failed targets and
Undo/Redo. The seven retained [SetPt point-witness comparisons](../plane-transforms.md)
qualify the coordinate map only. No fresh Rhino grip captures, performance
qualification, native mouse-preview appearance or full native grip
history/selection equivalence is claimed. Mouse hover/click tests independently
compare the preview and committed target; scene tests cover control/edit grips,
Copy, all four views and wireframe/shaded/ghosted modes. Additional checks cover
CPlane changes, singular whole-object rejection, reconstruction overflow,
last-target retention and cache invalidation. The pending
[12-recipe control-grip fixture](../../tools/rhino_oracle/fixtures/set_point_grips.json)
uses the public command/SDK probe and awaits live capture after the recorded
[Wine SSPI startup issue](../oracle-sspi.md) is resolved. It is not native evidence.
The [qualification record](../set-point-grips-qualification.json) retains source
hashes, checks and the limits of this implementation.

An explicit offscreen GPU test checks projected point/mesh previews, Copy source
styles, wireframe/shaded/ghosted output, two color formats and render-cache reuse.
It passed on the NVIDIA GB10 Vulkan adapter. This is implementation rendering
evidence; native preview appearance and performance remain unqualified. See the
[preview qualification record](../set-point-preview-qualification.json).

```sh
cargo test -p viboceros-command set_point
cargo test -p viboceros set_point
python3 -m unittest tools.rhino_oracle.test_grip_transform
cargo test -p viboceros gpu_affine_preview -- --ignored --nocapture
```
