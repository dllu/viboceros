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
the next command. SubD output, Chain drafting and MultipleFaces with FromEdge
are currently unsupported and return explicit errors.

The [qualification report](../face-create-qualification.json) retains 25 native
mesh recipes and five physical edge/Escape controls. Both command and point
entry match the 25 meshes within `1e-6`, including raw topology counts and n-gon
boundary winding. Native raw face ordering, stored normal arrays, arbitrary
nonmanifold append neighborhoods and comparative performance remain unqualified.
Quad surface samples use the same shortest-diagonal split on both sides.

Reference: [McNeel 3DFace command](https://docs.mcneel.com/rhino/8/help/en-us/commands/3dface.htm).
