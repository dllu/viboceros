# ChamferEdge

Build with `--features native-smlib`. `ChamferEdge` uses usd-brep's native
selected-edge fillet executive with a linear cross-section, retaining editable
B-rep surfaces, trims and topology.

Start `ChamferEdge`, select edges and press Enter. Edge-component preselection,
click/window selection and component ambiguity choices work. Enter `Distance`
followed by a value, or `Distance=0.5`; interactive distances accept units such
as `2mm`. Change `RailType=DistBetweenRails|RollingBall` while selecting. Invalid
options, changed sources and failed geometry preserve the getter for retry.
Cancel/Esc releases it without geometry changes.

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

Per-edge distances, variable handles along edges, DistFromEdge, open polysurfaces,
TrimAndJoin=No, previous edge sets, chain/face selection, previews, History editing
and saved native defaults remain future work. General collapsing offsets and
self-intersections require broader qualification.

Reference: [Rhino ChamferEdge](https://docs.mcneel.com/rhino/8/help/en-us/commands/chamferedge.htm)
and [RailType meanings](https://mcneel.github.io/rhinocommon-api-docs/api/RhinoCommon/html/T_Rhino_Geometry_RailType.htm).
