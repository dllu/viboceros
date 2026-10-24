# Polygon Boolean backend

With the `viboceros-smlib/native` feature, `viboceros_smlib::mesh_boolean` computes
union, intersection or first-minus-second on polygon meshes. It imports owned
working copies into usd-brep/SMLib, reuses its polygon Boolean and triangulation
code, and returns an optional triangle mesh. Empty results return `None`.
Disconnected regions share one returned mesh. Source geometry and stored channels
remain unchanged; output texture/color correspondence is not implemented.
The application exposes this API through the [mesh Boolean commands](mesh-boolean-commands.md).

Input quads use the existing public OpenNURBS spatial diagonal conversion.
Exactly coincident input vertices share working topology, including unwelded
seams. SMLib triangulates output polygon faces, including holes. The adapter owns
surviving kernel operands through failure and consumption, bounds input/output
sizes, and uses the existing process-wide native-call lock.

## Evidence and limits

The [qualification](mesh-boolean-qualification.json) compares 48 immutable Rhino
SDK captures with measured output geometry. Twenty-six match at `1e-7` model-unit
tolerance, using area, oriented volume and bidirectional surface samples. This
sampling does not prove continuous surface equality. The cases cover overlapping,
disjoint, contained, touching, rotated and unwelded boxes, plus repeated and
cyclically reordered coincident operands.

Coincident operands and two face-contact cases have native compatibility gaps.
The repeated capture verifies actual source vertices/faces before and after each
operation. These differences remain explicit; the adapter retains set-operation
semantics rather than rules tied to particular reference scenes. Analytic tests
also check coincident volumes, empty results, through holes, attributes on source
vertices and repeated native ownership.

Command selection, object attributes and history have a separate qualification.
Curved-surface tessellation and extreme numerical scales remain unqualified.

```sh
cargo test -p viboceros-smlib --features native --test mesh_boolean -- --nocapture --test-threads=1 > /tmp/mesh-boolean.log 2>&1
python3 -m tools.rhino_oracle.report_mesh_boolean /tmp/mesh-boolean.log
python3 -m unittest tools.rhino_oracle.test_mesh_boolean
```

Native captures use an owned process, private settings scheme and private Xvfb.
Public API: [Rhino mesh Boolean methods](https://developer.rhino3d.com/api/rhinocommon/rhino.geometry.mesh).
