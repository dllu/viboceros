# Mesh and SubD edge-loop selection

`SelEdgeLoop` immediately expands preselected mesh/SubD edges. With no eligible
preselection, pick seed edges: seeds are bright and the loop preview is dimmer.
Enter commits all previewed loops; Escape clears the newly picked seeds. Ctrl
removes a picked seed, while Ctrl on another edge of its preview has no effect.
Multiple seeds may share a loop. Whole-object preselection remains selected and
its edges are ignored by the getter. Other preselected faces and vertices survive
successful edge expansion. With no eligible preselected edges, or an empty
expansion, component preselection is cleared.
Selection does not add an Undo record or consume redo.

Headless commands use the same component-selection backend as the UI. Structured
selectors also support `SelEdgeLoop Edges=0,1 Object=uuid` and `Edges=All`; these
are Viboceros scripting extensions with zero-based component ordinals. The
result feeds `ToNURBS` directly. `SelNone` clears component selection. Structured
selector failures preserve existing selection; stale getters require picking again.

## Double-click selection

Ctrl+Shift-double-click an edge to select its loop. The same gesture removes a
selected connected run. Unselected gaps separate runs, so removing one fragment
leaves another fragment selected. Selection remains outside model history and
does not consume redo.

Existing selected edges bound additions. Two anchors select the intervening
range containing the clicked edge. One anchor on an open path selects from that
anchor through the clicked edge to the path end. Closed paths support bounded
ranges and wrap-around fragments. Other selected components remain selected.

Loops follow ordinary topology: boundary edges continue through valence-three
vertices and interior edges through valence-four vertices. Boundary vertices
of valence two,
extraordinary vertices, ambiguous continuations and nonmanifold branches stop
the walk. SubD continuation also retains the smooth/crease edge tag; a tagged
vertex alone does not stop it. Display density, smooth/flat appearance and model
tolerance do not choose the path. Closed loops terminate without repeated edges.

The gesture works in wireframe, shaded and ghosted views, both before a command
and during [ToNURBS](to-nurbs-edges.md) selection. Its first click retains a
selection witness so the second click can resolve addition/removal correctly.
Changed sources reject the gesture. Overlapping eligible edges use the existing
numbered ambiguity prompt. ToNURBS confirmation freezes the component selection.

The geometry API exposes `TriangleMesh::try_edge_loop` and
`SubD::try_edge_loop`. SubD uses the public OpenNURBS chain-neighbor query on a
private decoded source, with explicit continuation constraints. Mesh traversal
builds exact-location topology once and visits each path edge at most once.
Texture UV coordinates are not required in the measured mesh cases.

## Qualification

Qualification includes 28 actual Rhino `SelEdgeLoop` command workflows on open
grids, boundaries, creases, corner tags, extraordinary fans, triangles and cyclic
annuli. Rust imports every captured 3DM source and matches the complete component
sets. Another 64 native gesture controls establish edge-filter modifier behavior,
Shift additions/ranges, Ctrl removal, and descending ToNURBS component output
order, including ascending input anchors. Native edge filters remove face/edge
ambiguity for these controls; default unfiltered native deselection is not
qualified by them. Failed ambiguous captures are not qualification evidence.

In-memory viewport events and UI replays check selection sets, output curves and
ordering, undo/redo, stale sources and ambiguity handling without opening windows.
Native captures use private Xvfb. A further [44 standalone workflows](../oracle/edge-loop-workflows.md)
qualify seed accumulation/removal, preselection, acceptance, cancellation and
selection through redo/undo. Independent Rust grid recipes reproduce every
captured state; UI replays also check the resulting ToNURBS handoff.

See [SelEdgeRing](component-edge-rings.md) for opposite-edge selection and
[SelFaceLoop](component-face-loops.md) for crossed face strips. Mixed-preselection
rules are qualified by [additional native controls](../oracle/face-loops.md).
For standalone curve and surface tangent chains, use [SelChain](curve-chains.md).
Vertex chains and nested `SelEdgeLoop` inside another command remain separate work.
