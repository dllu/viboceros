# Mesh outline geometry

`TriangleMesh::outlines` produces coverage boundaries on a supplied plane using
orthogonal or central projection. Overlaps merge, holes remain, and face winding
and unwelded seams do not cancel coverage. Queries preserve source geometry and
channels. Quad splitting reuses public OpenNURBS triangulation; the projected
union uses the MIT/Apache-2.0 `i_overlay` dependency with its i64 engine.

The `MeshOutline` command, surface render mesh inputs, selection/history behavior,
and application viewport integration are still pending. This geometry API does
not register a command or claim complete Rhino compatibility.

## Native evidence

The [qualification record](mesh-outline-qualification.json) binds source hashes,
frozen producers and measured local contours. Its 61 comparisons cover 24 plane
recipes and 37 viewport recipes: open faces, boxes, holes, overlapping and touching
sheets, warped quads, offset/tilted planes, and parallel/perspective views. Contour
order and starting vertex may differ; comparisons retain orientation and remove
only redundant collinear vertices before matching cyclic boundaries.

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
ranges needs further qualification before command integration.

## Reproduce

```sh
cargo test -p viboceros-geometry --test mesh_outline -- --nocapture --test-threads=1 > /tmp/mesh-outline.log 2>&1
python3 -m tools.rhino_oracle.report_mesh_outline /tmp/mesh-outline.log
python3 -m unittest tools.rhino_oracle.test_mesh_outline
```

Native captures use a newly owned Rhino process on private Xvfb with a private
settings scheme. Generators refuse to overwrite existing observations.
