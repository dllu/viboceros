# 3DFace

`3DFace` constructs mesh triangles, quads and n-gons from ordered corner points.
Start the command and click corners or type coordinates. Triangles finish after
three corners and quads after four; Enter accepts a three-corner quad or finishes
an n-gon. `Undo` removes the last corner. Escape clears the unfinished face;
a second Escape discards the whole draft, including completed faces. Cancellation
preserves Redo and releases an append target's selection. Boundary previews
remain separate from the model until completion.

```text
3DFace PolygonType=Quad 0,0,0 4,0,0 4,3,0 0,3,0
3DFace PolygonType=Triangle 0,0,0 4,0,0 2,3,1
3DFace PolygonType=Ngon 0,0,0 3,0,0 4,2,0 2,4,0 -1,2,0 Enter
```

Select a mesh first to append, or use the interactive `Append` picker.
`Append=<id>` is the explicit-ID scripting extension. Appending preserves the
mesh's identity, name and layer. Standalone output uses the current layer.
Results are unselected. Each command produces one Undo operation.

Options include `PolygonType=Triangle|Quad|Ngon`, `InPlane=Yes|No`,
`WeldAngle=0..180`, and `Mode=SingleFace|MultipleFaces`. InPlane projects corners
after the first three onto their plane. MultipleFaces collects successive faces
into one mesh; press Enter at an empty corner prompt to finish. WeldAngle joins
exactly coincident seams within the chosen angle, including flat seams at zero
degrees. Unrelated pre-existing unwelded seams stay separate.

With an append mesh, `FromEdge=Yes` accepts a boundary-edge click and supplies the
first two corners with coherent winding. `Edge=<index>` supplies the local
boundary index for scripts. `FromEdgeReset=Yes` resets edge mode when starting
the next command. In MultipleFaces mode, pick a boundary for each triangle, quad
or n-gon. The picker includes faces drawn during the current command.

For quads, `FromEdge=Yes Chain=Yes Mode=MultipleFaces` starts an edge chain.
Pick a boundary and two points for the first face, then one point for each
following face. Either walking direction is supported. Enter ends the current
chain and returns to boundary picking; Enter there finishes the command. Closed
walks can continue onto newly created boundaries. `Undo` can remove the reused
point of the pending face while leaving the completed face in place.

`Edge=<index>` can also appear between point sequences to choose another boundary
in a script. Its index refers to the mesh including completed draft faces.
A standalone MultipleFaces draft can switch to FromEdge mode before admission.
SubD output remains unsupported.

The [qualification report](../face-create-qualification.json) retains 48 native
recipes: the original 25 mesh constructions, five physical edge/Escape controls,
and 18 additional edge and chain scenes. The original cases and 15 of the 18
new scenes match geometry, raw topology counts, attributes and history within
`1e-6`. Viewport tests exercise pointer press/release events and projected boundary
clicks against session-owned edges without temporary document objects.

Three collinear or overlapping cases (`chain_wrap`, `edge_collinear_inner`,
`chain_wrap_shift`) remain unqualified geometry or topology comparisons. Rhino's
public `IsValidWithLog` accepts records reconstructed from all 18 captured draft meshes; these three cases
are local compatibility gaps. Native raw face ordering, stored normal arrays,
arbitrary nonmanifold neighborhoods and comparative speed remain unqualified.
Quad surface samples use the same shortest-diagonal split on both sides,
omitting zero-area reference facets. Construction retains at most 100,000
corners, including automatically reused chain corners; ambiguous boundary
branches are rejected.

Reference: [McNeel 3DFace command](https://docs.mcneel.com/rhino/8/help/en-us/commands/3dface.htm).
