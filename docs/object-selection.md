# Command-first object selection

[`MeshToNURB`](commands/mesh-to-nurb.md) currently uses this workflow. It is not
yet a general interactive implementation of every object-taking Rhino command.

With no mesh preselected, enter `MeshToNURB`. Click meshes or drag selection
windows; ordinary clicks add picks without requiring Shift. Ctrl/Command removes
picks. Empty clicks leave picks alone. `SelAll` adds selectable meshes only;
`SelNone` clears the current picks. Hidden/locked objects and objects on hidden/
locked layers cannot be picked. A group's other members are not implicitly picked.

Type `TrimTriangularFaces=No`, `UseNgons=No`, or both in one input. Paired forms
such as `TrimTriangularFaces No` are also accepted. Invalid or duplicate options
leave the entire input editable and do not partially accept its choices.
Enter finishes once at least one mesh is picked. Escape clears picks and exits;
accepted options remain remembered without editing geometry or undo/redo history.
Starting another modeling command cancels the prompt first.

Display, grid snap, Osnap, SmartTrack, and CPlane controls preserve pending picks.
A nested CPlane prompt temporarily uses point input; cancelling it returns to
mesh picking. Undo/Redo toolbar buttons and the Delete-object shortcut are disabled
while object picking is unfinished. Successful conversion is one undoable edit.

## Separation of responsibilities

`viboceros-command::object_selection` defines the geometry filter, typed boolean
options, and a command-owned prompt description. Reading a description does not
change remembered choices. Explicit option acceptance changes command memory,
not document state. `execute_postselected` reuses the ordinary transaction and
rollback lifecycle, while allowing command-specific selection cleanup and ordering.

`app/object_selection` owns pending picks and input routing, separately from
point drafting and nested construction-plane prompts. The viewport receives a
selection filter alongside its independent drafting input. Filtering occurs before
point/curve/mesh hit priority and before projected window/crossing queries, so an
ineligible foreground point or curve cannot prevent a mesh pick. The app filters
again before mutating selection; both click and window input share that adapter.

Tests exercise real egui pointer events and app prompt transitions, including
invalid options, additive/removal picks, groups, hidden/locked objects, cancellation,
undo/redo, nested CPlane input, and transparent controls. Rhino comparisons and
private-Xvfb native GUI export checks are detailed in the command reference.
