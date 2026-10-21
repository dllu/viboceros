# 3DFace

`3DFace` constructs mesh or SubD triangles, quads and n-gons from ordered corner points.
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

Select a mesh or SubD first to append, or use the interactive `Append` picker.
`Append=<id>` is the explicit-ID scripting extension. Appending preserves the
source's identity, name and layer. Standalone output uses the current layer.
Results are unselected. Each command produces one Undo operation.

Options include `PolygonType=Triangle|Quad|Ngon`, `InPlane=Yes|No`,
`WeldAngle=0..180`, and `Mode=SingleFace|MultipleFaces`. InPlane projects corners
after the first three onto their plane. MultipleFaces collects successive faces
into one mesh; press Enter at an empty corner prompt to finish. WeldAngle joins
exactly coincident seams within the chosen angle, including flat seams at zero
degrees. Unrelated pre-existing unwelded seams stay separate.

With an append mesh or SubD, `FromEdge=Yes` accepts a boundary-edge click and supplies the
first two corners with coherent winding. `Edge=<index>` supplies the local
boundary index for scripts. `FromEdgeReset=Yes` resets edge mode when starting
the next command. In MultipleFaces mode, pick a boundary for each triangle, quad
or n-gon. The picker includes faces drawn during the current command.

For quads, `FromEdge=Yes Chain=Yes Mode=MultipleFaces` starts an edge chain.
Pick a boundary and two points for the first face, then one point for each
following face. Either walking direction is supported. Enter ends the current
chain and returns to boundary picking; Enter there finishes the command. Closed
walks can continue onto newly created boundaries. `Undo` removes the reused point and then the seeded edge corners of the
pending face, leaving completed faces in place. After removing an edge corner,
remaining points can form a fresh face.

`Edge=<index>` can also appear between point sequences to choose another boundary
in a script. Its index refers to the mesh including completed draft faces.
A standalone MultipleFaces draft can switch to FromEdge mode before admission.

`Output=SubD Interpolated=No` creates or appends subdivision faces. Triangle,
quad, n-gon, InPlane, MultipleFaces and corner Undo share the point getter.
Adjacent faces share exact-coordinate vertices and smooth joined boundaries.
Detached SubD components become separate current-layer objects. Source creases
and corners away from a join remain intact; joined endpoints are retagged.
Explicit output-family switches create separate objects beside the source.
See [SubD geometry](../subd.md) for display and interchange behavior.
`Interpolated=Yes` fits SubD vertex limit stations to the picked or InPlane-projected
points. Appending keeps the source's original vertex limit stations fixed;
control points move across the joined net to satisfy those constraints. Exact
source control coordinates identify joins. The option is remembered and affects
only SubD output. Fitting runs at completion and checks native residuals against
the document absolute tolerance before applying the result.

The [qualification report](../face-create-qualification.json) retains 73 native
recipes. The command and viewport getter comparisons match sampled surfaces
within `1e-6`, vertex/face counts, cyclic quad ordering, n-gon boundary winding,
attributes and grouped history. The corner controls include collinear and
overlapping quads, near-edge side changes, reversed source faces, rotation,
scaling, tilted planes and repeated point Undo. Viewport tests exercise pointer
press/release events and projected boundary picks without temporary document
objects.

Quad records retain collapsed facets and zero polygon normals. Mesh projection
uses the existing exact triangle, segment and point queries for these records.
The quad's starting corner is preserved when its order changes, because a tied
diagonal can otherwise change the sampled surface. Boundary continuation needs
a defined reference normal; welding requests normals only for affected seams.

Stored mesh normal-array parity, arbitrary nonmanifold neighborhoods,
texture/vertex-color channels and comparative speed remain unqualified.
Construction retains at most 100,000 corners, including automatically reused
chain corners. Ambiguous source boundary loops are rejected; a nonunique
continuation returns to boundary selection.

SubD qualification is separate: [report](../subd-qualification.json), with ten
standalone recipes captured twice (control topology and density-two limit meshes).
Both command and point-getter tests compare tags, winding, surface stations,
current-layer output, selection and grouped history at `1e-6`.
[Append controls](../subd-append-qualification.json) add 32 recipes covering source
identity, detached objects, nonmanifold joins, reversed faces, late bridges,
boundary chains, output-family switches and cancellation.
[Interpolation controls](../subd-interpolation-qualification.json) add 36
interpolated recipes and three ordering controls, including tilted concave
append faces, source creases/corners, chained boundary picks and cancellation.

Reference: [McNeel 3DFace command](https://docs.mcneel.com/rhino/8/help/en-us/commands/3dface.htm).
