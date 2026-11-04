# Face-loop and mixed component-selection evidence

[Command behavior](../commands/component-face-loops.md) · [Oracle overview](../oracle.md)

Four immutable families retain requests, compressed raw Rhino responses and
content-addressed producer snapshots, captured only in private Xvfb sessions:

- `face_loop_discovery`: eight mesh/SubD entry and preselection controls.
- `face_loop_commands`: 87 topology, seed accumulation/removal, preselection,
  acceptance, cancellation and redo/undo controls. Sources include grids,
  extraordinary fans, cyclic annuli, triangles, mixed quads/triangles,
  nonmanifold branches, unwelded vertices and logical n-gons.
- `component_pattern_preselection`: 24 mixed face/vertex-plus-edge controls
  across `SelEdgeLoop`, `SelEdgeRing` and `SelFaceLoop`, on grids and triangles.
- `component_pattern_holdouts`: six partial n-gon face strips and six mixed
  nonmanifold controls. These were captured separately from the main families.

Each observation records every seed action, complete component sets, source CRC
and a seeded Redo/Undo cycle. All 131 sources remain unchanged. Getter states
contain seed edges only; accepted face-loop results include crossed faces and
contributing seeds. Empty face strips discard their seeds. Mixed controls show
that successful expansion preserves other preselected components, while empty
expansion clears all component preselection.

Independent Rust polygon recipes reproduce all 131 workflows, including sparse
SubD component IDs, without consuming captured topology or output as input.
Separate geometry tests import 51 single-seed native archives. App replays check
complete selection states and history; real egui pointer events check bright
seeds, dim face previews and accepted outlines in all three display modes,
including strips selecting only part of a logical mesh n-gon. Local handoff
regressions check exact-face subdivision and point-extraction arguments.
A 6,000-seed triangle batch exercises topology reuse for empty strips.

The Python API exposes `face_loop_workflow` and `component_pattern_workflow`.
Use the [saved-state comparison example](edge-loop-workflows.md) with any family
above; it runs the release Rust probe without launching Rhino. All 131 new
workflows and the previous 129 loop/ring workflows match through this API.

These captures qualify component selection, not downstream geometry against
Rhino. Nested getters, face double-clicks, wire SubDs and invalid imported
topology are outside this evidence. Mesh logical n-gons and converted SubD faces
can yield different strips; raw accepted face IDs are the authority.
