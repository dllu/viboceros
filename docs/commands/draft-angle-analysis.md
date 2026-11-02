# Draft angle analysis

[Analysis commands](analysis-index.md) · [Direction analysis](direction-analysis.md)

`DraftAngleAnalysis` colors selected surfaces, polysurfaces, meshes and SubDs by
their oriented normals, or asks for objects first. Its panel stays open while
modeling. `DraftAngleAnalysisOff` closes it and clears membership.

The panel controls the lower red and upper blue angles, pull vector, and visible
edges/isocurves. Viboceros starts with a 0°–5° range and world Z pull vector.
Colors run through yellow, green and cyan between red and blue. The mapping
interpolates normal dot products, following OpenNURBS, and supports reversed or
equal bounds. Flipping a source changes its analysis colors.

Analysis faces are opaque and unlit in Wireframe, Shaded and Ghosted, and remain
pickable in Wireframe. Camera-independent color buffers share the existing display
mesh and normal caches across viewports; geometry, tolerance or settings changes
refresh the appropriate buffers. Source geometry and mesh vertex colors stay intact.

Starting analysis clears input selection. Repeating it adds selected objects;
selection changes alone leave membership intact. Hidden sources/layers do not
draw. Delete removes the live display and Undo restores it. Display settings and
membership stay outside model history; turning analysis off stays off through
subsequent Undo/Redo. Panel display controls preserve active point getters.
Draft and [curvature analysis](curvature-analysis.md) colors replace each other
only on the targeted objects.

Set the pull direction from world Z, the active CPlane Z, the view toward the
camera, two picked points, a curve/edge start tangent, or a named CPlane Z.
Choosing a frame/view direction updates it immediately; Set direction updates
it again from the current viewport. Two points and Curve or edge start their
pickers with Set direction. Escape discards an unfinished pick. Picks use the
normal point/snapping and curve/edge selection paths and create no geometry.

These console bindings expose panel actions as Viboceros extensions:

```text
DraftAngleAnalysis [Add|Remove]
DraftAngleAnalysis [Range=lower,upper] [Up=x,y,z] [ShowIsoCurves=Yes|No]
DraftAngleAnalysis Direction=WorldZ|CPlaneZ|View|TwoPoints|Curve
DraftAngleAnalysis Direction=NamedCPlane plane name
DraftAngleAnalysisOff
```

Ranges use degrees within ±90°. `Up` accepts finite nonzero vectors; use a unit
vector for angular interpretation. Like the public OpenNURBS/RhinoCommon mapping,
nonunit vectors scale the dot products. Command entry with mesh inputs enables
edges; a subsequent `ShowIsoCurves=No` can hide them. Closing/reopening retains
preferences within the registry session.
Two-point picking stores `end - start` without normalization, following the
native panel; curve/edge picking stores its oriented unit tangent at the start.
The console direction binding performs the Set action, beginning a picker for
TwoPoints or Curve. World/CPlane/view/named-frame controls preserve active point
getters; starting a direction picker begins a new input command.

22 frozen Rhino 8 recipes qualify selection, membership, visibility, settings
and history for meshes, planar panels, a rational patch, a holed face, box and
sphere. All 10,160 saved normal/color pairs match exactly, including reversed,
equal and almost-equal ranges and alternate pull vectors. The probe explicitly
seeds ranges in radians through RhinoCommon; native factory preferences are
recorded but are not part of the default-settings qualification.

Application replays cover command-first picks and preselection. Offscreen GPU
tests cover both color-target formats and all three display modes. This does not
establish identical tessellation, native pixels, saved-file analysis persistence,
or general performance parity. Another 26 native direction cases cover the five
choices available in the oracle build, including acknowledged physical surface-edge
picks and their start tangents. Named CPlanes have application frame tests.
Another [35 camera/vector cases](../oracle/camera-vectors.md) cover raw camera
metadata, navigation resets and nonunit View pull vectors. Import, named views and
SaveAs/Open retain raw camera vectors independently of the rendering frame.
Mesh-density controls and draft-curve extraction remain unimplemented.

```sh
cargo test -p viboceros-command --lib draft_angle_analysis
cargo test -p viboceros-oracle --test draft_angle_native
cargo test -p viboceros --bin viboceros draft_angle
cargo test -p viboceros --bin viboceros false_colors_are_unlit -- --ignored
python3 -m unittest tools.rhino_oracle.test_draft_angle
cargo test -p viboceros --bin viboceros draft_pull
python3 -m unittest tools.rhino_oracle.test_draft_pull
```

Reference: [Rhino DraftAngleAnalysis](https://docs.mcneel.com/rhino/8/help/en-us/commands/draftangleanalysis.htm).
