# InsertPoint

[Meshes and SubDs](meshes-index.md) · [Geometry and evidence](../geometry/insert-point.md)

Select one editable mesh or SubD and run `InsertPoint`, or run the command first
and pick its source. Click original control edges to insert points. Consecutive
picks connect across faces, inserting crossing points on intermediate source
edges. The original edge cage remains available after preview chords appear.
Enabled Osnap features project onto the chosen edge; the cursor shows both the
feature and its constrained edge point when they differ.

- `Undo` removes the last accepted point and rebuilds the preview.
- Enter commits the result as one document Undo step.
- Cancel, Escape, or starting another command discards the staged points.
- Finishing empty or after undoing every point preserves geometry and redo.

Coordinates choose the nearest source edge in 3D. Scripts may use normalized
parameters on zero-based original topology edges:

```text
InsertPoint 0,1.5,0 4,1.5,0 Enter
InsertPoint Edge=1 Parameter=0.25 Edge=3 Parameter=0.75 Undo Enter
```

Object identity, attributes and groups are retained. The command clears parent
selection and moves an accepted source to the end of the object table, matching
the captured Rhino workflows. Input is limited to 256 points. Changes to source
geometry, tolerance or editability invalidate a staged preview.

Mesh vertex channels and seams survive insertion. Existing n-gon memberships
follow original face adjacency; new polygon regions with more than four boundary
vertices receive n-gon overlays. Some native triangle/quad layout choices still
differ. The [qualification record](../insert-point-command-qualification.json)
keeps those differences visible. Native perspective crossing paths, changing
views, constrained Osnap, grip behavior and comparative speed remain unqualified.
