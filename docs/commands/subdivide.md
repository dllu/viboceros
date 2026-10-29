# SubDivide

`SubDivide` refines selected meshes and SubDs once, replacing their geometry
while retaining object IDs, names, layers, colors and groups. Each invocation
creates one Undo entry. Repeat the command for another level.

Select whole objects before running the command, or run it and select objects
at the prompt, then press Enter. Ctrl+Shift clicks and selection windows can
select mesh or SubD faces before or during the prompt. Component selection clears
when the command finishes. Rhino briefly selects partial SubD children during its
command callback; those picks are gone when control returns to the command line.
Select children explicitly to refine them again. Selecting every SubD face follows
the whole-object path.
Escape cancels; `SelNone` clears the prompt's whole-object and face selection.

Scripts can target zero-based stored face indices on selected objects:

```text
SubDivide
SubDivide Faces=0,2
SubDivide Faces=All
SubDivide Faces=0 Object=selected-object-uuid
SelSubD
SelOpenSubD
SelClosedSubD
```

`Object=` limits an explicit `Faces=` request to one selected object. Invalid
indices or stale source geometry reject the complete operation. Viewport picks
select complete displayed mesh n-gons; typed indices address their individual
stored triangle/quad members. SubD picks address control faces using the evaluated
surface. Clicks, crossing/window selection and highlights use cached face regions
in wireframe, shaded and ghosted views.

SubDs use public OpenNURBS Catmull–Clark subdivision. Mesh subdivision keeps
original vertices fixed, creates raw-edge midpoints and face centers, and refines
neighboring faces to maintain shared edges. Raw seams, vertex colors and
unselected n-gon regions survive. Triangle centers preserve the captured native
float-reciprocal arithmetic, including its effect at large coordinates. RGB and
stored transparency round independently.

The captured Rhino 8 build accepts one level per invocation, with no numeric
iteration or `DeleteInput` option. SubD output is bounded to 100,000 control
vertices/faces and 400,000 edges/corners. Mesh output and aggregate staged
candidates are bounded to 4,000,000 vertices and 1,000,000 faces.

[Recorded qualification](../subdivide-qualification.json) covers 141 native
workflows: whole/local/repeated edits, triangle/quad neighbor masks in both
windings, warped faces, seams, colors, n-gons, tags, attributes, selection and
Undo/Redo. The command comparison includes 139 constructive workflows; the GUI
getter comparison includes 135 workflows, excluding six partial raw n-gon
recipes. Mesh checks compare oriented stored pieces and triangle facets; SubD
checks compare controls, tags, limit stations and corresponding density-two facets
within `1e-6`. These checks do not certify continuous surface error, every topology,
texture/material channels, physical native mouse gestures or comparative speed.

Native captures use public component-selection APIs. Viewport gesture tests
independently cover our frontend. Full native parity and performance remain
unqualified. The [discovery capture](../../tools/rhino_oracle/observations/subdivide_discovery.json)
retains rejected option experiments separately from the supported totals.

[Sequential edit validation](../geometry/subd-edit-history.md) checks component IDs,
SDK/archive copies, completed-command selection and MergeFaces→SubDivide history.
