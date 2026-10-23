# UnifyMeshNormals

[Meshes and SubDs](meshes-index.md) · [Direction and topology](mesh-topology-commands.md)

`UnifyMeshNormals` repairs inconsistent triangle and quad winding across exact
coincident locations, including unwelded seams. Preselect meshes, or start the
command, select meshes and press Enter. Cancel abandons command-first selection.
Non-mesh selections are skipped. There are no numeric options.

Changed meshes retain raw vertices, face order, unused vertices, UV coordinates,
colors, identity, attributes and groups. The command rebuilds stored normals
through public OpenNURBS and aligns n-gon boundary cycles with their member faces.
N-gon starting vertices and member-face order are preserved; boundary repair
uses OpenNURBS `OrientNgons` rather than rebuilding the overlays.
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

Traversal visits a face's closing side before its other sides and shares an
initial orientation choice across disconnected regions. Nonmanifold constraints
follow the first incident face. Contradictory cycles receive a best-effort
spanning orientation; the operation does not guarantee global consistency or
outward normals. Repeated SDK calls are not always idempotent: some isolated
faces can flip again. Freshly rebuilt meshes reproduce that observed behavior.
The separate strict `unified_face_orientations` operation used by constructors
continues to skip nonmanifold edges and reject nonorientable manifold constraints.

The [qualification record](../unify-normals-qualification.json) covers 1,180 SDK
workflows and 65 native command workflows: 63 accepted registered commands and
65 application getters, including cancellation. Controls cover vertex and cyclic
origin permutations, seams, quads, unused aliases, consistent/inconsistent region
combinations, isolated faces, closed tetrahedra, nonmanifold fans and twisted strips.
Of these, 1,169 SDK workflows match exactly. The earlier six three-region and
two-triangle renumbering differences are resolved. Recorded face-index edits
independently verify counts, raw winding, normal sign changes and repeat calls.

Eleven workflows remain unqualified: nine secondary closed, nonmanifold or twisted
component inputs and two diagnostic probes for those inputs. Zero positional
error does not qualify their winding parity. Broader seed ordering, invalid n-gon
repair, cached face normals, restricted objects, grips, physical native
postselection and comparative performance remain unqualified.

[Rhino command reference](https://docs.mcneel.com/rhino/8/help/en-us/commands/unifymeshnormals.htm)
· [Public SDK overload](https://mcneel.github.io/rhinocommon-api-docs/api/RhinoCommon/html/M_Rhino_Geometry_Mesh_UnifyNormals_1.htm)
· [Command normal reconstruction](https://developer.rhino3d.com/guides/cpp/unifying-mesh-normals/)
