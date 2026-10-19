# SoftEditSrf

Select one surface and run `SoftEditSrf`. Pick a base point, then an offset
point. Base picks respect visible trims before snapping to the underlying
surface's edit grid. Repeat base/offset pairs; Enter finishes. Escape discards
the unfinished offset and keeps accepted edits. One Undo restores the command.

`U_Distance` and `V_Distance` accept positive numbers, model units and calculator
expressions. `FixEdges=Yes` pins boundary controls, including clamped seams.
`DirConstraint` offers `None`, `SrfNormal` (the initial default), `UTangent`, and
`VTangent`. Directions are evaluated at the projected pick before edit-grid
snapping. Mouse offsets use the existing view-line constraint solver; typed
points use the same axis projection. `Copy=Yes` creates each result from the
current source. Copy can change between accepted offsets.

The command API accepts complete input:

```text
SoftEditSrf 2,3,0 2,3,2 U_Distance=4 V_Distance=4 FixEdges=No Copy=No DirConstraint=None
```

This increment supports an open V direction on NURBS surfaces and single-face
B-reps. It includes trimmed surfaces, periodic U, ordinary cylinders and spheres,
rational weights, degree elevation, reversed charts and changed domains. Closed-V
surfaces, including ordinary tori and transposed spheres, are explicitly rejected
without changing the model. Their native captures are retained as deferred
parity evidence. Multiple-face and face-subobject selection remain unsupported.

Displacements use Gaussian falloff with standard deviation one third of each
distance, measured by precise curve arc lengths. V rows are edited in order;
each U isocurve uses the evolving surface. The result is nonlinear in the offset,
so destination previews recompute these U metrics. Collapsed pole rows retain a
single physical point. Degree, knots and raw rational weights are retained.

B-rep edits reuse the existing boundary reconstruction: UV trims and face
orientation are retained, rectangular edges are exact isocurves, and curved
trim images use bounded fits with continuous certificates. Source identity,
attributes and groups survive replacement. Copies use current-layer attributes
without groups. Trimmed B-rep geometry text is retained. Preselection survives;
postselection is released. Unaccepted previews preserve model and redo history.

Validation compares accepted geometry definitions, 81 surface samples, boundary
endpoint multisets, topology, metadata, selection and Undo/Redo against public
Rhino 8 commands. Vertex and edge numbering may change during reconstruction.
Native default length integration introduces small rounding differences; single
edits use `5e-8` and repeated edits use `1e-7` model-unit bounds. The exact counts
and maximum observed differences are recorded in the validation receipt.
Arbitrary large nets, extreme coordinate ranges, FixEdges with periodic U, mixed-sign rational weights and
cross-application performance remain unqualified.

```sh
cargo test -p viboceros-geometry --lib soft_edit
cargo test -p viboceros-command --test soft_edit_surface_command --config 'profile.test.package.viboceros-geometry.opt-level=2'
cargo test --bin viboceros tests::soft_edit_surface --config 'profile.test.package.viboceros-geometry.opt-level=2'
python3 -m unittest tools.rhino_oracle.test_soft_edit_surface
```

[Rhino command reference](https://docs.mcneel.com/rhino/8/help/en-us/commands/softeditsrf.htm)
· [Public SDK](https://mcneel.github.io/rhinocommon-api-docs/api/RhinoCommon/html/M_Rhino_Geometry_Surface_CreateSoftEditSurface.htm)
· [Validation and captures](../soft-edit-surface-validation.json)
