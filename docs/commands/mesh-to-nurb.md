# MeshToNURB

```text
MeshToNURB TrimTriangularFaces=Yes UseNgons=Yes
MeshToNURB TrimTriangularFaces=No
MeshToNURB
```

Converts triangle/quad meshes to exact degree-one NURBS B-reps. With meshes
preselected, conversion runs immediately, ignores other selected geometry,
retains source selection, and creates outputs in source document order.
With no eligible preselection, the GUI starts an [object prompt](../object-selection.md):
click or window-select meshes, type options, then press Enter. Picks are additive;
Ctrl/Command removes picks. Group membership does not expand mesh picks.
Prompted conversion clears selection and creates outputs in pick order.

Both paths preserve source identities, geometry, attributes, and memberships.
New outputs inherit source attributes (including name, color policy, and layer),
but are unselected and ungrouped. Existing empty groups are retained.

Edge-connected components become separate B-rep objects. Exact coincident edge
locations connect faces even across unwelded raw mesh vertices; a shared vertex
alone does not connect components. Component and face order follow the source.
Quads retain their potentially warped bilinear surfaces. Triangles become
rectangular patches with diagonal trims when `TrimTriangularFaces=Yes`, or
untrimmed patches with one collapsed side when `No`. This differs from
[ToNURBS](to-nurbs.md), which keeps disconnected mesh components in one object.

Choices belong to the command registry and survive undo, independently of
`ToNURBS` and the span-conversion commands. Partial updates preserve other
choices. Options entered during picking are accepted immediately, even if Escape
cancels before any mesh is picked. Cancellation clears picks and any initial
non-mesh selection, without geometry edits or changes to undo/redo history.
Invalid option input is atomic. Failed preselected commands do not accept choices;
failed prompted conversion retains choices already accepted at the prompt.
Native bootstrap choices are Yes/Yes.
Factory defaults and restart persistence are not established by these probes.
`UseNgons` is stored but does not affect current triangle/quad-only meshes.
Rhino n-gon regions and their conversion are not yet implemented or verified.

## Selection path and oracle

The headless native command requires selection; the GUI also supports command-first
picking. Native preselection accepts explicit typed options. Rhino hides options
for preselected meshes, so the oracle sets requested choices by converting a separate owned temporary mesh,
removes that mesh and its outputs, then runs the actual bare `MeshToNURB` command
on the fixture's preselection. It never rewrites result selection or attributes
to force a match. Temporary seed cleanup is tested after partial failures.

Postselection probes run the actual option prompt, select fixture IDs in order,
then finish or cancel. They do not use the preselection seed helper. An additional
private-Xvfb Rhino mouse test clicked one mesh in a group containing another mesh
and a point: only the clicked mesh was converted, matching native output exactly.
See the [Rhino reference](https://docs.mcneel.com/rhino/8/help/en-us/commands/meshtonurb.htm).

The 50 `mesh_nurbs_conversion.json` probes compare complete source records,
output NURBS surface nets, B-rep vertices/edges/trims, sampled geometry, object
order, attributes, selection, and populated/empty group tables. Coverage includes
both options, disconnected/interleaved pieces, vertex-only contacts, unwelded
seams, non-manifold edges, unused/duplicate vertices, warped quads, closed meshes,
mixed selection, partial selection, and geometrically identical sources.
Three `mesh_nurbs_conversion_sessions.json` sequences add 21 steps checking
triangle-trimming memory, partial changes, undo, independence from `ToNURBS`,
and accepting a triangle-trimming choice while converting a quad mesh.
All 53 comparisons pass at absolute `1e-8`, relative `1e-12`, with maximum
observed error `8.89e-16`. N-gon option effects cannot be inferred from these
triangle/quad-only cases.

The 60 `mesh_nurbs_postselection.json` cases cover the same geometry families,
partial/reversed picks, initial non-mesh selection, and cancellation with no picks.
Three `mesh_nurbs_postselection_sessions.json` sequences add 18 steps checking
cancelled choices, pre/postselection transitions, undo, and independent `ToNURBS`
memory. All 63 comparisons pass at the same epsilon and maximum observed error.

A private-Xvfb GUI check imports a polyline, disconnected mesh, and point with
source attributes and populated/empty groups. Six exported 3DM models verify
exact source geometry, separate output B-reps, triangle trims, attributes,
ungrouped outputs, undo/redo, and option memory independent of `ToNURBS`.
The final model was also inspected in all four ghosted viewports.
An additional six-export command-first GUI check verifies mouse picking through
an overlapping curve, Ctrl removal, crossing selection, cancellation, both triangle
choices, undo/redo, and option memory. Sources, output geometry, attributes, and
group tables match exactly. Display changes preserve the pending picks.

## Implementation and performance

The independent command module borrows source meshes, stages all outputs before
mutation, and commits one undoable operation. Aggregate output is bounded to
1,048,576 NURBS controls (four per mesh face). This also bounds output object
count; oversized batches fail without modifying sources, groups, selection,
or history. Preselected failures retain old options; prompted choices have already
been accepted separately from geometry work.

The geometry `mesh/components` module shares one vertex-remapping scratch array
across components, clearing only touched entries. This eliminates initialization
proportional to vertex count times component count, without changing raw vertex
or face order. Connectivity still uses ordered maps; this is not a claim that
the entire algorithm is linear. Tests exercise 2,048 vertex-touching components,
including a shared vertex with alternating local indices, plus mixed quads and
duplicate raw vertices. Both disjoint splitting and mesh `Explode` use the helper.

For a focused benchmark, run:

```sh
cargo run --release -p viboceros-geometry --example mesh_components
```

On the development machine, mean extraction times in five-run batches for 8,000
disconnected triangles fell from roughly 13–15 ms to 6–9 ms. Geometry construction is
outside the timed region; allocating and dropping outputs is included.
General Rhino performance parity is not established.
