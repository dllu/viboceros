# Shared-boundary B-rep meshing

[Mesh commands](commands/meshes.md) · [B-rep morphing](brep-morphing.md) · [Architecture](architecture.md)

Smooth-seam meshing checks open shells as well as closed solids. A closedness
test alone cannot distinguish an intentional opening from a crack between
adjacent faces with different knot grids.

## Meshing and audit

`brep/tessellation/independent` samples each face's surface grid or constrained
trim region. Compatible regular grids retain quadrilaterals. Each generated
polygon records its source B-rep face. `TriangleMesh::topology_with_boundary`
builds exact-location topology and retains the original polygon/raw-vertex
indices of every naked mesh side in one pass.

`brep/tessellation/audit` indexes B-rep edge incidence once. It requires every
naked mesh side to match a naked B-rep edge on that side's own source face, and
each source naked edge must receive a match. Unrelated faces cannot supply
boundary membership. When the source is manifold, consistently oriented, or
closed, the mesh must preserve the corresponding property. Missing holes and
unexpected internal cracks therefore cannot pass merely because the source is
already open.

Membership uses exact shared sample lookups, conservative same-sign rational
control/endpoint bounds, cached point/edge queries, and native curve closest points.
Points introduced by splitting a sampled boundary chord are also accepted on
that chord: touching trim loops can require such splits without placing the
split point on the exact curved edge. Same-sign degree-one spans have precisely
their exact line-segment loci and avoid general closest-point searches when
all spans are representable as nondegenerate lines. Those exact spans are kept
separately from vertex-snapped chords, since loose topological endpoint vertices
need not lie exactly on their edge curves.
Recorded component tolerances retain their model-space meaning; UV tolerances
do not expand spatial matching. Bounding checks include endpoint allowances.

When independent sampling fails this audit, `brep/tessellation/conforming`
uses one canonical sample table per shared edge, projects it onto incident UV
trims, and constrained-triangulates the faces. The result must pass the same
audit. This fallback may replace quads with triangles; it does not cap openings.
Full-order interior surface breaks remain conservatively unsupported by the
fallback, even with coincident limits, because it does not partition positional
jumps. An interior crack must not be bridged to manufacture a watertight mesh.

`JaggedSeams=Yes` skips both snapping and the shared-boundary audit. This now
applies to planar and nonplanar trimmed faces, not only full-domain rectangles.
The surface and UV trims determine their mesh; changing the independent 3D
edge fit does not move jagged-mesh vertices.

These are sampled membership/incidence checks, not continuous boundary coverage
or self-intersection proofs. Ambiguous coincident geometry, ill-conditioned
projection, degeneracies, and resource limits can prevent reconstruction.
`Density` remains bounded per-knot-span sampling: joining tolerance is not a
universal chord-height error guarantee, and Rhino's exact mesh layout is not
reproduced.

## Regression and oracle evidence

Before the fixes, an open two-face unit-box corner had naked boundary length
`8` instead of `6`, an open face's interior jump passed tessellation, and jagged
trimmed faces snapped to displaced edge curves. Those regressions now pass,
alongside tests for preserved quads, omitted holes, incorrect face ownership,
polygon-soup provenance, and morphed capped cylinders. Existing internally
touching curved-loop cases still mesh. The large trimmed-face case includes
273 loops and 17,472 boundary segments, exercising the indexed matching path.
A loose-endpoint regression also verifies that independently sampled linear
boundaries retain valid quads instead of triggering unnecessary reconstruction.

`brep_mesh_boundaries.json` independently builds unit-box face subsets in both
engines, selects faces by geometric centers, and inserts unequal U/V knots
without changing their loci. Five cases cover an open corner, five-face open
box, closed box, disconnected opposite faces, and translated open corner.
The native probe additionally executes `Mesh` and verifies unchanged selected
source geometry plus an unselected result identical to direct meshing.

Rhino uses public [Mesh.CreateFromBrep](https://developer.rhino3d.com/api/RhinoCommon/html/M_Rhino_Geometry_Mesh_CreateFromBrep_1.htm)
and [GetNakedEdges](https://mcneel.github.io/rhinocommon-api-docs/api/RhinoCommon/html/M_Rhino_Geometry_Mesh_GetNakedEdges.htm).
Comparisons cover area, boundary length, loop count/closure, closed/manifold/
orientation flags, and sampled positions along every naked source edge.
They do not require the engines to produce identical tessellation vertices or
face counts.

On Rhino 8.32, `IsManifold(true)` on appended, unwelded face meshes disagreed
with their actual polygon incidence, despite matching boundary geometry.
The probe queries a duplicate after public
[CombineIdentical(true, true)](https://mcneel.github.io/rhinocommon-api-docs/api/RhinoCommon/html/M_Rhino_Geometry_Collections_MeshVertexList_CombineIdentical.htm),
matching native exact-location topology. It first requires every polygon's
ordered coordinates to remain exactly unchanged. No vertex movement, face
removal, or tolerance relaxation is used. All owned geometry and meshing
parameters are disposed on success and failure.

All five fresh comparisons match exactly for the recorded quantities (checked
at absolute `1e-9`, relative `1e-12`). Illustrative native release meshing times
are tens to hundreds of microseconds on these small planar fixtures. Timings
exclude source construction, command checks, sampling, and coordinate-topology
normalization; different mesh layouts and Wine/FEX translation prevent a
general native-Rhino performance-parity claim.

```sh
tools/rhino_oracle/run_headless.sh compare \
  tools/rhino_oracle/fixtures/brep_mesh_boundaries.json \
  --absolute-epsilon 1e-9 --relative-epsilon 1e-12
```

The checkpoint passes 1,325 Rust tests, 26 Python tests, and strict Clippy.
All 119 native fixtures (1,202 operations) execute; recorded curve and morph
comparisons retain their documented epsilons. This is bounded evidence, not
complete Rhino meshing compatibility or a continuous geometric certificate.
