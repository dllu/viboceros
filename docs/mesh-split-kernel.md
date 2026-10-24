# Open mesh partition kernel

The native `viboceros_smlib::mesh_split` API partitions a target mesh with open or
closed mesh cutters. It returns connected surface pieces without capping the
cuts. No interaction returns an unchanged copy; a partial cut can return one
imprinted piece. A caller applies the `DiscardUnsplitMeshes` policy before
admitting document changes.

This is kernel groundwork for `MeshSplit` and `MeshTrim`. Their command dispatch,
getters, attributes, groups, history, projected curves and infinite-plane cutters
remain to be implemented. The captured command workflows retain those behaviors
for subsequent integration. Imprinted parts return triangles; vertex channels
and logical n-gons still need transfer. Native component order remains unqualified.

Rhino defaults to `CoplanarFaces=Keep`, `CreateNgons=Yes` and
`DiscardUnsplitMeshes=Yes`. Successful splits create fresh, unselected objects
with target attributes and groups; cutters remain selected.

The adapter reuses usd-brep/SMLib triangle imprints and spatial trees, existing
mesh intersection coverage, and exact vertex compaction. Intersection constraints
are retained as geometry across successive cutters. A spatial index finds the
collinear intervals covering each final edge; traversing uncut radial edges
extracts connected pieces. Coplanar coverage boundaries avoid fragmenting shared
areas at every triangle seam. Finite-value checks, bounded output and work counts,
and owned native handles cover failure paths. Input storage remains unchanged.

The [qualification](mesh-split-kernel-qualification.json) retains 36 native command
workflows and compares 30 cases with mesh inputs: 19 default command results and 11
public `Mesh.Split` controls. Twenty-eight match at `1e-7`, checking component
count, per-component surface area, oriented area vector, signed volume about a
common origin, and bidirectional vertex/edge/triangle samples. N-gon options,
partial and disjoint intermediate results, and private SDK input preservation
are independently retained in the captures.

Two differences remain. Mixed coplanar/crossing cuts produce different component
grouping. With three crossing cutters, Rhino's command and SDK return eight
pieces totaling `50.9` square units from a `52` square-unit source; this kernel
retains the complete source surface. Extreme scales, near contacts, native face
segmentation and comparative performance require further qualification.

```sh
cargo test -p viboceros-smlib --features native --test mesh_split
python3 -m tools.rhino_oracle.report_mesh_split_kernel REPLAY_LOG
```
