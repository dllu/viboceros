# ExtractPt components

[ExtractPt](extract-points.md) accepts mesh and SubD faces, edges and topology
vertices. Use Ctrl+Shift to pick components before starting or while the command
is selecting. Window selection is also supported. Components override whole
extraction for the same owner. Other selected whole objects still contribute. Selected grips override
components on the same owner.

Typed input accepts repeatable, zero-based component ordinals:

```text
ExtractPt Component=uuid,Face,0 Component=uuid,Edge,2
ExtractPt Component=uuid,Vertex,0 Output=PointCloud OutputLayer=Current
```

Each UUID identifies a selectable mesh or SubD; invalid or unavailable sources
reject the complete invocation before edits. Duplicate component arguments are
processed once. Typed face indices refer to stored faces; mesh edge and vertex
indices refer to exact-location-welded topology, including unwelded meshes.

Mesh component locations are combined, deduplicated exactly and sorted by
coordinates. SubD components contribute in vertex, edge, face order and then
ascending component ordinal. With multiple component owners, preselection follows
document order and command-first extraction follows owner pick order. Each face retains its corner order, including shared locations
repeated by another selected component.

`SubDLocation=Surface` is the default. Selected SubD vertices and face corners
contribute their limit locations; smooth edges contribute the control points of
clamped cubic edge curves. Regular curves have four or five controls; curves
near extraordinary vertices use an endpoint-constrained, eight-span fit to 17
exact limit stations, yielding eleven controls. This approximates the edge
between stations and has no continuous model-tolerance guarantee.

`SubDLocation=ControlNet` extracts control vertices, face corners and oriented
edge endpoints. This explicit Viboceros option supplies control-net locations
without requiring a flat SubD viewport. Flat SubD rendering is not implemented.
Whole SubD extraction continues to return its control vertices.

Output attributes, groups and layering follow ExtractPt's ordinary options.
Preselected components remain selected immediately after extraction; Undo and
Redo release them. Command-first extraction releases component and grip picks
when it finishes. Source geometry remains unchanged. Stale component witnesses
reject the operation rather than silently selecting another source. The
one-million-output-point limit and native sampling budgets apply; fitting a
SubD edge currently refines the source globally within those budgets.

The [component qualification](../extract-point-components-qualification.json)
compares 94 native cases through typed commands and their corresponding
interactive selection paths (94 getter workflows), including welded/unwelded meshes, combined
components, point clouds, surface/control-net locations, attributes and history.
Another 67 edge forms from irregular, regular, open-boundary, triangular and
creased SubDs compare degree, knots, controls and 17 evaluations. These checks
use an epsilon of `1e-7` against Rhino output, independently of the approximation
error between a fitted edge and the exact limit surface.

Original requests, source 3DMs, native producer snapshots and observations remain
bound to the report. Earlier diagnostic captures are retained; corrected mesh
vertex selection captures recognize Rhino's `MeshVertex` component records.
