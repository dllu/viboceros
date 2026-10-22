# SubDCrease

[Meshes and SubDs](meshes-index.md) · [Hard creases](crease.md)

`SubDCrease` edits soft weights on interior SubD control edges. Ctrl+Shift
preselect edges and run the command, or run it first, select edges, and press
Enter. Whole-object, boundary-only and vertex-only selections do not select an
interior edge chain automatically.

Click a highlighted vertex label, enter a weight from 0 to 100, and continue
editing. Labels follow the preview's limit surface. Existing profile bends and
discontinuities constrain interpolation along the selected control edges.
Invalid weights retain the getter for retry. Enter at the outer prompt commits
one Undo step; Escape discards every pending edit and releases component and
grip selection. Existing `PointsOn` grips stay on.

`ConstantWeight` sets all selected chains uniformly. At a vertex prompt,
`ConstantWeightChain` and `CreaseThisChain` affect its chain. `CreaseAll` makes all
selected chains hard; `Crease` inside a constant-weight prompt makes that prompt's
chains hard. Soft 100 remains different from a hard crease. `SmallerText` and
`LargerText` resize the labels.

For scripts, select the parent objects and use source control-table ordinals:

```text
SubDCrease Edges=0,1,2 Weight=50
SubDCrease Edges=0,1,2 Vertex=0 Weight=80 Vertex=1 Weight=20
SubDCrease Edges=All Chain=0 Weight=70
SubDCrease Edges=All Chain=0 CreaseThisChain
SubDCrease Edges=All CreaseAll
```

`Object=<selected-uuid>` restricts the input. `Chain` indices follow document
source order and the native control-chain sorter. Specify a chain when a vertex
belongs to several selected chains. The complete edit is staged before
admission; stale geometry, tolerance or editability rejects it atomically.
Changed objects retain IDs and attributes and move to the document end in their
original source order. No-op and cancelled edits preserve Redo.

The [session qualification](../subd-crease-session-qualification.json) compares
48 native workflows and 96 application replays. The
[chain observations](../oracle/soft-crease-chains.md) describe these Rhino 8.32
profiles, including asymmetric odd loops. Sampled surface comparisons
use `1e-6`; arbitrary profiles, continuous surface error and comparative
performance remain unqualified. See [sharpness storage](../geometry/subd-soft-crease.md)
for native archive normalization near integer thresholds.
