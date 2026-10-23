# RebuildMesh

[Meshes and SubDs](meshes-index.md) · [Mesh editing](mesh-editing.md)

`RebuildMesh` reconstructs mesh normals and removes n-gon overlays and unused
vertices. Referenced double-precision positions, face winding and coincident raw
seams survive. UV coordinates and vertex colors are discarded by default;
preserved channels follow the source-ordered vertex compaction.

Start the command with no meshes selected to change `PreserveTextureCoordinates`
and `PreserveVertexColors`, select meshes, then press Enter. Both accept Yes/No;
entering an option name alone toggles it. Accepted values persist across
documents and Undo/Redo within one registry. Cancel discards staged values and
selection without changing geometry or history.

Preselected invocation executes immediately with accepted defaults. In native
Rhino, option tokens sent afterward become a separate command. Viboceros also
supports complete script invocations with explicit preservation arguments:

```text
RebuildMesh PreserveTextureCoordinates=Yes PreserveVertexColors=No
```

The geometry API is `TriangleMesh::rebuilt(preserve_texture_coordinates,
preserve_vertex_colors)`. It uses the existing compaction and OpenNURBS normal
reconstruction paths and leaves its source unchanged. All document replacements
are staged before mutation, retaining IDs, attributes and groups. Processed
meshes move to the object table end and record Undo even for unchanged geometry.
Preselection survives acceptance and Undo; command-first picks and Redo selection
are released. Non-mesh selections are skipped.

The [qualification record](../rebuild-mesh-qualification.json) covers 15 public
SDK inputs and 32 supported native workflows, including all four preservation
combinations, retained defaults, bare toggles, cancellation, raw channels and
document lifecycle. One rejected late-option discovery remains excluded.

Surface curvatures, surface parameters and cached face-normal storage are not
modeled. Physical native postselection, restricted objects, grips, invalid-index
repair, numeric extremes and comparative performance remain unqualified.

[Rhino command reference](https://docs.mcneel.com/rhino/8/help/en-us/commands/rebuildmesh.htm)
