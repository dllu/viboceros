# SoftEditSrf

Select one surface and run `SoftEditSrf`. Pick a base point, then an offset
point. Base picks respect visible trims before snapping to the underlying
surface's edit grid. Repeat base/offset pairs; Enter finishes. Escape discards
the unfinished offset and keeps accepted edits. One Undo restores the command.

`U_Distance` and `V_Distance` accept positive numbers, model units and calculator
expressions. `FixEdges=Yes` pins open boundaries and clamped seams. Periodic
directions retain their synchronized seam controls.
`DirConstraint` offers `None`, `SrfNormal` (the initial default), `UTangent`, and
`VTangent`. Directions are evaluated at the projected pick before edit-grid
snapping. Mouse offsets use the existing view-line constraint solver; typed
points use the same axis projection. `Copy=Yes` creates each result from the
current source. Copy can change between accepted offsets.

The command API accepts complete input:

```text
SoftEditSrf 2,3,0 2,3,2 U_Distance=4 V_Distance=4 FixEdges=No Copy=No DirConstraint=None
```

NURBS surfaces and single-face B-reps support open, closed and periodic charts,
including tori, transposed cylinders and spheres, poles, trimmed surfaces,
rational weights, degree elevation, reversed charts and changed domains.
Multiple-face and face-subobject selection remain unsupported.

Displacements use Gaussian falloff with standard deviation one third of each
distance, measured by precise curve arc lengths. Stored V sites are visited in
order, with their own row before any seam aliases.
Each visit measures its U isocurve on the evolving surface. The selected V
isocurve determines seam aliases anew at every accepted base pick, including
after previous edits open part of the seam. The result is nonlinear in the
offset, so destination previews recompute these U metrics. Numerically collapsed
pole rows retain a single physical point. Degree, knots and raw rational weights
are retained.

B-rep edits preserve UV trims, face orientation and natural seam/pole topology.
Rectangular spatial edges use the first incident exact isocurve. Other incident
boundary images receive control-hull deviation bounds, plus one model tolerance
for evaluation roundoff. Singular boundary images receive analogous vertex
bounds. The document tolerance stays unchanged. Curved trim images use the
existing strict reconstruction with continuous certificates.

Some native edits separate seam sides or spread a pole while retaining its
original topology. Viboceros represents these deviations through explicit
component tolerances. As in the captured native workflows, Copy declines an
object whose single-vertex spatial edge is geometrically open; that offset adds
no object or history entry. Accepted earlier offsets remain available.

Source identity, attributes and groups survive replacement. Copies use
current-layer attributes without groups. Trimmed B-rep geometry text is
retained. Preselection survives; postselection is released. Unaccepted previews
preserve model and redo history.

Validation compares accepted geometry definitions, 81 surface samples, boundary
endpoint multisets, topology, metadata, selection and Undo/Redo against public
Rhino 8 commands. Vertex and edge numbering may change during reconstruction.
Closed-chart captures additionally compare complete UV trim definitions and 17
surface/edge samples per trim, and check measured component bounds. Native
default length integration introduces small rounding differences: open-V single
edits use `5e-8`, closed-V singles use `1e-7`, and repeated edits use `1e-7`
(open charts) or `2e-7` (closed charts) model-unit bounds. The validation receipts
record exact counts and maximum differences. Arbitrary large nets, extreme
coordinate ranges, mixed-sign rational weights and cross-application performance
remain unqualified.

```sh
cargo test -p viboceros-geometry --lib soft_edit
cargo test -p viboceros-command --test soft_edit_surface_command --config 'profile.test.package.viboceros-geometry.opt-level=2'
cargo test --bin viboceros tests::soft_edit_surface --config 'profile.test.package.viboceros-geometry.opt-level=2'
python3 -m unittest tools.rhino_oracle.test_soft_edit_surface tools.rhino_oracle.test_soft_edit_surface_closed
```

[Rhino command reference](https://docs.mcneel.com/rhino/8/help/en-us/commands/softeditsrf.htm)
· [Public SDK](https://mcneel.github.io/rhinocommon-api-docs/api/RhinoCommon/html/M_Rhino_Geometry_Surface_CreateSoftEditSurface.htm)
· [Open-chart validation](../soft-edit-surface-validation.json)
· [Closed-chart validation](../soft-edit-surface-closed-validation.json)
