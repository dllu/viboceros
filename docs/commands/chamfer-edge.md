# ChamferEdge

`ChamferEdge` uses usd-brep's native
selected-edge fillet executive with a linear cross-section, retaining editable
B-rep surfaces, trims and topology.

Start `ChamferEdge`, set `NextChamferDistance` for subsequent picks, and press
Enter to adjust the selected edges' distances.
Click or drag a handle, use `SetAll` for a common distance, or use
`SelectEdges` to resume picking. Use `AddHandle`, `CopyHandle` and `RemoveHandle`
as in [FilletEdge](fillet-edge.md). `LinkHandles=Yes` scales the other handles by
the same ratio as the edited handle. Varying sizes along an edge require
`RailType=RollingBall`.
`ShowChamferDistance=No` hides size labels until
hovered. Press Enter from adjustment to apply. Edge-component preselection,
click/window selection and component ambiguity choices work. Enter `Distance`
followed by a value, or `Distance=0.5`; interactive distances accept units such
as `2mm`. Change `RailType=DistBetweenRails|RollingBall` while selecting. Invalid
options, changed sources and failed geometry preserve the getter for retry.
Cancel/Esc releases it without geometry changes.

Use `TrimAndJoin=No` to preview and create separate chamfer surfaces. The option
also works with explicit edge indices and `All=Yes`.

`Preview` toggles a wireframe of the staged result. `Preview=Yes|No` works at
startup and while picking. Changes to the edge set, distance or rail update it.
`FromTwoPoints` sets the distance from two picked or typed points and previews
the measurement as you move the second point.
`FromCurve` uses the local curvature radius at a picked or typed curve
location for the chamfer distance. With preview enabled, move over the curve
to preview the chamfer at that value.
`FaceEdges` adds sharp face-boundary edges. Enter keeps the picks and returns
to edge selection. Esc restores the prior edge set.
Use `ChainEdges` for [connected-edge selection](curve-chains.md), or
double-click an edge to select its tangent chain.
`PreviousEdgeSelection` recalls canceled picks. Use `PreviousEdgeSelection=2`
to select the second most recent set.
Use `Edit` or double-click a joined result to [reopen the latest edge set](fillet-edge.md).

For scripts, address original edges by object ID and index, or select whole
objects and use All=Yes:

```text
ChamferEdge Distance=1 RailType=DistBetweenRails object-id 0,2
ChamferEdge 1 object-id 0 second-object-id 3,4
ChamferEdge Distance=1 RailType=RollingBall All=Yes
```

Distances must be finite and positive. Defaults are `Distance=1` model unit and
`RailType=DistBetweenRails`. That mode measures the width between the two rails:
a width-one chamfer at a right angle cuts back each face by `1/sqrt(2)`.
RollingBall measures rolling-ball radius; at a right angle, radius one cuts back
each face by one. A single distance applies to all selected edges.

Sources currently require closed manifold B-reps. Duplicate edges are processed
once. Source geometry is imported into an owned temporary native solid and the
original edge index map is retained. Work is localized before native modeling
to reduce coordinate cancellation. Every result is staged before document edits;
object IDs, attributes and groups remain intact. One Undo/Redo step covers the
entire batch. A failed later source preserves earlier sources and history.
FilletEdge and ChamferEdge share this staging and selection workflow. Chamfer
imports use a working absolute tolerance no larger than `distance*1e-9`; declared
source edge/vertex tolerances remain intact. This avoids a native curved-rim
failure at coarse ambient tolerances without changing the kernel's solvers.
The existing native boundary validator currently limits each body to 362 faces.

Native tests cover every box edge, both distance modes, intersecting chamfers,
acute/obtuse prism edges, concave notches and curved cylinder rims. Python recipes resolve
physical edge points independently in each engine and call public
[Brep.CreateFilletEdges](https://mcneel.github.io/rhinocommon-api-docs/api/RhinoCommon/html/M_Rhino_Geometry_Brep_CreateFilletEdges.htm)
with BlendType.Chamfer and the requested RailType. See the
[qualification record](../chamfer-edge-qualification.json) for current evidence.
Licensed Rhino capture remains pending after the recorded
[Wine startup failure](../oracle-sspi.md); full shape/getter parity and comparative
performance are unqualified.

Variable `DistBetweenRails` distances, `DistFromEdge`, open polysurfaces,
Persisted edge sets and saved native defaults remain future work. General collapsing offsets and
self-intersections require broader qualification.

Reference: [Rhino ChamferEdge](https://docs.mcneel.com/rhino/8/help/en-us/commands/chamferedge.htm)
and [RailType meanings](https://mcneel.github.io/rhinocommon-api-docs/api/RhinoCommon/html/T_Rhino_Geometry_RailType.htm).
