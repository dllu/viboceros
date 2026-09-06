# ConvertToSingleSpans

```text
ConvertToSingleSpans Direction=U DeleteInput=No
ConvertToSingleSpans Direction=V DeleteInput=Yes
ConvertToSingleSpans Direction=Both
ConvertToSingleSpans Direction=U Toggle
```

Exactly decomposes selected NURBS surfaces and single-face B-reps at nonempty
knot spans. U and V produce strips; Both produces Bézier patches. `Toggle`
switches U and V and is unavailable in Both. Curves, points, point clouds,
meshes, and multi-face B-reps are ignored. No eligible selection is an error.

If the requested direction(s) would produce only one piece, the source is left
completely unchanged, including trims, domains, identity, attributes, groups,
and selection. An accepted no-op still remembers the chosen options; it adds
no undo entry and does not clear redo.

Otherwise, requested axes become clamped single spans with `[0,1]` domains.
An untouched axis retains its complete knot vector, control count, and domain.
Both clamps and reparameterizes both axes even if one already has one span.
Trimmed inputs yield pieces of the full underlying surface, not the trim region.
Sources follow document order; patches follow increasing U-outer/V-inner order.

Outputs are fresh, unnamed, ungrouped, unselected current-layer objects with
color-by-layer attributes. `DeleteInput=No` retains sources exactly;
`DeleteInput=Yes` deletes converted sources, retaining empty group definitions.
It does not replace the first piece into the source object. Both choices leave
ineligible and no-op sources alone. Geometry is staged before document mutation;
the edit is atomic and undoable.

Direction and deletion choices are independently remembered by this command,
not shared with [`ConvertToBeziers`](beziers.md). See
[command-option lifetime](../command-options.md); use explicit options for
reproducible scripts. `ConvertSurfaceToSingleSpans` is a native alias.

## Geometry and verification

`NurbsSurface::try_single_span_patches` returns original span domains; the
command owns unit reparameterization. One-axis extraction uses local projective
blossoming with an independent origin and weight scale for each control line.
Both uses the shared tensor Bézier extractor. The same output/work limits and
explicit unrepresentable-control errors apply as in [Bézier conversion](beziers.md).
Periodic and unclamped structure on the untouched axis is retained.

The 78 `single_span_conversion.json` Rhino comparisons cover all directions and
deletion choices, U/V toggling, no-ops, one-axis-only multispans, rational and
unclamped surfaces, periodic seams, trimmed faces, ignored multi-face B-reps
(including reversed faces), and mixed/multiple sources. They compare full output NURBS definitions,
25 points per surface, domains, source identity, creation order, attributes,
selection, and complete group tables at absolute `1e-8`, relative `1e-12`.
The maximum observed error is below `9e-16`. Surface outputs must have trivial
trimming. Session probes separately check remembered choices and undo.

OpenNURBS omits the two mathematically unused outer knot slots. The conversion
oracle pads each omitted slot with its neighbor in both engines; it compares
every stored knot and never modifies model geometry. See
[McNeel's knot convention](https://developer.rhino3d.com/en/guides/opennurbs/superfluous-knots/).
Sample agreement is bounded evidence, not a continuous error or performance proof.

The [Rhino command reference](https://docs.mcneel.com/rhino/8/help/en-us/commands/converttosinglespans.htm)
documents direction and toggle controls. Domain, attribute, no-op, and option
memory policies above were measured with the licensed Rhino 8 oracle.

A private GUI check exercised U/Yes, V/No, the remembered bare command, U/V
toggle, and remembered Bézier deletion after undo/redo. Six 3DM exports retained
the expected exact geometry, split/untouched domains, source and fresh output
attributes, and empty groups.
