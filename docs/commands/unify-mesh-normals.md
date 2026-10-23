# UnifyMeshNormals

[Meshes and SubDs](meshes-index.md) · [Direction and topology](mesh-topology-commands.md)

`UnifyMeshNormals` repairs inconsistent triangle and quad winding across exact
coincident locations, including unwelded seams. Preselect meshes, or start the
command, select meshes and press Enter. Cancel abandons command-first selection.
Non-mesh selections are skipped. There are no numeric options.

Changed meshes retain raw vertices, face order, unused vertices, UV coordinates,
colors, identity, attributes and groups. The command rebuilds stored normals
through public OpenNURBS and aligns n-gon boundary cycles with their member faces.
Changed objects move to the table end and share one Undo step. Preselection
survives acceptance and Undo; command-first picks and Redo selection are released.
Unchanged peers release selection on history replay. An entirely consistent
selection retains current picks and creates no Undo step, so Undo affects the
preceding edit.

`TriangleMesh::unified_normals` exposes the distinct SDK behavior: preserve n-gon
boundary cycles and negate stored normals on each vertex referenced by flipped
faces once, even when several flipped faces share it. `normal_unification_count`
counts face edits without copying geometry or channels. The command uses
`unified_normals_for_command` to rebuild normals without removing unused vertices.
The Python oracle's triangle `mesh_unify_normals` operation uses the SDK path.

Nonmanifold edges constrain their other incident faces relative to the first
incident face. Contradictory cycles receive a best-effort spanning orientation;
the operation does not guarantee global consistency or outward normals. The
separate strict `unified_face_orientations` operation used by constructors
continues to skip nonmanifold edges and reject nonorientable manifold constraints.

The [qualification record](../unify-normals-qualification.json) covers 55 SDK
inputs and 27 native command workflows: 25 accepted command replays and 27
application getters, including cancellation. Fifty-four SDK inputs match exactly.
One vertex-renumbered two-triangle holdout chooses the opposite consistent
component winding locally; its zero positional error does not qualify winding
parity. Arbitrary seed ordering, additional contradictory meshes, invalid n-gon
repair, cached face normals, restricted objects, grips, physical native
postselection and comparative performance remain unqualified.

[Rhino command reference](https://docs.mcneel.com/rhino/8/help/en-us/commands/unifymeshnormals.htm)
· [Public SDK overload](https://mcneel.github.io/rhinocommon-api-docs/api/RhinoCommon/html/M_Rhino_Geometry_Mesh_UnifyNormals_1.htm)
