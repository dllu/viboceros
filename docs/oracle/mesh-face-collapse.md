# Mesh face collapse investigation

`CollapseMeshFace` is being implemented. It remains unregistered while its native
collapse behavior is qualified; the production geometry kernel is unchanged.
Rhino's [command reference](https://docs.mcneel.com/rhino/8/help/en-us/commands/collapsemeshface_commands.htm)
describes collapsing a selected face and adjusting its neighbors.

The [measurement report](mesh-face-collapse-validation.json) retains 104 actual
Rhino command observations across ten immutable families. These cover face
indices, triangle/quad neighborhoods, seams, disjoint parts, nonplanar geometry,
vertex colors, UVs, normals, source precision and input methods. Each observation
binds its request and producer sources to SHA-256 archives.

## What the captures establish

- Sixty-six invocations change stored positions or face connectivity; 38 return
  `Success` without changing either. Success alone cannot qualify an edit.
- Explicit Wireframe/Shaded controls use the same projected face positions and
  acknowledge the prompt, mouse movement and mouse-down before sending Enter.
  All four Wireframe cases are no-ops; all four Shaded cases edit geometry.
- All four Shaded getter outputs exactly match preselection's complete mesh
  storage: positions, face indices, colors, UVs, normals and n-gons.
- The eighteen captured Shaded edits with command diagnostics emit `Output mesh is
  invalid.` The four explicit-mode edited objects nevertheless report
  `Mesh.IsValid == True` after insertion. These are distinct observations; the
  reason for the warning remains unresolved.
- Independent closed cube, octahedron and tetrahedron inputs reproduce the warning;
  it is not confined to the open planar neighborhoods. Decimal-position
  holdouts retain both position buffers rather than assuming exact float storage.
- The sixteen precision cases have matching float/double position buffers.
  Both authored precision settings become double-precision document objects;
  these are not evidence of two distinct stored document representations.
- Comparing a source after Undo does not establish an undoable edit when the
  command did nothing. The evidence helper requires a geometry change before
  reporting verified Undo/Redo. It separately records missing diagnostics.
- Early families did not clear earlier cases' history. Their immutable stored
  geometry observations remain useful, but their Undo equality flags alone
  cannot qualify history. The current probe clears Undo/Redo before each case.
  Four alternating changed/no-op controls bind that isolation to the frozen
  producer and verify source restoration and Redo without counting no-ops.

These measurements are **not Rust compatibility passes**. The current clean-edit
reference check admits none: the outcomes are no-ops, have incomplete validity/
diagnostic evidence, or carry the native invalid-mesh warning. The next step is
to explain that warning and establish a valid collapse target before admitting
the independent kernel operation. Required geometry comparisons retain epsilon
`1e-7`; no existing fitting gate was changed or rerun.

## Reproduction

```sh
python3 -m unittest tools.rhino_oracle.test_mesh_face_collapse
python3 -m tools.rhino_oracle.mesh_face_collapse_evidence \
  mesh_face_collapse mesh_face_collapse_modes
```

Live generators run through `tools/rhino_oracle/run_headless.sh exec python3 -m`
with their `tools.rhino_oracle.generate_mesh_face_collapse*` module names.
Successful captures refuse overwrite. Live requests require an owned private
settings scheme, one iteration and a bounded homogeneous operation batch.
The managed getter reuses the existing public SDK input driver. No proprietary
implementation was inspected. A manually assisted diagnostic trial is archived
outside the source checkout and excluded from these 104 observations.
