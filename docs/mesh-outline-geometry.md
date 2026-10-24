# Mesh outline geometry

`TriangleMesh::outlines` produces coverage boundaries on a supplied plane using
orthogonal or central projection. Overlaps merge, holes remain, and face winding
and unwelded seams do not cancel coverage. Queries preserve source geometry and
channels. Quad splitting reuses public OpenNURBS triangulation; the projected
union uses the MIT/Apache-2.0 `i_overlay` dependency with its i64 engine.

The [MeshOutline command](commands/mesh-outline.md) uses this geometry with the
active viewport, object selection and history. Curved surface render mesh parity
and numerical boundary cases remain incomplete; see the command qualification.

## Native evidence

The [qualification record](mesh-outline-qualification.json) binds source hashes,
frozen producers and measured local contours. Of 196 replays, 176 match at `1e-7`
model-unit tolerance. The original 61 cover plane and viewport recipes: open
faces, boxes, holes, overlapping and touching sheets, warped quads, offset/tilted
planes, and parallel/perspective views. Another 135 isolate face representation,
camera clipping, document tolerance, model scale and cyclic/reversed face ordering.
Contour order and starting vertex may differ; comparisons retain orientation and
remove only redundant collinear vertices before matching cyclic boundaries.

Nineteen replays expose a perspective quad topology gap. In the captured warped
quad, changing only the far clipping distance from 43 to 44 changes Rhino's outline
from a concave quadrilateral to a triangle. Equivalent explicit BD triangles stay
triangular across the clipping sweep; rotating or reversing the quad record does
not change its result. Changing near distance or document tolerance alone in the
separate fixed-far controls does not reproduce the switch. A further triangle
replay at 100× uniform scale has a maximum coordinate discrepancy of
`3.1073e-7`. Both kinds of gap remain measured and excluded from qualification.
The implementation retains public OpenNURBS spatial quad triangulation until
there is evidence for a general replacement rule.

An initial 35 viewport observations did not record clipping distances. They remain
immutable exploratory evidence and are excluded from full-coordinate qualification.
The later viewport family explicitly sets and records all six frustum parameters.
In these captures, a parallel view aligned with the supplied construction plane
projects there. Other views project to normalized clipping depth zero: the
arithmetic mean of near/far distances for parallel views, their harmonic mean for
perspective. See the public [Mesh.GetOutlines overloads](https://developer.rhino3d.com/api/rhinocommon/rhino.geometry.mesh/getoutlines)
and [MeshOutline command](https://docs.mcneel.com/rhino/8/help/en-us/commands/meshoutline.htm).

The overlay uses an adaptive integer grid, not exact rational intersections.
Coordinates outside its documented binary64 range are rejected before calling
the adapter. A perspective triangle crossing the eye plane has an unbounded
image and is rejected. Native behavior at the eye plane and extreme numerical
ranges remains unqualified.

## Reproduce

```sh
cargo test -p viboceros-geometry --test mesh_outline -- --nocapture --test-threads=1 > /tmp/mesh-outline.log 2>&1
python3 -m tools.rhino_oracle.report_mesh_outline /tmp/mesh-outline.log
python3 -m unittest tools.rhino_oracle.test_mesh_outline
```

Native captures use a newly owned Rhino process on private Xvfb with a private
settings scheme. Generators refuse to overwrite existing observations.
