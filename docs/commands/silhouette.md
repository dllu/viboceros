# Silhouette

`Silhouette [InternalEdges=Yes|No] [Direction=x,y,z]` creates spatial NURBS
outline curves from selected surfaces, B-reps and manifold meshes. Build with
`--features native-smlib`; the operation uses USD BRep's SMLib visibility engine.

Start the command, select sources, set `InternalEdges` if needed, then press
Enter. Preselection also works. The application uses the active parallel camera
direction, independently of its construction plane. `Direction` explicitly gives
a nonzero vector **toward the viewer**, requires preselection, and describes a
parallel view even when the viewport is Perspective. Without that option,
perspective viewports decline the operation and retain source picks.

Outputs keep their spatial depth; they are not flattened onto the CPlane. Source
geometry remains unchanged. Results use the current layer, become selected, and
form one Undo/Redo operation. Every source completes before results are admitted;
a failing source leaves the document unchanged.

`InternalEdges=No` is the default. `Yes` includes the kernel's visible feature,
smooth and seam edges. Visibility is computed separately for each source object.
Meshes use their actual triangulation, including warped quads; internal edges can
therefore include triangulation edges. Non-manifold mesh imports may decline.
The command limits the combined output to 100,000 curves.

The native adapter receives the document's absolute tolerance, splits it between
silhouette tracing and curve conversion, and uses a 0.05-radian tracing angle.
These are kernel approximation settings, not a continuous geometric error
certificate. Perspective silhouettes, clipping planes, `ProjectToCPlane`, full
Rhino `InternalEdges` compatibility and performance parity remain unqualified.

Tests cover box depth and direction reversal, sphere equators in axial and
oblique views, torus inner/outer outlines, open surfaces, meshes, rollback,
viewport direction and history. Seven pending Python oracle recipes retain raw
curve definitions, samples and lengths; they use the public
[RhinoCommon Silhouette API](https://developer.rhino3d.com/api/RhinoCommon/html/T_Rhino_Geometry_Silhouette.htm).
No fresh Rhino capture is claimed: the licensed Wine instance has the recorded
[SSPI startup failure](../oracle-sspi.md). SDK and command segmentation, visibility,
orientation and seams still need comparison. See the
[qualification record](../silhouette-qualification.json).

Run the local recipes after building the application with the native feature:

```sh
python3 -m tools.rhino_oracle viboceros tools/rhino_oracle/fixtures/silhouette_parallel.json --native-smlib
```

The same request is accepted by `OracleClient.run_rhino` for future public SDK
capture. Raw curve order and parameterization are deliberately retained rather
than normalized into an asserted parity result.
