# Project onto meshes

[Project](project.md) accepts mesh targets for point and curve inputs in both
portable and `native-smlib` builds. Picking, preselection, direction, output
layers and DeleteInput use the same workflow as surface targets.
`Direction=Custom` uses the shared two-point drafting getter described in the
[Project reference](project.md).
Temporary SubCrv ranges use the same indexed mesh query as whole curves.

Each target gets one reusable bounding-volume index. Point queries intersect
both directions of the infinite line, admit closed triangle barycentrics with
exact rational arithmetic, and deduplicate identical locations across shared
edges and vertices. Nearby distinct sheets remain distinct. Final point
rounding must stay within the absolute document tolerance. Coplanar lines that
overlap a region are ambiguous and reject the command atomically; isolated
coplanar vertex contacts are supported.

Quads reuse the existing shorter-diagonal splitter, with exact length comparison
on ties or poorly conditioned chords. This follows the calculation convention
reported in [McNeel's projection/display issue](https://discourse.mcneel.com/t/v6-quad-mesh-triangulation-vs-opengl-differ/53802),
rather than assuming a warped quad has the bilinear MeshToNURB image. The
viewport currently uses its existing fixed display diagonal; a warped quad's
projection and displayed tessellation can therefore differ.

Curve queries use conservative control bounds and, where applicable, an exact
common ruled plane to prune candidate triangles. Existing planar n-gon discovery
coalesces connected candidate regions only after independent exact planarity
checks. Existing trimmed triangle and planar face construction, analytic
planar overlap queries, the selected curve/edge contact producer and interval
trimming provide the curve images. Native builds retain usd-brep edge queries
and trimming. Since each affine image is already planar, it does not require a
numerical curve/surface solve to discover its plane relation.

Exact duplicate curve definitions are removed. The shared Join implementation
receives a source-parameter predicate and zero endpoint tolerance, preserving
control images while joining consecutive source spans. Approximate contacts
and unmerged coplanar regions can remain separate. Isolated triangle contacts
of a curve input create no zero-dimensional output, matching the curve-only
return type of [Curve.ProjectToMesh](https://developer.rhino3d.com/api/rhinocommon/rhino.geometry.curve/projecttomesh?overload=2).
Existing curve/face clipping restrictions still apply; this increment does not
prove complete discovery or broad output-topology parity with Rhino.

The index admits at most one million triangles. Coplanar candidate groups with
holes may remain as separate triangle faces. Source common-sign weight checks,
face preparation, existing contact and Join budgets can decline difficult
inputs. Mesh targets are not converted into saved model surfaces or altered.

Qualification includes independent exact membership, one-ulp misses, nearby
sheets, coplanar contacts, rounding limits, shortest-diagonal invariance,
conservative random slab checks and indexed grid pruning. Command and UI
regressions cover rational curves, shared diagonals, warped quads, layer/history
behavior, filtered picking and atomic miss/failure cases. Thirteen Python API
recipes retain native and portable outputs. Their SDK counterparts use public
ProjectToMesh and ProjectPointsToMeshes with explicit ownership cleanup.
The 2026-10-14 isolated Rhino attempt timed out before the worker responded.
Live Rhino parity and comparative performance remain unqualified.

```sh
cargo test -p viboceros-geometry --lib mesh::projection
cargo test -p viboceros-command --features native-smlib project::tests
python3 -m tools.rhino_oracle viboceros \
  tools/rhino_oracle/fixtures/project_mesh.json --native-smlib
```

See the [qualification record](../project-mesh-qualification.json) for retained
responses, checks and source hashes. Historical captures keep their original
source hashes through Git references when shared code moves or changes.
