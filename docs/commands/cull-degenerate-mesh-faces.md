# CullDegenerateMeshFaces

Select meshes and run `CullDegenerateMeshFaces`, or run it first, select meshes,
and press Enter. Esc cancels picking. The command accepts no arguments and works
in every build.

The command uses OpenNURBS to remove degenerate faces and repair collapsed quads
as triangles where possible. It reports face removals and quad repairs separately:
a repaired quad can leave the face count unchanged. It removes all unused vertex
occurrences, including ones that were already unused. Equal positions remain
unwelded, and surviving vertex colors follow their original occurrences.

Classification uses double coordinates and fixed native criteria, independently
of document tolerance. The pinned kernel includes a `1e-36` cross-product-length
floor and a `1e-8` relative diagonal test for quads; sufficiently small nonzero
faces can therefore be removed. This is not a general mesh validity certificate
or a tolerance-based simplifier.

Surviving objects retain their IDs, attributes, groups and selection. Meshes with
no remaining faces are deleted. All selected meshes are prepared before one
document transaction, so a failure leaves every source unchanged. Undo restores
both replacements and deleted objects; Redo repeats the cleanup. A no-op keeps
Redo available. Prepared cleanup rejects stale source geometry.

N-gons are face overlays. Native cleanup can drop overlays as their faces or
vertices disappear. A quad repair can also leave a stale outer boundary in the
pinned kernel; the adapter rebuilds such boundaries with OpenNURBS' boundary
routines. The existing Rust n-gon validator retains valid connected groups and
dissolves groups that no longer satisfy its topology rules, preserving their
remaining facets. An unaffected annular n-gon retains its hole, including through
`MeshToNurb`.

Use [Check and SelBadObjects](check.md) to inspect structural validity before or
after cleanup. Collinear faces can pass structural `Check` despite zero area;
cleanup can still remove them.

The [qualification record](../cull-degenerate-mesh-faces-qualification.json)
records native regressions, command/UI history tests, oracle recipes and sanitizer
evidence. The SDK probe uses owned copies and explicitly calls
`Faces.CullDegenerateFaces()` followed by `Vertices.CullUnused()`. It observes
actual double-coordinate records, counts and area, including empty results; it
does not emulate Rhino's document deletion or getter behavior. No licensed
Rhino capture was obtained for this increment. Command/getter parity and
comparative performance remain unqualified; the latest recorded launch issue
is the [Wine startup blocker](../oracle-sspi.md).

References: [Rhino command](https://docs.mcneel.com/rhino/8/help/en-us/commands/culldegeneratemeshfaces.htm),
[RhinoCommon face methods](https://developer.rhino3d.com/api/RhinoCommon/html/Methods_T_Rhino_Geometry_Collections_MeshFaceList.htm).
