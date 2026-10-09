# Edge analysis

Select NURBS surfaces, B-reps or meshes and run `ShowEdges`. With no selection,
the application prompts for eligible objects. A shared Edge Analysis window and
console session control colored edge overlays and endpoint markers.

```text
ShowEdges Show=Naked
ShowEdges Show=All Color=255,0,255
ShowEdges Show=NonManifold
ShowEdges Add
ShowEdges Remove
ShowEdges Zoom All
ShowEdges Zoom Current
ShowEdges Zoom Next
ShowEdges Zoom Previous
ShowEdges Mark
ShowEdgesOff
ZoomNaked
ZoomNonManifold
```

`Naked` is the initial mode. B-rep edges are classified by their trim use count;
mesh edges use exact-location topology and original polygon sides. `All` includes
every B-rep edge, but only naked or unwelded mesh edges. `NonManifold` selects
edges used by more than two faces. Closed boxes and spheres have no naked edges.
These classifications follow the [Rhino 8 command reference](https://docs.mcneel.com/rhino/8/help/en-us/commands/showedges.htm).

Mode and RGB color edits retain the current sources. `Add` and `Remove` use the
eligible selected objects. Bare `ShowEdges` replaces the sources when eligible
objects are selected. `ShowEdgesOff`, or closing the window, clears the display;
the session remembers its mode and color. `ZoomNaked` and `ZoomNonManifold`
select the corresponding mode and request a fit of the current edge in the
active viewport. Navigation wraps at the first and last displayed edges.

Display, color and navigation do not create model objects or history entries.
`Mark` creates two point objects on the current layer for each focused edge.
Coincident ends of a closed edge and corners shared by several edges retain
separate point objects. All focus marks every displayed edge; current focus
marks one. Actions execute in order and may repeat: `ZoomNaked Mark Next Mark`
marks the first and second edges in one Undo step. A fresh Zoom command starts
at the first eligible edge. The analysis session remains open after Undo.
Hidden objects and hidden layers suppress their overlays.

Geometry snapshots and tolerance changes invalidate the topology cache.
Unchanged frames and color edits reuse the edge data; the four viewports share
sampled curve segments. Sessions are limited to one million edges. Straight
degree-one edges draw their spans directly; other curves use the existing
viewport sampler.

The [validation record](../edge-analysis-provenance.json) records five command,
two application and one viewport regression in a full 5,610-test Rust run.
Ten [live Rhino SDK captures](../edge-analysis-native.md) also match edge order,
domains, classifications and curve samples without exclusions. Native dialog
behavior, zoom framing and curved display accuracy still need qualification.
Standalone SubD and extrusion types are not supported. Deleted
sources leave the session; after Undo restores an object, select and add it
again. Analysis settings are transient and are not saved in model files.

Fifty [native command workflows](../edge-mark.md) qualify endpoint duplicates,
point creation order, navigation wraparound and Undo/Redo for the recorded
surface, trimmed-hole, mesh and mixed-object cases. In naked/non-manifold All
focus, B-reps are marked before meshes; current navigation retains source order.
