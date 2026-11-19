# FilletEdge

`FilletEdge` rounds selected edges of closed B-reps with circular rolling-ball
fillets using usd-brep.

```text
FilletEdge NextRadius=1
FilletEdge 1 object-id 0,3,5
FilletEdge Radius=1 All=Yes
```

The first form starts viewport edge picking. `NextRadius` sets the size for
subsequent picks, including chains and face edges. Select edges and press Enter
to open size adjustment. Click a handle to type a radius, use `FromCurve` or
`FromTwoPoints`, or drag the handle up or down. Shift-click handles or drag a
window from empty viewport space to select several. Ctrl-click removes a
handle from the selection. Edit any selected handle to change the group.
`AddHandle` places a radius along a selected edge. `CopyHandle` asks for a source handle and a new location.
`RemoveHandle` deletes a placed handle. SMLib interpolates the radii over the
edge's curve parameter. An open edge with only endpoint handles varies
linearly. `LinkHandles=Yes` scales the other handles by the same ratio as the
edited handle. `SetAll=2` sets every radius and the next radius to 2. Use
`SelectEdges` to resume picking or
`ShowRadius=No` to hide labels until hovered. Press Enter to apply the blend.
Esc leaves an active size getter or cancels the command from the handle prompt.
Component preselection is retained for confirmation. The indexed form accepts
multiple object/edge-list pairs; repeated edge indices are applied once.
`All=Yes` uses edges shared by distinct faces of selected B-reps and excludes
same-face seams. Hidden or locked objects, stale geometry, changed tolerance,
invalid indices and unsupported inputs fail before document replacement.

Type `Preview` to toggle the pending result's wireframe. `Preview=Yes|No` also
works when starting the interactive command. The preview updates as picks,
radius or corner treatment change. Enter applies the matching staged result.
Use `TrimAndJoin=No` to preview and create separate fillet surfaces while keeping
the source B-rep intact. New surfaces inherit source object attributes such as
name, layer and color. The indexed and `All=Yes` forms accept this option too.
The default is `TrimAndJoin=Yes`.
Use `FromTwoPoints` to set the radius with two viewport picks or typed
coordinates. With preview enabled, the second point updates the wireframe.
`FromCurve` takes the local curvature radius at a picked or typed curve
location. Hovering with preview enabled shows the resulting blend. Esc returns
to the previous prompt, and a straight location leaves the curve picker active.
Use `FaceEdges` to add the sharp boundary edges of one or more faces. Enter
keeps those picks and returns to edge selection. Esc restores the prior edge
set. Tangent edges and same-face seams are excluded.
Use `ChainEdges` to select connected edges on closed polysurfaces. Set
`ChainContinuity`, `Direction`, `GapTolerance` or `AngleTolerance` as in
[connected-edge selection](curve-chains.md). `Next`, `All` and `Undo` edit the
chain. At a fork, pick a branch or press Enter to confirm the highlighted path.
Enter at the chain prompt adds the chain to the fillet selection. Esc cancels
the fillet. Double-click an edge at the main prompt to select its tangent chain.
`PreviousEdgeSelection` restores the most recent available canceled edge set.
Use `PreviousEdgeSelection=2` to recall the second most recent set. Each edge
blend command remembers up to 20 distinct sets in the open document. Recall
requires the original source geometry to be present and selectable.

Use `Edit` to select one joined result and press Enter, or double-click the
result to reopen its latest edge set. `Edit=object-id` opens a specific result.
The same workflow works for fillets, chamfers and blends. The original edges
and saved handles are available for adjustment and preview. Use `SelectEdges` to add or remove original edges.
Removing every edge and pressing Enter restores the geometry from before
that set. Edits are undoable. The edge set is available while the document
is open.

The operation stages every result, preserves object identity, attributes and
groups, and records one undo step. All inputs remain unchanged if any stage
fails. The adapter imports fresh native solids with original Rust edge-index
correspondence; it never guesses native edge numbers from topology enumeration.
It tightens circular cross-section generation and validates closed input and
output, boundary face intersections and unexpected contacts before conversion.
Pole connectors require a continuous surface-image certificate at their vertex.

This checkpoint supports variable radii on closed B-reps in Linux native
builds. `Corner=Ball` requires a common radius on each solid.
Open polysurfaces, alternative rail types and broader failure cases remain work.
The intersection guard bounds input and output to 362 faces. Kernel intersection curves must lie entirely on shared trimmed topological edges, and
isolated contacts must lie on shared edges or vertices. Adjacent faces are
checked too, with supplementary kernel searches for interior branches. The
check does not certify individual-surface injectivity or exhaustive intersection
discovery for arbitrary surfaces. Cases outside the qualified fixtures remain
unproven.

[Rhino's command reference](https://docs.mcneel.com/rhino/8/help/en-us/commands/filletedge.htm)
documents the full handle and rail workflow. The independent fixture capture uses
the public [`Brep.CreateFilletEdges` API](https://mcneel.github.io/rhinocommon-api-docs/api/RhinoCommon/html/M_Rhino_Geometry_Brep_CreateFilletEdges.htm)
under private Xvfb. See [implementation and qualification](../fillet-edge.md).
