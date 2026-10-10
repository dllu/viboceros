# FilletEdge

`FilletEdge` rounds selected edges of closed B-reps with a constant circular
rolling-ball radius. Build with `--features native-smlib` to enable the native
kernel. Default builds expose the command and report the missing backend without
changing geometry.

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

The operation stages every result, preserves object identity, attributes and
groups, and records one undo step. All inputs remain unchanged if any stage
fails. The adapter imports fresh native solids with original Rust edge-index
correspondence; it never guesses native edge numbers from topology enumeration.
It tightens circular cross-section generation and validates closed output,
nonadjacent face intersections and unexpected face contacts before conversion.
Pole connectors require a continuous surface-image certificate at their vertex.

This checkpoint supports one constant radius, closed B-reps and Linux native
builds. Variable-radius handles, dynamic preview, open polysurfaces, alternative
rail types, chained selection, editing old fillets and broader failure cases
remain work. The intersection guard bounds output to 362 faces and does not
fully certify individual-surface self-intersections or additional intersections
between adjacent faces. Cases outside the qualified fixtures remain unproven.

[Rhino's command reference](https://docs.mcneel.com/rhino/8/help/en-us/commands/filletedge.htm)
documents the full handle and rail workflow. The independent fixture capture uses
the public [`Brep.CreateFilletEdges` API](https://mcneel.github.io/rhinocommon-api-docs/api/RhinoCommon/html/M_Rhino_Geometry_Brep_CreateFilletEdges.htm)
under private Xvfb. See [implementation and qualification](../fillet-edge.md).
