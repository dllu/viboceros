# Architecture and implementation status

[Project overview](../README.md)

The dependency direction runs from mathematical primitives through document
state and commands to the user interface. Geometry does not depend on the UI,
file formats, or command parsing.

| Module | Responsibility |
| --- | --- |
| `viboceros-geometry` | Validated primitives, NURBS, intersections, B-rep topology, tessellation, and mass properties; nalgebra and faer provide linear algebra. |
| `viboceros-document` | Objects, attributes, layers, groups, selection, transactions, and bounded undo/redo. |
| `viboceros-drafting` | Snapping and tracking calculations. |
| `viboceros-io` | STL, OpenNURBS 3DM bindings, and initial STEP interchange. |
| `viboceros-command` | Command registration, argument parsing, and document operations. |
| `src/` | egui application and wgpu viewport rendering. |
| `viboceros-oracle`, `tools/rhino_oracle/` | Matching native and public Rhino API probes and a Python comparison client. |
| `third_party/` | Pinned OpenNURBS source and its license. |

## Current foundation

The current foundation supports finite 3D points, vectors, line segments,
analytic circles, circular arcs, and ellipses, validated open and closed
polylines, planes, bounding boxes, rational NURBS curves with analytic first
and second derivatives and exact knot refinement, splitting, and interval trimming,
rational NURBS surfaces with analytic partial derivatives and exact tensor
splitting, knot refinement, and rectangular domain trimming,
validated shared-topology B-reps with exact rational parameter-space trims,
validated mixed triangle/quad polygon meshes, layers, groups, and bounded
undo/redo.
Native point clouds preserve point order and duplicates, cache finite bounds,
and use a balanced XY spatial index for snapping and picking.

Implementation is incomplete. Exact polycurves, general surface/surface
intersections, arbitrary trimmed-surface mass properties, editable STEP B-reps,
and much of Rhino's command set remain to be implemented. See the
[command reference](commands/README.md) and [file-format documentation](file-formats.md)
for capability boundaries. Rhino Render is outside the project scope.

Tests cover numerical operations, topology, document state, commands, import/export,
and UI interactions. Passing the current suite is evidence for those cases,
not a proof of full Rhino compatibility. The [oracle](oracle.md) checks public
Rhino outputs and records timing independently of startup and fixture setup.
