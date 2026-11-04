# Mesh and SubD edge-loop selection

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
Native captures use private Xvfb. Edge rings, face/vertex chains, surface tangent
chains and the standalone SelEdgeLoop command are separate remaining work.
