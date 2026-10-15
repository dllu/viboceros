# Pull curves to meshes

`Pull Sources=curve-id Targets=mesh-id` creates a polyline following the mesh's
closest path. It uses the same source/target picker, temporary subcurves,
DeleteInput, output layers and Undo/Redo edit as other Pull targets. It works in
portable and native builds. A fully collapsed curve image creates no curve and
preserves the source, including with DeleteInput=Yes.

The geometry entry points are `MeshProjectionIndex::pull_curve` and
`pull_polyline`. A supplied polyline keeps its source chords and corners.
Degree-one NURBS curves use their exact span chords, including rational weights,
reversals and constant spans. Their nonuniform parameter speed needs no extra
vertices in the geometric mesh image. Other curves use the shared exact
[bounded approximation](../curve-approximation.md) at one quarter of absolute
model tolerance.

For each source chord, the existing BVH conservatively prunes triangles using
exact distance ordering and a feasible mesh vertex as an upper bound. Each
remaining triangle contributes affine closest maps for its face, edges and
vertices. Exact rational halfspace restrictions find their source intervals.
Quadratic Bernstein bounds compare candidate distances over complete intervals;
endpoint or midpoint sampling alone cannot admit a path. Small unresolved
transitions require a common spatial bound. Shared-edge transitions therefore
follow folded faces instead of connecting distant feet through empty space.
Quads retain the existing shorter spatial diagonal.

Large candidate sets can partition the source parameter interval into smaller
queries. The original rational chord remains unchanged. Outward endpoint boxes
and exact comparisons against feasible vertices preserve conservative pruning.
Partitioning targets 64 candidates, stops at depth 16, and falls back to the
original path when it cannot reduce the largest local candidate set. The same
global work and error limits still apply. See the
[local performance measurements](../mesh-pull-performance.md).

Output segments remain within absolute model tolerance of the mesh, accounting
for rounded coordinates and small transition bridges. The nearest image is
resolved for the approximating source chords. On a nonconvex mesh this does not
prove stability of the original curved source's closest branch under source
approximation. Global branch stability, Rhino tessellation/vertex counts and
native command interaction remain unqualified.

Each resolved affine segment lies in a closed triangle feature. Endpoint
rounding is bounded by tolerance/16. Consecutive feet may bridge only when their
distance is at most tolerance/2, so the bridge stays within 5*tolerance/16 of the
mesh. An unresolved distance crossing may be accepted only when every remaining
candidate's complete affine image fits in a ball of radius tolerance/16.

Discontinuous closest branches, including jumps across some connected concave
folds or disconnected components, decline when they cannot fit within tolerance.
The whole command rolls back, including earlier staged targets. Mixed-sign
curve weights and approximation limits inherit the shared sampler's policy.
Tracing additionally bounds work to 4,000,000 units, retained features per chord
to 131,072, rational sizes to 8,192 bits, subdivision depth to 64 and output to
the shared curve-division point limit. Dense candidate sets and tight tolerances
can exceed these bounds. Comparative speed is not qualified.

The Python `pull_curve_mesh` operation takes a full `curve`, mesh `vertices`
and triangle/quad `faces`. Viboceros runs the Pull command; Rhino uses public
[Curve.PullToMesh(mesh, tolerance)](https://mcneel.github.io/rhinocommon-api-docs/api/RhinoCommon/html/M_Rhino_Geometry_Curve_PullToMesh.htm).
Both return arc-length samples, length and closedness. SDK geometry comparison
does not qualify native command metadata. Worker tests cover source/result/mesh
ownership at SDK and sampling failures. Fresh live Rhino capture remains
unavailable because of the [existing Wine SSPI issue](../oracle-sspi.md).

```sh
cargo test -p viboceros-geometry --lib mesh::projection
cargo test -p viboceros-command --lib pull::tests
cargo test -p viboceros app::tests::project::pull
python3 -m tools.rhino_oracle viboceros \
  tools/rhino_oracle/fixtures/pull_curve_mesh.json
```

See [qualification and source provenance](../mesh-pull-qualification.json).
