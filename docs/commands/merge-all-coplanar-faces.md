# MergeAllCoplanarFaces

Meshes merge through OpenNURBS in every build. Enable `--features native-smlib`
to also merge adjacent planar B-rep faces through usd-brep. Select meshes,
surfaces or polysurfaces and run `MergeAllCoplanarFaces`, or
start the command, select objects and press Enter. Cancel/Esc releases the getter;
errors retain picked objects for retry. The command has no options and uses the
document distance and angular tolerances. A single surface is unchanged.

The native topology remover retains the existing outer and inner trims.
Merged supports can retain unused control geometry beyond those trims; the
oracle records the native tight boundary bounds. Holes,
disconnected regions, rational closed interfaces and closed solids are covered
by regressions. Curved interfaces, seams and surviving boundary vertices are
protected. Internal interface vertices can disappear. Modeling runs on private,
localized copies; every result stages before any document replacement. IDs,
attributes, groups and selection survive, with one Undo/Redo entry for the batch.
A failure or no-op preserves Redo. Prepared commits reject changed sources or
tolerances.

Planarity admission checks the complete positive-weight control hull against a
native best-fit plane. Each group stays within one fixed anchor plane and the
angular tolerance, avoiding chained plane drift. This is conservative for
trimmed faces whose unused support is nonplanar. Native coincidence and topology
checks can also decline a merge. Inputs are limited to 362 faces and two million
surface controls per B-rep; existing transfer/intersection limits also apply.
Intersections are checked before and after merging, including open manifold
sheets. These checks do not constitute a general validity certificate for every
possible CAD input.

Mesh regions use the native topology and face-set merger. Two triangles may
become a quad; larger regions use n-gons over retained facets. N-gons can contain
holes; inner boundary edges stay visible and mesh-to-NURBS conversion retains
those holes. Native welding joins exact-location unwelded seams inside merged
regions and may compact vertices; it can choose one vertex color at a welded
seam. On already welded meshes, colors and interior vertices are retained.
Existing n-gons are treated as complete units. Face normals are computed in
double precision; every group must satisfy one fixed seed plane and angular
cutoff. Nearby edges without shared topology are not currently sewn across a
gap. Mesh limits are four million vertices, one million faces and 32 million
charged face checks. Degenerate faces fail before document replacement.

`AddNgonsToMesh` now calls OpenNURBS' planar n-gon finder. It retains its separate
single-boundary policy and does not apply the merge command's angular cutoff.
The shared native bridge lives in `viboceros-opennurbs`, separate from file I/O.

SubD merging, the selected-face `MergeCoplanarFace` command, component face
selection, and full Rhino command parity remain unimplemented. Portable
builds report that multi-face merging requires the native kernel. The public
SDK comparison recipes use `Brep.MergeCoplanarFaces(distance, angle)` and
`Mesh.MergeAllCoplanarFaces(distance, angle)` on owned copies per iteration. Fresh licensed Rhino captures remain pending the recorded
[Wine startup blocker](../oracle-sspi.md); mocks and local analytic checks do not
establish Rhino parity or comparative performance.

The [B-rep qualification](../coplanar-faces-qualification.json) and
[mesh qualification](../coplanar-mesh-qualification.json) record source
hashes, observations and validation. Kernel repairs exclude topology-zone
inflation from the planarity test and honor protected topology before mutable
coincidence queries.

References: [Rhino command](https://docs.mcneel.com/rhino/8/help/en-us/commands/mergecoplanarface.htm),
[public B-rep API](https://mcneel.github.io/rhinocommon-api-docs/api/RhinoCommon/html/Overload_Rhino_Geometry_Brep_MergeCoplanarFaces.htm).

Mesh SDK reference: [MergeAllCoplanarFaces](https://mcneel.github.io/rhinocommon-api-docs/api/RhinoCommon/html/M_Rhino_Geometry_Mesh_MergeAllCoplanarFaces_1.htm).
