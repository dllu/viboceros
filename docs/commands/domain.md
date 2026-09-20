# Domain

[Command index](README.md)

Select one curve or surface and enter `Domain` to report its stored parameter
intervals. With no preselection, the application opens an object-selection prompt;
pick one eligible object and press Enter. Points, point clouds, and meshes are
not eligible. Multiple selected objects are rejected.

Curves report one interval; surfaces report U and V intervals. All native curve
families use their own domains, without converting to NURBS or using curve length
as a substitute. Reparameterized and imported domains are retained in the report.
No geometry, attributes, selection, or model undo/redo is changed by evaluation.

For a multi-face B-rep, the application then asks for a component-surface location.
`Domain 0.5,0.5,10` performs the same query at a world-space location. The nearest
trimmed face supplies its underlying surface's U/V domains; no aggregate domain is
invented for the polysurface. A single-face B-rep needs no additional pick.

For deterministic scripts, `Domain Face=0` reports a zero-based B-rep face index.
Face order is the current document topology order, not a persistent face identifier.
Out-of-range indices and unsupported options are errors. Example output:

```text
Curve domain = [-2,8]
Face 1: U domain = [5,6]; V domain = [-8,-3]
```

The component-point phase preserves the accepted object selection. Esc cancels
that phase without model edits; failed point evaluations stay open for correction.

[Rhino's Domain command](https://docs.mcneel.com/rhino/8/help/en-us/commands/domain.htm)
also supports `SubCrv`; that option is not implemented. `Face=index` is a native
scripting convenience, not a claim of identical Rhino command syntax. Component
lookup currently uses 3D nearest-face distance, not a screen-space hit aperture.

Native tests cover stored/reparameterized curve intervals, independently
reparameterized component surfaces, explicit and picked faces, invalid selection,
redo retention, and both preselected and postselected UI transitions. Live Rhino
command-output captures have not yet been added for this command.
