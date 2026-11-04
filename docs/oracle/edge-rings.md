# Edge-ring selection evidence

[Command behavior](../commands/component-edge-rings.md) · [Oracle overview](../oracle.md)

Three immutable families retain complete native requests, raw compressed
responses and content-addressed producer snapshots:

- `edge_ring_discovery`: two initial mesh/SubD grid controls.
- `edge_ring_commands`: 75 topology and selection-lifecycle controls, including
  boundaries, creases, extraordinary vertices, triangles, cycles, intersecting
  rings, multiple seeds, whole/component preselection and cancellation. Mesh
  controls include nonmanifold branches and unwelded raw vertices.
- `edge_ring_ngons`: eight mesh/SubD controls with logical n-gons over triangles
  and quads, including converted collinear boundaries and sparse native IDs.

Each capture records before/after selection, every seed action, source CRC and
a seeded Redo/Undo cycle. All 85 sources remain unchanged and redo survives.
A collapsed mesh was rejected during source insertion; the failed run published
no observation and is not qualification evidence.

Rust constructs its own polygon recipes, invokes the shared command session and
compares complete states, including native component IDs. Separate tests import
43 single-seed native archives and compare geometry-level traversal directly.
App replays check all 85 workflows and ToNURBS handoff. Egui pointer events check
ordinary and n-gon sources in wireframe, shaded and ghosted modes. A 6,000-seed
triangle batch exercises topology reuse and preservation of pending redo.

The Python API exposes `edge_ring_workflow`. Use the saved-state comparison
example in [loop workflows](edge-loop-workflows.md) with
`family = "edge_ring_commands"` or `"edge_ring_ngons"`; it does not launch Rhino.
The Rust source recipes use no captured topology or output as input.

This evidence concerns component selection. Existing ToNURBS curve-image
qualification remains separate. Nested getters, rings selected by double-click,
wire SubDs and invalid imported topology are not qualified by these captures.
