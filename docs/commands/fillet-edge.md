# FilletEdge

`FilletEdge` rounds selected edges of closed B-reps with a constant circular
rolling-ball radius using usd-brep.

```text
FilletEdge Radius=1
FilletEdge 1 object-id 0,3,5
FilletEdge Radius=1 All=Yes
```

The first form starts viewport edge picking. Select edges, enter `Radius=value`
to change the constant radius, then press Enter to apply or Esc to cancel.
Component preselection is retained for confirmation. The indexed form accepts
multiple object/edge-list pairs; repeated edge indices are applied once.
`All=Yes` uses edges shared by distinct faces of selected B-reps and excludes
same-face seams. Hidden or locked objects, stale geometry, changed tolerance,
invalid indices and unsupported inputs fail before document replacement.

Type `Preview` to toggle the pending result's wireframe. `Preview=Yes|No` also
works when starting the interactive command. The preview updates as picks,
radius or corner treatment change. Enter applies the matching staged result.
Use `FromTwoPoints` to set the radius with two viewport picks or typed
coordinates. With preview enabled, the second point updates the wireframe.
Use `FaceEdges` to add the sharp boundary edges of one or more faces. Enter
keeps those picks and returns to edge selection. Esc restores the prior edge
set. Tangent edges and same-face seams are excluded.
Use `ChainEdges` to select connected edges on closed polysurfaces. Set
`ChainContinuity`, `Direction`, `GapTolerance` or `AngleTolerance` as in
[connected-edge selection](curve-chains.md). `Next`, `All` and `Undo` edit the
chain. At a fork, pick a branch or press Enter to confirm the highlighted path.
Enter at the chain prompt adds the chain to the fillet selection. Esc cancels
the fillet. Double-click an edge at the main prompt to select its tangent chain.

The operation stages every result, preserves object identity, attributes and
groups, and records one undo step. All inputs remain unchanged if any stage
fails. The adapter imports fresh native solids with original Rust edge-index
correspondence; it never guesses native edge numbers from topology enumeration.
It tightens circular cross-section generation and validates closed input and
output, boundary face intersections and unexpected contacts before conversion.
Pole connectors require a continuous surface-image certificate at their vertex.

This checkpoint supports one constant radius, closed B-reps and Linux native
builds. Variable-radius handles, open polysurfaces, alternative
rail types, editing old fillets and broader failure cases
remain work. The intersection guard bounds input and output to 362 faces. Kernel
intersection curves must lie entirely on shared trimmed topological edges, and
isolated contacts must lie on shared edges or vertices. Adjacent faces are
checked too, with supplementary kernel searches for interior branches. The
check does not certify individual-surface injectivity or exhaustive intersection
discovery for arbitrary surfaces. Cases outside the qualified fixtures remain
unproven.

[Rhino's command reference](https://docs.mcneel.com/rhino/8/help/en-us/commands/filletedge.htm)
documents the full handle and rail workflow. The independent fixture capture uses
the public [`Brep.CreateFilletEdges` API](https://mcneel.github.io/rhinocommon-api-docs/api/RhinoCommon/html/M_Rhino_Geometry_Brep_CreateFilletEdges.htm)
under private Xvfb. See [implementation and qualification](../fillet-edge.md).
