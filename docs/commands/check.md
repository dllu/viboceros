# Check and SelBadObjects

`Check` reports the structural validity of selected objects. Start the command,
select objects and press Enter, or use a preselection. Results and native
OpenNURBS diagnostics appear in the command history. The command leaves geometry,
attributes, groups, selection, Undo and Redo unchanged. Esc cancels object picking.

`SelBadObjects` adds invalid, selectable model objects to the current selection.
It selects each match directly, without expanding groups. Hidden or locked
objects and objects on hidden or locked layers are skipped. Selection does not
create a geometry Undo entry or discard Redo.

Both commands accept no arguments and work in every build. Checks use private
OpenNURBS copies of the supported points, point clouds, analytic/NURBS curves,
polylines, polycurves, surfaces, B-reps and polygon meshes. The existing 3DM
constructors supply their exact native representations; no file is written.
Instance references check their actual definition UUID and placement. They do
not recursively classify definition members as bad instances. The document
separately validates its block catalog and reference graph.

Each result is **valid**, **invalid**, or **not checked**. Conversion failures,
unrepresentable native data and payloads over 256 MiB are reported as not checked;
they are never silently treated as valid or selected as bad. The payload limit
bounds native transfer after Rust payload preparation. Native diagnostics retain
up to 8 KiB of text, with a truncation marker. The combined `Check` report retains
up to 128 KiB of per-object reports, counts every result and reports omitted
entries; select fewer objects to see those details.

These are data-structure checks, like Rhino's `Check`. A passing result does not
prove absence of folded/self-intersecting surfaces, duplicate mesh faces or
nonmanifold topology. For example, coincident corners can make a stored mesh
face invalid, while distinct collinear corners can pass despite zero area.
Normal-dependent modeling commands may reject the latter.
[CullDegenerateMeshFaces](cull-degenerate-mesh-faces.md) removes degenerate mesh
faces and repairs collapsed quads. Use the existing
[edge diagnostics](show-edges.md) and mesh tools for further inspection.

The [qualification record](../geometry-check-qualification.json) links actual
command observations, regression tests and sanitizer evidence. The public SDK
probe calls `Duplicate` and `IsValidWithLog` on owned copies. Its stored-mesh
recipes preserve double coordinates and collapsed positions instead of repairing
or rejecting them before the check. Fresh licensed Rhino command/getter captures
remain pending the recorded [Wine startup blocker](../oracle-sspi.md). Neither
full Rhino parity nor comparative performance is established.

References: [Rhino Check](https://docs.mcneel.com/rhino/8/help/en-us/commands/check.htm),
[GeometryBase](https://developer.rhino3d.com/api/rhinocommon/rhino.geometry.geometrybase?version=8.x).
